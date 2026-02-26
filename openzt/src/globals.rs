//! Centralized registry for accessing global C++ manager instances.
//!
//! This module provides a type-safe, lazy-initialized way to access global
//! manager instances from the injected DLL. It encapsulates pointer chain
//! resolution and eliminates scattered magic numbers.

use std::ffi::CString;
use std::marker::PhantomData;
use std::sync::OnceLock;

// Forward declarations of manager types - these are defined in their respective modules
// We use opaque pointers here to avoid circular dependencies

/// Opaque type for ZTWorldMgr
#[repr(C)]
pub struct ZTWorldMgr {
    _private: [u8; 0], // Size will be determined by the actual definition
}

/// Opaque type for ZTHabitatMgr
#[repr(C)]
pub struct ZTHabitatMgr {
    _private: [u8; 0],
}

/// Opaque type for ZTAdvTerrainMgr_raw
#[repr(C)]
pub struct ZTAdvTerrainMgr_raw {
    _private: [u8; 0],
}

/// Opaque type for ZTGameMgr
#[repr(C)]
pub struct ZTGameMgr {
    _private: [u8; 0],
}

/// Opaque type for BFResourceMgr
#[repr(C)]
pub struct BFResourceMgr {
    _private: [u8; 0],
}

/// Walks a pointer chain and returns the final address.
///
/// # Arguments
/// * `base` - The starting address
/// * `offsets` - Array of offsets to apply:
///   - `&[]`: Direct access, no dereference
///   - `&[0]`: Dereference once, add 0
///   - `&[0, 0x58]`: Dereference, add 0, dereference, add 0x58
///
/// # Safety
/// This function performs raw pointer dereferencing. The caller must ensure
/// that all addresses in the chain are valid and aligned.
unsafe fn resolve_chain(base: usize, offsets: &[usize]) -> Option<usize> {
    let mut addr = base;

    // Empty offsets = direct access, no dereference
    if offsets.is_empty() {
        return Some(addr);
    }

    // For each offset: dereference, then add offset
    for &offset in offsets {
        unsafe {
            addr = *(addr as *const usize);
        }
        if addr == 0 {
            return None;
        }
        addr = addr.wrapping_add(offset);
    }

    Some(addr)
}

/// Cached global instance wrapper for stable singleton objects.
///
/// This type is used for global C++ objects that are created once and live
/// for the lifetime of the game (e.g., manager singletons). The resolved
/// pointer is cached after the first access to avoid repeated chain resolution.
pub struct CachedGlobalInstance<T> {
    base: usize,
    offsets: &'static [usize],
    cache: OnceLock<usize>,
    _marker: PhantomData<*mut T>,
}

impl<T> CachedGlobalInstance<T> {
    /// Creates a new cached global instance.
    ///
    /// # Arguments
    /// * `base` - The base address (usually module_base + offset)
    /// * `offsets` - The pointer chain offsets
    pub const fn new(base: usize, offsets: &'static [usize]) -> Self {
        Self {
            base,
            offsets,
            cache: OnceLock::new(),
            _marker: PhantomData,
        }
    }

    /// Returns a raw pointer to the instance, or null if resolution fails.
    ///
    /// The first call resolves the pointer chain and caches the result.
    /// Subsequent calls return the cached value.
    pub unsafe fn get(&self) -> *mut T {
        let addr = self.cache.get_or_init(|| {
            unsafe { resolve_chain(self.base, self.offsets).unwrap_or(0) }
        });
        if *addr == 0 {
            std::ptr::null_mut()
        } else {
            *addr as *mut T
        }
    }
}

// SAFETY: CachedGlobalInstance only contains plain integers (base address and offsets)
// and a OnceLock which is thread-safe. It's safe to share across threads.
unsafe impl<T> Send for CachedGlobalInstance<T> {}
unsafe impl<T> Sync for CachedGlobalInstance<T> {}

/// Centralized registry for all global manager instances.
pub struct Globals {
    ztworldmgr: CachedGlobalInstance<ZTWorldMgr>,
    zthabitatmgr: CachedGlobalInstance<ZTHabitatMgr>,
    ztadvterrainmgr: CachedGlobalInstance<ZTAdvTerrainMgr_raw>,
    ztgamemgr: CachedGlobalInstance<ZTGameMgr>,
    bfresourcemgr: CachedGlobalInstance<BFResourceMgr>,
}

impl Globals {
    /// Returns a shared reference to the ZTWorldMgr (read-only).
    pub fn ztworldmgr(&self) -> &crate::ztworldmgr::ZTWorldMgr {
        unsafe {
            &*(self.ztworldmgr.get() as *const crate::ztworldmgr::ZTWorldMgr)
        }
    }

    /// Returns a raw mutable pointer to the ZTWorldMgr for mutation.
    pub fn ztworldmgr_ptr(&self) -> *mut crate::ztworldmgr::ZTWorldMgr {
        unsafe {
            self.ztworldmgr.get() as *mut crate::ztworldmgr::ZTWorldMgr
        }
    }

    /// Returns a shared reference to the ZTHabitatMgr (read-only).
    pub fn zthabitatmgr(&self) -> &crate::zthabitatmgr::ZTHabitatMgr {
        unsafe {
            &*(self.zthabitatmgr.get() as *const crate::zthabitatmgr::ZTHabitatMgr)
        }
    }

    /// Returns a raw mutable pointer to the ZTHabitatMgr for mutation.
    pub fn zthabitatmgr_ptr(&self) -> *mut crate::zthabitatmgr::ZTHabitatMgr {
        unsafe {
            self.zthabitatmgr.get() as *mut crate::zthabitatmgr::ZTHabitatMgr
        }
    }

    /// Returns a shared reference to the ZTAdvTerrainMgr_raw (read-only).
    pub fn ztadvterrainmgr(&self) -> &crate::ztadvterrainmgr::ZTAdvTerrainMgr_raw {
        unsafe {
            &*(self.ztadvterrainmgr.get() as *const crate::ztadvterrainmgr::ZTAdvTerrainMgr_raw)
        }
    }

    /// Returns a raw mutable pointer to the ZTAdvTerrainMgr_raw for mutation.
    pub fn ztadvterrainmgr_ptr(&self) -> *mut crate::ztadvterrainmgr::ZTAdvTerrainMgr_raw {
        unsafe {
            self.ztadvterrainmgr.get() as *mut crate::ztadvterrainmgr::ZTAdvTerrainMgr_raw
        }
    }

    /// Returns a shared reference to the ZTGameMgr (read-only).
    pub fn ztgamemgr(&self) -> &crate::ztgamemgr::ZTGameMgr {
        unsafe {
            &*(self.ztgamemgr.get() as *const crate::ztgamemgr::ZTGameMgr)
        }
    }

    /// Returns a raw mutable pointer to the ZTGameMgr for mutation.
    pub fn ztgamemgr_ptr(&self) -> *mut crate::ztgamemgr::ZTGameMgr {
        unsafe {
            self.ztgamemgr.get() as *mut crate::ztgamemgr::ZTGameMgr
        }
    }

    /// Returns a shared reference to the BFResourceMgr (read-only).
    pub fn bfresourcemgr(&self) -> &crate::resource_manager::bfresourcemgr::BFResourceMgr {
        unsafe {
            &*(self.bfresourcemgr.get() as *const crate::resource_manager::bfresourcemgr::BFResourceMgr)
        }
    }

    /// Returns a raw mutable pointer to the BFResourceMgr for mutation.
    pub fn bfresourcemgr_ptr(&self) -> *mut crate::resource_manager::bfresourcemgr::BFResourceMgr {
        unsafe {
            self.bfresourcemgr.get() as *mut crate::resource_manager::bfresourcemgr::BFResourceMgr
        }
    }
}

// SAFETY: Globals only contains CachedGlobalInstance values which are Send + Sync
unsafe impl Send for Globals {}
unsafe impl Sync for Globals {}

/// Static storage for the globals registry.
static GLOBALS: OnceLock<Globals> = OnceLock::new();

/// Gets the module base address for the given module name.
///
/// # Arguments
/// * `name` - The name of the module (e.g., "zoo.exe")
///
/// # Panics
/// Panics if the module cannot be found.
pub fn get_module_base(name: &str) -> usize {
    let cname = CString::new(name).unwrap();
    unsafe {
        windows::Win32::System::LibraryLoader::GetModuleHandleA(
            windows::core::PCSTR(cname.as_ptr() as _)
        )
        .unwrap()
        .0 as usize
    }
}

/// Ensures the globals registry is initialized and returns it.
///
/// The registry is lazily initialized on first access. This function is
/// thread-safe and can be called from any thread.
fn ensure_globals() -> &'static Globals {
    GLOBALS.get_or_init(|| {
        let base = get_module_base("zoo.exe");
        Globals {
            // All offsets are &[0] because each address points to a pointer to the struct
            // (single indirection)
            ztworldmgr: CachedGlobalInstance::new(base + 0x00238040, &[0]),
            zthabitatmgr: CachedGlobalInstance::new(base + 0x0023805c, &[0]),
            ztadvterrainmgr: CachedGlobalInstance::new(base + 0x00238058, &[0]),
            ztgamemgr: CachedGlobalInstance::new(base + 0x00238048, &[0]),
            // BFResourceMgr uses empty offsets because the global address points directly
            // to the struct (no indirection)
            bfresourcemgr: CachedGlobalInstance::new(base + 0x002380C0, &[]),
        }
    })
}

/// Returns the global manager registry.
///
/// This is the main entry point for accessing global managers.
///
/// # Example
/// ```ignore
/// let world_mgr = globals().ztworldmgr();
/// println!("Map size: {}x{}", world_mgr.map_x_size, world_mgr.map_y_size);
/// ```
pub fn globals() -> &'static Globals {
    ensure_globals()
}
