use std::{
    ffi::{c_char, CStr, CString},
    fmt, marker,
    mem::transmute,
    path::PathBuf,
    ptr,
};

#[cfg(target_os = "windows")]
use windows::Win32::System::Memory::{VirtualProtect, PAGE_EXECUTE_READWRITE, PAGE_PROTECTION_FLAGS};

// TODO: Test replacing most uses of get_from_memory with map_from_memory : Unclear if we need mem::forget each reference afterwards?
pub fn map_from_memory<T>(address: u32) -> &'static mut T {
    unsafe { transmute::<u32, &mut T>(address) }
}

pub fn get_from_memory<T>(address: u32) -> T {
    unsafe { ptr::read(address as *const T) }
}

pub fn checked_get_from_memory<T: Checkable>(address: u32) -> anyhow::Result<T> {
    T::check(address)?;
    Ok(unsafe { ptr::read(address as *const T) })
}

pub fn save_to_memory<T>(address: u32, value: T) {
    unsafe { ptr::write(address as *mut T, value) };
}

#[cfg(target_os = "windows")]
pub fn save_to_protected_memory<T>(address: u32, value: T) -> anyhow::Result<()> {
    unsafe {
        {
            let mut old_protect: PAGE_PROTECTION_FLAGS = PAGE_PROTECTION_FLAGS(0);
            VirtualProtect(address as *mut _, std::mem::size_of::<T>(), PAGE_EXECUTE_READWRITE, &mut old_protect)?;
            ptr::write(address as *mut _, value);
            VirtualProtect(address as *mut _, std::mem::size_of::<T>(), old_protect, &mut old_protect)?;
        }
    }
    Ok(())
}

pub fn get_base_path() -> PathBuf {
    let mut exe_location = std::env::current_exe().unwrap();
    exe_location.pop();
    exe_location
}

pub fn get_ini_path() -> PathBuf {
    let mut exe_location = get_base_path();
    exe_location.push("zoo.ini");
    exe_location
}

#[cfg(target_os = "windows")]
pub fn patch_nop(address: u32) -> anyhow::Result<()> {
    unsafe {
        {
            let mut old_protect: PAGE_PROTECTION_FLAGS = PAGE_PROTECTION_FLAGS(0);
            VirtualProtect(address as *mut _, 1, PAGE_EXECUTE_READWRITE, &mut old_protect)?;
            ptr::write(address as *mut _, 0x90u8);
            VirtualProtect(address as *mut _, 1, old_protect, &mut old_protect)?;
        }
    }
    Ok(())
}

pub fn get_string_from_memory_with_size(address: u32, size: u32) -> String {
    get_string_from_memory_bounded(address, address + size, address + size)
}

pub fn get_string_from_memory_bounded(start: u32, end: u32, buffer_end: u32) -> String {
    let mut bytes = Vec::new();
    let mut char_address = start;
    while {
        let byte = get_from_memory::<u8>(char_address);
        byte != 0 && char_address < end && char_address < buffer_end
    } {
        bytes.push(get_from_memory::<u8>(char_address));
        char_address += 1;
    }
    crate::encoding_utils::decode_game_text(&bytes)
}

pub fn get_string_from_memory(address: u32) -> String {
    let mut bytes = Vec::new();
    let mut char_address = address;
    while {
        let byte = get_from_memory::<u8>(char_address);
        byte != 0
    } {
        bytes.push(get_from_memory::<u8>(char_address));
        char_address += 1;
    }
    crate::encoding_utils::decode_game_text(&bytes)
}

pub fn save_string_to_memory(address: u32, string: &str) {
    let encoded_bytes = crate::encoding_utils::encode_to_ansi(string);
    let mut char_address = address;
    for byte in encoded_bytes {
        save_to_memory::<u8>(char_address, byte);
        char_address += 1;
    }
    save_to_memory::<u8>(char_address, 0);
}

// TODO: What else should we have here?
// TODO: Impl for ZTString (rename this), ZTShortString and a simple cstr/*const c_char wrapper?
// TODO: Should implement helper methods for any struct that implements ZTString, this should just be the minimum interface requried
pub trait ZTString {
    fn len(&self) -> usize;
    fn capacity(&self) -> usize;
    fn replace(&mut self, new_string: String) -> anyhow::Result<()>;
    fn get_cstr(&self) -> &CStr;
    fn copy_to_string(&self) -> String;
}

#[derive(Debug, Clone)]
#[repr(C)]
pub struct ZTBufferString {
    start_ptr: u32,
    end_ptr: u32,
    buffer_end_ptr: u32,
}

impl fmt::Display for ZTBufferString {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "{}", self.copy_to_string())
    }
}

impl ZTString for ZTBufferString {
    fn len(&self) -> usize {
        (self.end_ptr - self.start_ptr) as usize
    }

    fn capacity(&self) -> usize {
        (self.buffer_end_ptr - self.start_ptr) as usize
    }

    fn replace(&mut self, new_string: String) -> anyhow::Result<()> {
        if new_string.len() + 1 > self.capacity() {
            Err(anyhow::anyhow!("New string is too long"))
        } else {
            let new_end_ptr = self.start_ptr + new_string.len() as u32;
            save_string_to_memory(self.start_ptr, &new_string);
            self.end_ptr = new_end_ptr;
            Ok(())
        }
    }

    fn get_cstr(&self) -> &CStr {
        unsafe { CStr::from_ptr(self.start_ptr as *const c_char) }
    }

    fn copy_to_string(&self) -> String {
        get_string_from_memory_bounded(self.start_ptr, self.end_ptr, self.buffer_end_ptr)
    }
}

#[derive(Debug, Clone)]
#[repr(C)]
pub struct ZTBoundedString {
    start_ptr: u32,
    end_ptr: u32,
}

impl ZTString for ZTBoundedString {
    fn len(&self) -> usize {
        (self.end_ptr - self.start_ptr) as usize
    }

    fn capacity(&self) -> usize {
        self.len()
    }

    fn replace(&mut self, new_string: String) -> anyhow::Result<()> {
        if new_string.len() + 1 != self.capacity() {
            Err(anyhow::anyhow!("New string is too long"))
        } else {
            let new_end_ptr = self.start_ptr + new_string.len() as u32;
            save_string_to_memory(self.start_ptr, &new_string);
            self.end_ptr = new_end_ptr;
            Ok(())
        }
    }

    fn get_cstr(&self) -> &CStr {
        unsafe { CStr::from_ptr(self.start_ptr as *const c_char) }
    }

    fn copy_to_string(&self) -> String {
        get_string_from_memory_bounded(self.start_ptr, self.end_ptr, self.end_ptr)
    }
}

impl fmt::Display for ZTBoundedString {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "{}", self.copy_to_string())
    }
}

#[derive(Debug, Clone)]
#[repr(C)]
pub struct ZTStringPtr {
    ptr: *const c_char,
}

impl ZTString for ZTStringPtr {
    fn len(&self) -> usize {
        unsafe { CStr::from_ptr(self.ptr).to_bytes().len() }
    }

    fn capacity(&self) -> usize {
        self.len()
    }

    fn replace(&mut self, _new_string: String) -> anyhow::Result<()> {
        // TODO: We could probably implement this, by getting the current length of the string and making sure the new string is the exact same size? Or padding with spaces if smaller?
        Err(anyhow::anyhow!("Cannot replace string without bounds"))
    }

    fn get_cstr(&self) -> &CStr {
        unsafe { CStr::from_ptr(self.ptr) }
    }

    fn copy_to_string(&self) -> String {
        unsafe { CStr::from_ptr(self.ptr).to_string_lossy().into_owned() }
    }
}

impl fmt::Display for ZTStringPtr {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "{}", self.copy_to_string())
    }
}

impl From<CString> for ZTStringPtr {
    fn from(c_string: CString) -> Self {
        ZTStringPtr { ptr: c_string.into_raw() }
    }
}

#[derive(Debug)]
#[repr(C)]
pub struct ZTArray<T> {
    // ptr: *const T,
    start_ptr: u32,
    end_ptr: u32,
    buffer_end_ptr: u32,
    // Rust doesn't allow us to have a struct that is generic over T without referencing it,
    //  because we're just storing pointers to an array of type T we use PhantomData to tell the compiler that we're using T
    _marker: marker::PhantomData<T>,
    // _marker: marker::PhantomData<&'a T>,
}

impl<T> fmt::Display for ZTArray<T> {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(
            f,
            "ZTArray {{ start_ptr: {:#x}, end_ptr: {:#x}, buffer_end_ptr: {:#x} }} -> len({})",
            self.start_ptr,
            self.end_ptr,
            self.buffer_end_ptr,
            self.len()
        )
    }
}

impl<T> ZTArray<T> {
    pub fn len(&self) -> usize {
        ((self.end_ptr - self.start_ptr) / 4) as usize
    }

    pub fn capacity(&self) -> usize {
        ((self.buffer_end_ptr - self.start_ptr) / 4) as usize
    }

    pub fn get_ptr(&self, index: usize) -> u32 {
        get_from_memory(self.start_ptr + (index * 4) as u32)
    }

    pub fn get(&self, index: usize) -> T {
        get_from_memory::<T>(get_from_memory(self.start_ptr + (index * 4) as u32))
    }

    pub fn set(&self, index: usize, value: T) {
        save_to_memory(get_from_memory(self.start_ptr + (index * 4) as u32), value);
    }

    pub fn get_vec(&self) -> Vec<T> {
        let mut vec = Vec::new();
        for i in 0..self.len() {
            vec.push(self.get(i));
        }
        vec
    }
}

pub trait Checkable {
    fn check(ptr: u32) -> anyhow::Result<()>;
}
