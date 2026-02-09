use std::{
    collections::{HashMap, HashSet},
    ffi::CString,
    path::Path,
    slice,
    sync::atomic::{AtomicU32, AtomicU64, Ordering},
    sync::{Arc, Mutex},
    time::Instant,
};

use anyhow::Context;
use std::sync::LazyLock;
use tracing::{error, info, trace};

use super::ztd::ZtdArchive;
use crate::{
    resource_manager::{
        bfresourcemgr::BFResourcePtr,
        ztfile::{ztfile_to_raw_resource, ZTFile, ZTFileType},
    },
    util::{get_from_memory, ZTString},
};

static LAZY_RESOURCE_MAP: LazyLock<Mutex<HashMap<String, LazyResource>>> = LazyLock::new(|| Mutex::new(HashMap::new()));
static TOTAL_LOADED_BYTES: AtomicU64 = AtomicU64::new(0);

// Track files that originated from disabled ZTDs
// Used to log errors only when vanilla actually tries to load them
static DISABLED_ZTD_FILES: LazyLock<Mutex<HashSet<String>>> = LazyLock::new(|| Mutex::new(HashSet::new()));

struct LazyResourceMap {}

#[derive(Clone)]
enum ResourceBacking {
    LazyZipFile { archive: Arc<Mutex<ZtdArchive>> },
    LoadedZipFile { archive: Arc<Mutex<ZtdArchive>>, data: u32 },
    Custom { data: u32 },
}

struct ConcreteResource {
    archive_name: Option<String>,
    filename: String,
    type_: ZTFileType,
    data: u32,
}

struct LazyResource {
    pub backing: ResourceBacking,
    pub filename: String,
    pub type_: ZTFileType,
    last_accessed: Instant,
    ref_count: Arc<AtomicU32>,
}

impl LazyResourceMap {
    fn remove(file_name: String) -> Option<()> {
        let mut binding = LAZY_RESOURCE_MAP.lock().unwrap();
        let value = binding.remove(&file_name)?;

        // Subtract size if resource was loaded
        let size = match &value.backing {
            ResourceBacking::LoadedZipFile { data, .. } | ResourceBacking::Custom { data } => unsafe { &*(*data as *const BFResourcePtr) }.content_size as u64,
            ResourceBacking::LazyZipFile { .. } => 0,
        };
        if size > 0 {
            TOTAL_LOADED_BYTES.fetch_sub(size, Ordering::Relaxed);
        }

        LazyResourceMap::drop_inner(value);
        Some(())
    }

    fn drop_inner(resource: LazyResource) {
        let data = match resource.backing {
            ResourceBacking::LoadedZipFile { data, archive: _ } => data,
            ResourceBacking::Custom { data } => data,
            ResourceBacking::LazyZipFile { archive: _ } => {
                return;
            }
        };
        let bf_resource_ptr = unsafe { Box::from_raw(data as *mut BFResourcePtr) };
        let size = bf_resource_ptr.content_size;

        trace!("Dropping resource: {} (type: {:?}, size: {} bytes)", resource.filename, resource.type_, size);

        match resource.type_ {
            ZTFileType::Ini
            | ZTFileType::Ai
            | ZTFileType::Ani
            | ZTFileType::Cfg
            | ZTFileType::Lyt
            | ZTFileType::Scn
            | ZTFileType::Uca
            | ZTFileType::Ucs
            | ZTFileType::Ucb
            | ZTFileType::Toml
            | ZTFileType::Txt => {
                let data_string = unsafe { CString::from_raw(data as *mut i8) };
                drop(data_string);
            }
            ZTFileType::Animation | ZTFileType::Bmp | ZTFileType::Lle | ZTFileType::Tga | ZTFileType::Wav | ZTFileType::Palette | ZTFileType::Zoo => {
                let data_vec: Box<[u8]> =
                    unsafe { Box::from_raw(std::ptr::slice_from_raw_parts_mut(bf_resource_ptr.data_ptr as *mut _, bf_resource_ptr.content_size as usize)) };
                drop(data_vec);
            }
        }
        drop(bf_resource_ptr);
    }

    fn insert_lazy(file_name: String, archive: Arc<Mutex<ZtdArchive>>) {
        let file_type = match ZTFileType::try_from(Path::new(&file_name)) {
            Ok(file_type) => file_type,
            Err(e) => {
                // Skip logging errors for known vanilla Zoo Tycoon files with unsupported types
                let known_vanilla_files = ["objects/ddogstnd/fancyblg_icons.zip", "bfupdateres.h"];
                if !known_vanilla_files.iter().any(|&f| file_name.eq_ignore_ascii_case(f)) {
                    error!("Error inserting file: {} error: {}", file_name, e);
                }
                return;
            }
        };

        let mut binding = LAZY_RESOURCE_MAP.lock().unwrap();
        if let Some(existing) = binding.insert(
            file_name.clone().to_ascii_lowercase(),
            LazyResource {
                backing: ResourceBacking::LazyZipFile { archive },
                filename: file_name.clone(),
                type_: file_type,
                last_accessed: Instant::now(),
                ref_count: Arc::new(AtomicU32::new(0)),
            },
        ) {
            LazyResourceMap::drop_inner(existing);
        }
    }

    fn insert_custom(file_name: String, file_type: ZTFileType, data: u32) {
        // Add to counter for custom resources
        let bf_ptr = unsafe { &*(data as *const BFResourcePtr) };
        TOTAL_LOADED_BYTES.fetch_add(bf_ptr.content_size as u64, Ordering::Relaxed);

        let mut binding = LAZY_RESOURCE_MAP.lock().unwrap();
        if let Some(existing) = binding.insert(
            file_name.to_ascii_lowercase(),
            LazyResource {
                backing: ResourceBacking::Custom { data },
                filename: file_name.clone(),
                type_: file_type,
                last_accessed: Instant::now(),
                ref_count: Arc::new(AtomicU32::new(0)),
            },
        ) {
            // Subtract size of replaced resource
            let old_size = match &existing.backing {
                ResourceBacking::LoadedZipFile { data, .. } | ResourceBacking::Custom { data } => unsafe { &*(*data as *const BFResourcePtr) }.content_size as u64,
                ResourceBacking::LazyZipFile { .. } => 0,
            };
            if old_size > 0 {
                TOTAL_LOADED_BYTES.fetch_sub(old_size, Ordering::Relaxed);
            }
            LazyResourceMap::drop_inner(existing);
        }
    }

    fn get(key: &str) -> anyhow::Result<Option<ConcreteResource>> {
        let mut binding = LAZY_RESOURCE_MAP.lock().unwrap();
        let lowercase_key = key.to_ascii_lowercase();
        let Some(resource) = binding.get_mut(&lowercase_key) else {
            info!("LazyResource not found: {}", lowercase_key);
            return Ok(None);
        };

        // Update last accessed time
        resource.last_accessed = Instant::now();

        // Increment ref count when resource is accessed
        resource.ref_count.fetch_add(1, Ordering::Relaxed);

        // Track if we're loading a new resource (for triggering auto-unload)
        let was_lazy = matches!(resource.backing, ResourceBacking::LazyZipFile { .. });

        // Clone the fields we need before potentially dropping the binding
        let filename = resource.filename.clone();
        let type_ = resource.type_;

        // TODO: Use std::mem::take/replace to avoid cloning
        let (archive_name, data) = match resource.backing.clone() {
            ResourceBacking::LazyZipFile { archive } => {
                let mut binding = archive.lock().unwrap();
                let archive_name = binding.name().to_string();
                let mut file = binding.by_name(&filename).with_context(|| format!("Error finding file in archive: {}", filename))?;
                let mut file_buffer = vec![0u8; file.size() as usize].into_boxed_slice();

                file.read_exact(&mut file_buffer).with_context(|| format!("Error reading file: {}", filename))?;

                let ztfile = ZTFile::builder()
                    .file_name(filename.clone())
                    .file_size(file_buffer.len() as u32)
                    .type_(type_)
                    .raw_data(file_buffer)
                    .build();
                let data = ztfile_to_raw_resource(&archive_name, filename.clone(), ztfile)?;
                resource.backing = ResourceBacking::LoadedZipFile {
                    archive: archive.clone(),
                    data: data.2,
                };
                (Some(archive_name.clone()), data.2)
            }
            ResourceBacking::LoadedZipFile { archive, data } => {
                let binding = archive.lock().unwrap();
                (Some(binding.name().to_string()), data)
            }
            ResourceBacking::Custom { data } => (None, data),
        };

        // If this was a lazy load, add to counter and check if we need to unload
        if was_lazy {
            // Add to total size counter when resource is loaded
            let bf_ptr = unsafe { &*(data as *const BFResourcePtr) };
            TOTAL_LOADED_BYTES.fetch_add(bf_ptr.content_size as u64, Ordering::Relaxed);

            // Trigger auto-unload check after loading new resource
            drop(binding); // Release lock before calling unload
            Self::maybe_unload_resources();
        }

        Ok(Some(ConcreteResource {
            archive_name: archive_name.clone(),
            filename,
            type_,
            data,
        }))
    }

    fn loaded_len() -> usize {
        let binding = LAZY_RESOURCE_MAP.lock().unwrap();
        binding
            .values()
            .filter(|x| matches!(x.backing, ResourceBacking::LoadedZipFile { .. } | ResourceBacking::Custom { .. }))
            .count()
    }

    fn not_loaded_len() -> usize {
        let binding = LAZY_RESOURCE_MAP.lock().unwrap();
        binding.values().filter(|x| matches!(x.backing, ResourceBacking::LazyZipFile { .. })).count()
    }

    fn len() -> usize {
        let binding = LAZY_RESOURCE_MAP.lock().unwrap();
        binding.len()
    }

    fn contains_key(key: &str) -> bool {
        let binding = LAZY_RESOURCE_MAP.lock().unwrap();
        binding.contains_key(key)
    }

    fn file_names() -> Vec<String> {
        let binding = LAZY_RESOURCE_MAP.lock().unwrap();
        binding.keys().cloned().collect()
    }

    /// Unload a single resource (transition LoadedZipFile -> LazyZipFile)
    /// Returns the size of the unloaded resource
    /// Only unloads if the resource's ref_count is 0
    fn unload_resource(key: &str) -> Option<u64> {
        let mut binding = LAZY_RESOURCE_MAP.lock().unwrap();
        let resource = binding.get_mut(key)?;

        // Only unload LoadedZipFile (NOT Custom or LazyZipFile)
        let (data, archive) = match &resource.backing {
            ResourceBacking::LoadedZipFile { data, archive } => (*data, archive.clone()),
            _ => return None,
        };

        // Check if resource has any refs - if so, don't unload
        let ref_count = resource.ref_count.load(Ordering::Relaxed);
        if ref_count > 0 {
            trace!("Skipping unload for {} (has {} active refs)", resource.filename, ref_count);
            return None;
        }

        // Get size before dropping
        let size = unsafe { &*(data as *const BFResourcePtr) }.content_size as u64;

        trace!("Unloading resource: {} (size: {} bytes)", resource.filename, size);

        // Transition back to lazy
        resource.backing = ResourceBacking::LazyZipFile { archive: archive.clone() };
        resource.last_accessed = Instant::now();

        // Drop the loaded data (preserve ref_count)
        let temp_resource = LazyResource {
            backing: ResourceBacking::LoadedZipFile { archive: archive.clone(), data },
            filename: resource.filename.clone(),
            type_: resource.type_,
            last_accessed: Instant::now(),
            ref_count: resource.ref_count.clone(),
        };
        Self::drop_inner(temp_resource);

        // Update global counter
        TOTAL_LOADED_BYTES.fetch_sub(size, Ordering::Relaxed);

        Some(size)
    }

    /// Unload all loaded resources
    /// Only unloads resources with ref_count == 0
    fn unload_all_loaded() -> UnloadResult {
        let mut binding = LAZY_RESOURCE_MAP.lock().unwrap();
        let keys_to_unload: Vec<String> = binding
            .iter()
            .filter(|(_, r)| {
                // Only unload LoadedZipFile with ref_count == 0
                matches!(r.backing, ResourceBacking::LoadedZipFile { .. }) && r.ref_count.load(Ordering::Relaxed) == 0
            })
            .map(|(k, _)| k.clone())
            .collect();

        let count = keys_to_unload.len();
        let mut total_size = 0u64;

        for key in keys_to_unload {
            if let Some(resource) = binding.remove(&key) {
                // Get size before dropping
                let size = match &resource.backing {
                    ResourceBacking::LoadedZipFile { data, .. } => unsafe { &*(*data as *const BFResourcePtr) }.content_size as u64,
                    _ => 0,
                };
                total_size += size;
                Self::drop_inner(resource);
                TOTAL_LOADED_BYTES.fetch_sub(size, Ordering::Relaxed);
            }
        }

        info!("Unloaded {} resources (freed {} bytes, {} MB)", count, total_size, total_size / (1024 * 1024));

        UnloadResult { count, total_size }
    }

    /// Automatic unloading based on memory limits and stale timeout
    fn maybe_unload_resources() {
        use crate::resource_manager::mod_config::get_openzt_config;
        use std::time::Duration;

        let config = get_openzt_config();
        let max_bytes = config.resource_cache.max_memory_mb as u64 * 1024 * 1024;
        let target_bytes = config.resource_cache.target_memory_mb as u64 * 1024 * 1024;
        let stale_duration = Duration::from_secs(config.resource_cache.stale_timeout_seconds);

        // Use running total instead of calculating
        let current_size = TOTAL_LOADED_BYTES.load(Ordering::Relaxed);

        // Only unload if over max threshold
        if current_size <= max_bytes {
            return;
        }

        let mut binding = LAZY_RESOURCE_MAP.lock().unwrap();

        // Collect candidates: (key, last_accessed, size, is_custom)
        // Custom resources are counted in size but NEVER unloaded
        // Only consider resources with ref_count == 0
        let mut candidates: Vec<(String, Instant, u64, bool)> = binding
            .iter()
            .filter_map(|(k, r)| {
                // Skip resources with active refs
                if r.ref_count.load(Ordering::Relaxed) > 0 {
                    return None;
                }
                match &r.backing {
                    ResourceBacking::LoadedZipFile { data, .. } => {
                        let ptr = unsafe { &*(*data as *const BFResourcePtr) };
                        Some((k.clone(), r.last_accessed, ptr.content_size as u64, false))
                    }
                    ResourceBacking::Custom { data } => {
                        let ptr = unsafe { &*(*data as *const BFResourcePtr) };
                        Some((k.clone(), r.last_accessed, ptr.content_size as u64, true))
                    }
                    ResourceBacking::LazyZipFile { .. } => None,
                }
            })
            .collect();

        // Sort by last_accessed (oldest first)
        candidates.sort_by_key(|(_, accessed, _, _)| *accessed);

        let mut unloaded_size = 0u64;
        let mut unloaded_count = 0usize;
        let now = Instant::now();

        for (key, accessed, size, is_custom) in candidates {
            // Stop if we're under target (considering only non-custom resources)
            let unloadable_total = current_size - unloaded_size;
            if unloadable_total <= target_bytes {
                break;
            }

            // Skip custom resources (they're counted in size but never unloaded)
            if is_custom {
                continue;
            }

            // Unload if stale OR still over target
            let is_stale = now.duration_since(accessed) > stale_duration;
            let still_over_target = unloadable_total > target_bytes;

            if is_stale || still_over_target {
                drop(binding); // Release lock before calling unload_resource
                if Self::unload_resource(&key).is_some() {
                    unloaded_size += size;
                    unloaded_count += 1;
                }
                binding = LAZY_RESOURCE_MAP.lock().unwrap(); // Re-acquire for next iteration
            }
        }

        if unloaded_count > 0 {
            info!(
                "Auto-unloaded {} resources due to memory pressure/stale timeout (freed {} bytes, {} MB)",
                unloaded_count,
                unloaded_size,
                unloaded_size / (1024 * 1024)
            );
        }
    }
}

pub fn check_file(file_name: &str) -> bool {
    LazyResourceMap::contains_key(&file_name.to_lowercase())
}

pub fn get_file_ptr(file_name: &str) -> Option<u32> {
    if let Ok(Some(resource)) = LazyResourceMap::get(&file_name.to_lowercase()) {
        Some(resource.data)
    } else {
        None
    }
}

pub fn get_file(file_name: &str) -> Option<(String, Box<[u8]>)> {
    match LazyResourceMap::get(file_name) {
        Ok(Some(file)) => {
            let resource_ptr = get_from_memory::<BFResourcePtr>(file.data);
            let tmp_slice = unsafe { slice::from_raw_parts(resource_ptr.data_ptr as *const _, resource_ptr.content_size as usize) };
            let mut new_slice = vec![0; resource_ptr.content_size as usize];
            new_slice.copy_from_slice(tmp_slice);
            Some((resource_ptr.bf_zip_name.copy_to_string(), new_slice.into_boxed_slice()))
        }
        Ok(None) => {
            info!("File not found: {}", file_name);
            None
        }
        Err(e) => {
            info!("Error getting file: {} error: {}", file_name, e);
            None
        }
    }
}

pub fn get_file_names() -> Vec<String> {
    LazyResourceMap::file_names()
}

pub fn get_num_resources() -> usize {
    LazyResourceMap::len()
}

pub fn add_ztfile(path: &Path, file_name: String, ztfile: ZTFile) -> anyhow::Result<()> {
    let ztd_path = path.to_str().with_context(|| format!("Failed to convert path to string: {}", path.display()))?;
    let (file_name, type_, data) = ztfile_to_raw_resource(ztd_path, file_name, ztfile)?;
    LazyResourceMap::insert_custom(file_name, type_, data);
    Ok(())
}

/// Add a ZTFile from memory/in-memory source, using mod_id as the source identifier
///
/// This is used when resources come from in-memory file maps rather than
/// filesystem paths, such as when patch source files are read from .ztd archives.
pub fn add_ztfile_from_memory(mod_id: &str, file_name: String, ztfile: ZTFile) -> anyhow::Result<()> {
    // Use the mod_id as the ztd_path for tracking
    let ztd_path = format!("zip::openzt_mods/{}", mod_id);
    let (file_name, type_, data) = ztfile_to_raw_resource(&ztd_path, file_name, ztfile)?;
    LazyResourceMap::insert_custom(file_name, type_, data);
    Ok(())
}

pub fn add_lazy(file_name: String, archive: Arc<Mutex<ZtdArchive>>) {
    LazyResourceMap::insert_lazy(file_name, archive);
}

pub fn remove_resource(file_name: &str) -> bool {
    LazyResourceMap::remove(file_name.to_lowercase()).is_some()
}

/// Update or insert a resource in the resource map
///
/// This is more efficient than calling remove_resource() + add_ztfile() as it only
/// acquires the lock once and atomically replaces the resource if it exists.
///
/// # Arguments
/// * `file_name` - The resource file name (will be lowercased for case-insensitive lookup)
/// * `file_type` - The type of the file
/// * `data` - Pointer to the resource data
pub fn update_resource(file_name: String, file_type: ZTFileType, data: u32) {
    LazyResourceMap::insert_custom(file_name, file_type, data)
}

/// Cache statistics for resource management
pub struct CacheStats {
    pub loaded_resources: usize,
    pub total_memory_bytes: u64,
    pub total_memory_mb: u64,
}

/// Result of unloading resources
pub struct UnloadResult {
    pub count: usize,
    pub total_size: u64,
}

/// Unload all loaded resources, freeing memory
///
/// Returns the number of resources unloaded and the total size freed in bytes.
pub fn unload_all_resources() -> UnloadResult {
    info!("Unloading all loaded resources");
    LazyResourceMap::unload_all_loaded()
}

/// Get current cache statistics
pub fn get_cache_stats() -> CacheStats {
    let binding = LAZY_RESOURCE_MAP.lock().unwrap();

    // Count loaded resources (both LoadedZipFile and Custom)
    let loaded_count = binding
        .values()
        .filter(|r| matches!(r.backing, ResourceBacking::LoadedZipFile { .. } | ResourceBacking::Custom { .. }))
        .count();

    // Use global counter for total size (includes custom resources)
    let total_size = TOTAL_LOADED_BYTES.load(Ordering::Relaxed);

    CacheStats {
        loaded_resources: loaded_count,
        total_memory_bytes: total_size,
        total_memory_mb: total_size / (1024 * 1024),
    }
}

/// Increment the reference count for a resource
///
/// This should be called when a resource is acquired for use.
/// Resources with ref_count > 0 will not be unloaded.
pub fn increment_ref(file_name: &str) -> bool {
    let lowercase_key = file_name.to_lowercase();
    let binding = LAZY_RESOURCE_MAP.lock().unwrap();
    if let Some(resource) = binding.get(&lowercase_key) {
        resource.ref_count.fetch_add(1, Ordering::Relaxed);
        true
    } else {
        false
    }
}

/// Decrement the reference count for a resource
///
/// This should be called when a resource is released.
/// Returns the new ref count (0 means the resource can now be unloaded).
pub fn decrement_ref(file_name: &str) -> Option<u32> {
    let lowercase_key = file_name.to_lowercase();
    let binding = LAZY_RESOURCE_MAP.lock().unwrap();
    if let Some(resource) = binding.get(&lowercase_key) {
        // We use fetch_sub with a check to prevent going below 0
        let mut old_count = resource.ref_count.load(Ordering::Relaxed);
        while old_count > 0 {
            match resource
                .ref_count
                .compare_exchange_weak(old_count, old_count - 1, Ordering::Relaxed, Ordering::Relaxed)
            {
                Ok(_) => return Some(old_count - 1),
                Err(new_old) => old_count = new_old,
            }
        }
        Some(0)
    } else {
        None
    }
}

/// Get the current reference count for a resource
///
/// Returns None if the resource doesn't exist.
pub fn get_ref_count(file_name: &str) -> Option<u32> {
    let lowercase_key = file_name.to_lowercase();
    let binding = LAZY_RESOURCE_MAP.lock().unwrap();
    binding.get(&lowercase_key).map(|r| r.ref_count.load(Ordering::Relaxed))
}

/// Dereference a resource by file name
///
/// This function is intended to be hooked into Vanilla Zoo Tycoon's
/// dereferencing function. It decrements our ref count when the game
/// releases a resource.
///
/// # Arguments
/// * `file_name` - The name of the resource to dereference
///
/// # Returns
/// * `true` if the resource was found and dereferenced
/// * `false` if the resource was not found
///
/// # Note
/// This is currently a stub that needs to be integrated with the
/// game's resource management via detours/hooks.
pub fn deref_resource(file_name: &str) -> bool {
    if let Some(new_count) = decrement_ref(file_name) {
        trace!("Dereferenced resource: {} (new count: {})", file_name, new_count);

        // If ref count reached 0, trigger a check to see if we should unload
        if new_count == 0 {
            // TODO: Should we trigger unloading here, or let the normal lazy unload handle it?
            // For now, we'll let the normal maybe_unload_resources() handle it
            // which is triggered after loading new resources.
        }
        true
    } else {
        false
    }
}

pub const DISABLED_ARCHIVE_NAME: &str = "<disabled>";

/// Create an empty resource (zero-length content)
/// Used for disabling specific file types from ZTDs
///
/// # Arguments
/// * `filename` - The resource file name
/// * `file_type` - The type of the file
pub fn create_empty_resource(filename: String, file_type: ZTFileType) -> anyhow::Result<()> {
    let lowercase_filename = filename.to_lowercase();

    // Create empty CString for text files
    let empty_cstring = CString::new("")?;
    let data_ptr = empty_cstring.into_raw() as u32;

    let bf_zip_name = CString::new(DISABLED_ARCHIVE_NAME.to_string())?;
    let bf_resource_name = CString::new(lowercase_filename.clone())?;

    let resource_ptr = Box::into_raw(Box::new(BFResourcePtr {
        num_refs: 100,
        bf_zip_name: bf_zip_name.into(),
        bf_resource_name: bf_resource_name.into(),
        data_ptr,
        content_size: 0,
    }));

    // Use LazyResourceMap::insert_custom to add the empty resource
    LazyResourceMap::insert_custom(lowercase_filename, file_type, resource_ptr as u32);
    Ok(())
}

/// Check if a file is already loaded in the resource map
///
/// # Arguments
/// * `file_name` - The file name to check (case-insensitive)
///
/// # Returns
/// * `true` if the file exists in the resource map
/// * `false` if the file is not in the resource map
pub fn check_file_loaded(file_name: &str) -> bool {
    LAZY_RESOURCE_MAP.lock().unwrap().contains_key(&file_name.to_lowercase())
}

/// Mark a file as originating from a disabled ZTD
///
/// This is used to track files from disabled ZTDs so that we can log
/// appropriate errors only when the vanilla game actually tries to load them.
///
/// # Arguments
/// * `file_name` - The file name to mark (case-insensitive)
pub fn mark_disabled_ztd_file(file_name: &str) {
    DISABLED_ZTD_FILES.lock().unwrap().insert(file_name.to_lowercase());
}

/// Check if a file originated from a disabled ZTD
///
/// This is used to detect when vanilla is trying to load a file from a disabled ZTD.
///
/// # Arguments
/// * `file_name` - The file name to check (case-insensitive)
///
/// # Returns
/// * `true` if the file was marked as coming from a disabled ZTD
/// * `false` otherwise
pub fn is_disabled_ztd_file(file_name: &str) -> bool {
    DISABLED_ZTD_FILES.lock().unwrap().contains(&file_name.to_lowercase())
}
