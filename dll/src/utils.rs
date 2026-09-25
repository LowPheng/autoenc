use std::{
    ffi::OsString,
    os::{
        raw::c_char,
        windows::ffi::{OsStrExt, OsStringExt},
    },
    path::{Path, PathBuf},
};

use widestring::U16CString;
use windows_sys::Win32::{
    Foundation::MAX_PATH,
    Globalization::{
        CP_ACP, CP_OEMCP, MultiByteToWideChar, WC_NO_BEST_FIT_CHARS, WideCharToMultiByte,
    },
    Storage::FileSystem::AreFileApisANSI,
};

pub fn get_directory_wide(f: impl Fn(*mut u16, u32) -> u32) -> PathBuf {
    let mut buf = vec![0u16; MAX_PATH as usize];
    loop {
        let len = f(buf.as_mut_ptr(), buf.len() as u32) as usize;
        if len < buf.len() {
            buf.truncate(len);
            return PathBuf::from(OsString::from_wide(&buf));
        }
        buf.resize(len, 0);
    }
}

pub fn path_to_wide(path: impl AsRef<Path>) -> U16CString {
    U16CString::from_os_str(path.as_ref().as_os_str()).expect("Windows paths cannot contain NUL")
}

fn file_code_page() -> u32 {
    if unsafe { AreFileApisANSI() } != 0 {
        CP_ACP
    } else {
        CP_OEMCP
    }
}

pub unsafe fn fopen_str_to_path(ptr: *const c_char) -> Option<PathBuf> {
    let code_page = file_code_page();
    let len = unsafe { MultiByteToWideChar(code_page, 0, ptr.cast(), -1, std::ptr::null_mut(), 0) };
    if len == 0 {
        return None;
    }
    let mut buf = vec![0u16; len as usize];
    let written =
        unsafe { MultiByteToWideChar(code_page, 0, ptr.cast(), -1, buf.as_mut_ptr(), len) };
    if written == 0 {
        return None;
    }
    buf.truncate(written as usize - 1);
    Some(PathBuf::from(OsString::from_wide(&buf)))
}

pub fn path_to_fopen_str(path: impl AsRef<Path>) -> Option<Vec<c_char>> {
    let code_page = file_code_page();
    let wide = path
        .as_ref()
        .as_os_str()
        .encode_wide()
        .chain(Some(0))
        .collect::<Vec<_>>();
    let mut used_default = 0;
    let len = unsafe {
        WideCharToMultiByte(
            code_page,
            WC_NO_BEST_FIT_CHARS,
            wide.as_ptr(),
            wide.len() as i32,
            std::ptr::null_mut(),
            0,
            std::ptr::null(),
            &mut used_default,
        )
    };
    if len == 0 {
        return None;
    }
    let mut buf = vec![0; len as usize];
    let written = unsafe {
        WideCharToMultiByte(
            code_page,
            WC_NO_BEST_FIT_CHARS,
            wide.as_ptr(),
            wide.len() as i32,
            buf.as_mut_ptr().cast(),
            len,
            std::ptr::null(),
            &mut used_default,
        )
    };
    if written == 0 || used_default != 0 {
        return None;
    }
    buf.truncate(written as usize);
    Some(buf)
}

pub fn write_file(path: impl AsRef<Path>, data: impl AsRef<[u8]>) -> Result<(), std::io::Error> {
    if let Some(parent) = path.as_ref().parent() {
        std::fs::create_dir_all(parent)?;
    }
    std::fs::write(path, data)?;
    Ok(())
}
