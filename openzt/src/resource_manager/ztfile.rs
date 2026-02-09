use std::{ffi::CString, fmt, path::Path, str};

use anyhow::{anyhow, Context};
use openzt_configparser::ini::{Ini, WriteOptions};

use crate::{
    animation::Animation,
    resource_manager::{bfresourcemgr::BFResourcePtr, lazyresourcemap::get_file_ptr},
    util::{get_from_memory, save_to_memory, ZTString},
};

#[derive(Debug, Clone)]
pub enum ZTFile {
    Text(CString, ZTFileType, u32),
    RawBytes(Box<[u8]>, ZTFileType, u32),
}

#[derive(Debug, Clone, Copy)]
pub enum ZTFileType {
    Ai,
    Ani,
    Cfg,
    Lyt,
    Scn,
    Uca,
    Ucs,
    Ucb,
    Ini,
    Txt,
    Toml,
    Animation,
    Palette,
    Tga,
    Wav,
    Lle,
    Bmp,
    Zoo,
}

impl fmt::Display for ZTFileType {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self {
            ZTFileType::Animation => write!(f, "animation"),
            ZTFileType::Palette => write!(f, "palette"),
            ZTFileType::Ani => write!(f, "ani"),
            ZTFileType::Ai => write!(f, "ai"),
            ZTFileType::Cfg => write!(f, "cfg"),
            ZTFileType::Lyt => write!(f, "lyt"),
            ZTFileType::Scn => write!(f, "scn"),
            ZTFileType::Uca => write!(f, "uca"),
            ZTFileType::Ucs => write!(f, "ucs"),
            ZTFileType::Ucb => write!(f, "ucb"),
            ZTFileType::Ini => write!(f, "ini"),
            ZTFileType::Txt => write!(f, "txt"),
            ZTFileType::Toml => write!(f, "toml"),
            ZTFileType::Tga => write!(f, "tga"),
            ZTFileType::Wav => write!(f, "wav"),
            ZTFileType::Lle => write!(f, "lle"),
            ZTFileType::Bmp => write!(f, "bmp"),
            ZTFileType::Zoo => write!(f, "zoo"),
        }
    }
}

impl TryFrom<&Path> for ZTFileType {
    type Error = &'static str;

    fn try_from(path: &Path) -> Result<Self, Self::Error> {
        let extension = path.extension().unwrap_or_default().to_ascii_lowercase();
        Ok(match extension.to_str().unwrap_or_default() {
            "ai" => ZTFileType::Ai,
            "ani" => ZTFileType::Ani,
            "cfg" => ZTFileType::Cfg,
            "lyt" => ZTFileType::Lyt,
            "scn" => ZTFileType::Scn,
            "uca" => ZTFileType::Uca,
            "ucs" => ZTFileType::Ucs,
            "ucb" => ZTFileType::Ucb,
            "ini" => ZTFileType::Ini,
            "txt" => ZTFileType::Txt,
            "toml" => ZTFileType::Toml,
            "tga" => ZTFileType::Tga,
            "wav" => ZTFileType::Wav,
            "lle" => ZTFileType::Lle,
            "bmp" => ZTFileType::Bmp,
            "pal" => ZTFileType::Palette,
            "zoo" => ZTFileType::Zoo,
            "" => ZTFileType::Animation,
            _ => return Err("Invalid file type"),
        })
    }
}

impl From<BFResourcePtr> for ZTFile {
    fn from(bf_resource_ptr: BFResourcePtr) -> Self {
        let filename = bf_resource_ptr.bf_resource_name.copy_to_string();
        let file_extension = Path::new(&filename).extension().unwrap_or_default().to_str().unwrap_or_default();
        let file_size = bf_resource_ptr.content_size;
        let data = bf_resource_ptr.data_ptr;
        match file_extension {
            "ai" => ZTFile::Text(unsafe { CString::from_raw(data as *mut i8) }, ZTFileType::Ai, file_size),
            "cfg" => ZTFile::Text(unsafe { CString::from_raw(data as *mut i8) }, ZTFileType::Cfg, file_size),
            "lyt" => ZTFile::Text(unsafe { CString::from_raw(data as *mut i8) }, ZTFileType::Lyt, file_size),
            "scn" => ZTFile::Text(unsafe { CString::from_raw(data as *mut i8) }, ZTFileType::Scn, file_size),
            "uca" => ZTFile::Text(unsafe { CString::from_raw(data as *mut i8) }, ZTFileType::Uca, file_size),
            "ucs" => ZTFile::Text(unsafe { CString::from_raw(data as *mut i8) }, ZTFileType::Ucs, file_size),
            "ucb" => ZTFile::Text(unsafe { CString::from_raw(data as *mut i8) }, ZTFileType::Ucb, file_size),
            "ani" => ZTFile::Text(unsafe { CString::from_raw(data as *mut i8) }, ZTFileType::Ani, file_size),
            "ini" => ZTFile::Text(unsafe { CString::from_raw(data as *mut i8) }, ZTFileType::Ini, file_size),
            "txt" => ZTFile::Text(unsafe { CString::from_raw(data as *mut i8) }, ZTFileType::Txt, file_size),
            "tga" => ZTFile::RawBytes(
                unsafe { Box::from_raw(std::ptr::slice_from_raw_parts_mut(data as *mut _, file_size as usize)) },
                ZTFileType::Tga,
                file_size,
            ),
            "pal" => ZTFile::RawBytes(
                unsafe { Box::from_raw(std::ptr::slice_from_raw_parts_mut(data as *mut _, file_size as usize)) },
                ZTFileType::Palette,
                file_size,
            ),
            "wav" => ZTFile::RawBytes(
                unsafe { Box::from_raw(std::ptr::slice_from_raw_parts_mut(data as *mut _, file_size as usize)) },
                ZTFileType::Wav,
                file_size,
            ),
            "lle" => ZTFile::RawBytes(
                unsafe { Box::from_raw(std::ptr::slice_from_raw_parts_mut(data as *mut _, file_size as usize)) },
                ZTFileType::Lle,
                file_size,
            ),
            "bmp" => ZTFile::RawBytes(
                unsafe { Box::from_raw(std::ptr::slice_from_raw_parts_mut(data as *mut _, file_size as usize)) },
                ZTFileType::Bmp,
                file_size,
            ),
            _ => ZTFile::RawBytes(
                unsafe { Box::from_raw(std::ptr::slice_from_raw_parts_mut(data as *mut _, file_size as usize)) },
                ZTFileType::Animation,
                file_size,
            ),
        }
    }
}

impl ZTFile {
    pub fn builder() -> ZTFileBuilder<false, false, false, false> {
        ZTFileBuilder {
            file_name: None,
            file_size: None,
            type_: None,
            raw_data: None,
            cstring_data: None,
        }
    }
}

pub struct ZTFileBuilder<const HAS_FILE_NAME: bool, const HAS_FILE_SIZE: bool, const HAS_TYPE: bool, const HAS_DATA: bool> {
    file_name: Option<String>,
    file_size: Option<u32>,
    type_: Option<ZTFileType>,
    raw_data: Option<Box<[u8]>>,
    cstring_data: Option<CString>,
}

impl<const HAS_FILE_NAME: bool, const HAS_FILE_SIZE: bool, const HAS_TYPE: bool, const HAS_DATA: bool>
    ZTFileBuilder<HAS_FILE_NAME, HAS_FILE_SIZE, HAS_TYPE, HAS_DATA>
{
    pub fn file_name(self, file_name: String) -> ZTFileBuilder<true, HAS_FILE_SIZE, HAS_TYPE, HAS_DATA> {
        ZTFileBuilder {
            file_name: Some(file_name),
            file_size: self.file_size,
            type_: self.type_,
            raw_data: self.raw_data,
            cstring_data: self.cstring_data,
        }
    }

    pub fn file_size(self, file_size: u32) -> ZTFileBuilder<HAS_FILE_NAME, true, HAS_TYPE, HAS_DATA> {
        ZTFileBuilder {
            file_name: self.file_name,
            file_size: Some(file_size),
            type_: self.type_,
            raw_data: self.raw_data,
            cstring_data: self.cstring_data,
        }
    }

    pub fn type_(self, type_: ZTFileType) -> ZTFileBuilder<HAS_FILE_NAME, HAS_FILE_SIZE, true, HAS_DATA> {
        ZTFileBuilder {
            file_name: self.file_name,
            file_size: self.file_size,
            type_: Some(type_),
            raw_data: self.raw_data,
            cstring_data: self.cstring_data,
        }
    }
}

impl<const HAS_FILE_NAME: bool, const HAS_FILE_SIZE: bool, const HAS_TYPE: bool> ZTFileBuilder<HAS_FILE_NAME, HAS_FILE_SIZE, HAS_TYPE, false> {
    pub fn raw_data(self, raw_data: Box<[u8]>) -> ZTFileBuilder<HAS_FILE_NAME, HAS_FILE_SIZE, HAS_TYPE, true> {
        ZTFileBuilder {
            file_name: self.file_name,
            file_size: self.file_size,
            type_: self.type_,
            raw_data: Some(raw_data),
            cstring_data: self.cstring_data,
        }
    }

    pub fn cstring_data(self, cstring_data: CString) -> ZTFileBuilder<HAS_FILE_NAME, HAS_FILE_SIZE, HAS_TYPE, true> {
        ZTFileBuilder {
            file_name: self.file_name,
            file_size: self.file_size,
            type_: self.type_,
            raw_data: self.raw_data,
            cstring_data: Some(cstring_data),
        }
    }
}

impl ZTFileBuilder<true, true, true, true> {
    pub fn build(self) -> ZTFile {
        if self.raw_data.is_some() {
            unsafe { ZTFile::RawBytes(self.raw_data.unwrap_unchecked(), self.type_.unwrap_unchecked(), self.file_size.unwrap_unchecked()) }
        } else {
            unsafe { ZTFile::Text(self.cstring_data.unwrap_unchecked(), self.type_.unwrap_unchecked(), self.file_size.unwrap_unchecked()) }
        }
    }
}

pub fn modify_ztfile<F>(file_name: &str, modifier: F) -> anyhow::Result<()>
where
    F: Fn(&mut BFResourcePtr) -> anyhow::Result<()>,
{
    let bf_resource_ptr_ptr = get_file_ptr(file_name).ok_or_else(|| anyhow!("File not found: {}", file_name))?;
    let mut bf_resource_ptr = get_from_memory::<BFResourcePtr>(bf_resource_ptr_ptr);

    modifier(&mut bf_resource_ptr)?;

    save_to_memory::<BFResourcePtr>(bf_resource_ptr_ptr, bf_resource_ptr.clone());

    Ok(())
}

pub fn modify_ztfile_as_ini<F>(file_name: &str, modifier: F) -> anyhow::Result<()>
where
    F: Fn(&mut Ini) -> anyhow::Result<()>,
{
    modify_ztfile(file_name, |file: &mut BFResourcePtr| {
        let c_string = unsafe { CString::from_raw(file.data_ptr as *mut i8) };
        let bytes = c_string.as_bytes();
        let decoded_string = crate::encoding_utils::decode_game_text(bytes);
        let mut cfg = Ini::new_cs();
        cfg.set_comment_symbols(&[';', '#', ':']);

        cfg.read(decoded_string).map_err(|s| anyhow!("Error reading ini: {}", s))?;

        modifier(&mut cfg)?;

        let mut write_options = WriteOptions::default();
        write_options.space_around_delimiters = true;
        write_options.blank_lines_between_sections = 1;
        let new_string = cfg.pretty_writes(&write_options);
        file.content_size = new_string.len() as u32;

        let new_c_string = CString::new(new_string).with_context(|| format!("Error converting ini to CString after modifying {}", file_name))?;
        file.data_ptr = new_c_string.into_raw() as u32;
        Ok(())
    })
}

pub fn modify_ztfile_as_animation<F>(file_name: &str, modifier: F) -> anyhow::Result<()>
where
    F: Fn(&mut Animation) -> anyhow::Result<()>,
{
    modify_ztfile(file_name, |file: &mut BFResourcePtr| {
        let data_vec: Box<[u8]> = unsafe { Box::from_raw(std::ptr::slice_from_raw_parts_mut(file.data_ptr as *mut _, file.content_size as usize)) };
        let mut animation = Animation::parse(&data_vec)?;

        modifier(&mut animation)?;

        let (new_animation_bytes, length) = animation.write()?;
        let boxed_slice = new_animation_bytes.into_boxed_slice();
        let data_ptr = boxed_slice.as_ptr() as u32;
        std::mem::forget(boxed_slice);
        file.data_ptr = data_ptr;
        file.content_size = length as u32;
        Ok(())
    })
}

pub fn ztfile_to_raw_resource(path: &str, file_name: String, ztfile: ZTFile) -> anyhow::Result<(String, ZTFileType, u32)> {
    let mut ztd_path = path.to_string();
    ztd_path = ztd_path.replace('\\', "/").replace("./", "zip::./");
    let lowercase_filename = file_name.to_lowercase();

    let bf_zip_name = CString::new(ztd_path.clone()).with_context(|| format!("Error converting zip name to CString: {}", ztd_path))?;

    let bf_resource_name = CString::new(lowercase_filename.clone()).with_context(|| format!("Error converting resource name to CString: {}", lowercase_filename))?;

    match ztfile {
        ZTFile::Text(data, type_, length) => {
            let ptr = data.into_raw() as u32;
            let resource_ptr = Box::into_raw(Box::new(BFResourcePtr {
                num_refs: 100, // We set this very high to prevent the game from unloading the resource
                bf_zip_name: bf_zip_name.into(),
                bf_resource_name: bf_resource_name.into(),
                data_ptr: ptr,
                content_size: length,
            }));

            Ok((lowercase_filename, type_, resource_ptr as _))
        }
        ZTFile::RawBytes(data, type_, length) => {
            let ptr = data.as_ptr() as u32;
            std::mem::forget(data);
            let resource_ptr = Box::into_raw(Box::new(BFResourcePtr {
                num_refs: 100, // We set this very high to prevent the game from unloading the resource
                bf_zip_name: bf_zip_name.into(),
                bf_resource_name: bf_resource_name.into(),
                data_ptr: ptr,
                content_size: length,
            }));

            Ok((lowercase_filename, type_, resource_ptr as _))
        }
    }
}
