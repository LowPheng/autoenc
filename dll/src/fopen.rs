use std::{
    ffi::{CStr, CString, OsString},
    os::{
        raw::{c_char, c_int, c_void},
        windows::ffi::OsStringExt,
    },
    path::PathBuf,
    sync::OnceLock,
};

use widestring::{U16CStr, U16CString};
use windows_sys::Win32::{
    System::LibraryLoader::GetModuleHandleA,
    UI::WindowsAndMessaging::{MB_ICONERROR, MessageBoxW},
};

use crate::{
    hook::hook_iat,
    runtime::{FileRouting, Runtime},
    utils,
};

type FopenFn = unsafe extern "C" fn(*const c_char, *const c_char) -> *mut c_void;
type WfopenFn = unsafe extern "C" fn(*const u16, *const u16) -> *mut c_void;
type WfsopenFn = unsafe extern "C" fn(*const u16, *const u16, c_int) -> *mut c_void;

static ORIGINAL_FOPEN: OnceLock<FopenFn> = OnceLock::new();
static ORIGINAL_WFOPEN: OnceLock<WfopenFn> = OnceLock::new();
static ORIGINAL_WFSOPEN: OnceLock<WfsopenFn> = OnceLock::new();

static RUNTIME: OnceLock<Option<Runtime>> = OnceLock::new();

fn runtime() -> Option<&'static Runtime> {
    RUNTIME
        .get_or_init(|| {
            Runtime::initialize()
                .inspect_err(|error| unsafe {
                    let msg = U16CString::from_str_truncate(format!("{error:#}"));
                    MessageBoxW(
                        std::ptr::null_mut(),
                        msg.as_ptr(),
                        windows_sys::w!("Failed to initialize Autoenc"),
                        MB_ICONERROR,
                    );
                })
                .unwrap_or_default()
        })
        .as_ref()
}

fn call_routed_fopen(path: PathBuf, mode: &str, shflag: Option<c_int>) -> Option<*mut c_void> {
    if let Some((original, shflag)) = ORIGINAL_WFSOPEN.get().zip(shflag) {
        let path = utils::path_to_wide(path);
        let mode = U16CString::from_str(mode).expect("Invalid mode");
        Some(unsafe { original(path.as_ptr(), mode.as_ptr(), shflag) })
    } else if let Some(original) = ORIGINAL_WFOPEN.get() {
        let path = utils::path_to_wide(path);
        let mode = U16CString::from_str(mode).expect("Invalid mode");
        Some(unsafe { original(path.as_ptr(), mode.as_ptr()) })
    } else {
        let original = ORIGINAL_FOPEN.get().unwrap();
        let path = utils::path_to_fopen_str(path)?;
        let mode = CString::new(mode).expect("Invalid mode");
        Some(unsafe { original(path.as_ptr(), mode.as_ptr()) })
    }
}

unsafe extern "C" fn hooked_fopen(file: *const c_char, mode: *const c_char) -> *mut c_void {
    let original = ORIGINAL_FOPEN.get().unwrap();
    let Some(runtime) = runtime() else {
        unsafe {
            return original(file, mode);
        }
    };

    let Some(file_path) = (unsafe { utils::fopen_str_to_path(file) }) else {
        unsafe {
            return original(file, mode);
        }
    };
    let mode_str = unsafe { CStr::from_ptr(mode) }.to_string_lossy();
    match runtime.handle_file(file_path, &mode_str) {
        Ok(FileRouting::Original) => unsafe { original(file, mode) },
        Ok(FileRouting::Route(path, new_mode)) => {
            call_routed_fopen(path, new_mode, None).expect("path_to_fopen_str error")
        }
        Err(error) => unsafe {
            let msg = U16CString::from_str_truncate(format!("{error:#}"));
            MessageBoxW(
                std::ptr::null_mut(),
                msg.as_ptr(),
                windows_sys::w!("Autoenc Error"),
                MB_ICONERROR,
            );
            original(file, mode)
        },
    }
}

unsafe extern "C" fn hooked_wfopen(file: *const u16, mode: *const u16) -> *mut c_void {
    let original = ORIGINAL_WFOPEN.get().unwrap();
    let Some(runtime) = runtime() else {
        unsafe {
            return original(file, mode);
        }
    };

    let file_path =
        unsafe { PathBuf::from(OsString::from_wide(U16CStr::from_ptr_str(file).as_slice())) };
    let mode_str = unsafe { U16CStr::from_ptr_str(mode) }.to_string_lossy();
    match runtime.handle_file(file_path, &mode_str) {
        Ok(FileRouting::Original) => unsafe { original(file, mode) },
        Ok(FileRouting::Route(path, mode)) => call_routed_fopen(path, mode, None).unwrap(),
        Err(error) => unsafe {
            let msg = U16CString::from_str_truncate(format!("{error:#}"));
            MessageBoxW(
                std::ptr::null_mut(),
                msg.as_ptr(),
                windows_sys::w!("Autoenc Error"),
                MB_ICONERROR,
            );
            original(file, mode)
        },
    }
}

unsafe extern "C" fn hooked_wfsopen(
    file: *const u16,
    mode: *const u16,
    shflag: c_int,
) -> *mut c_void {
    let original = ORIGINAL_WFSOPEN.get().unwrap();
    let Some(runtime) = runtime() else {
        unsafe {
            return original(file, mode, shflag);
        }
    };

    let file_path =
        unsafe { PathBuf::from(OsString::from_wide(U16CStr::from_ptr_str(file).as_slice())) };
    let mode_str = unsafe { U16CStr::from_ptr_str(mode) }.to_string_lossy();
    match runtime.handle_file(file_path, &mode_str) {
        Ok(FileRouting::Original) => unsafe { original(file, mode, shflag) },
        Ok(FileRouting::Route(path, mode)) => call_routed_fopen(path, mode, Some(shflag)).unwrap(),
        Err(error) => unsafe {
            let msg = U16CString::from_str_truncate(format!("{error:#}"));
            MessageBoxW(
                std::ptr::null_mut(),
                msg.as_ptr(),
                windows_sys::w!("Autoenc Error"),
                MB_ICONERROR,
            );
            original(file, mode, shflag)
        },
    }
}

pub fn install_fopen_hooks() {
    let module = unsafe { GetModuleHandleA(std::ptr::null()) } as *const u8;
    if module.is_null() {
        let error = std::io::Error::last_os_error();
        panic!("GetModuleHandleA Error: {error}");
    }

    let dll_pat = |name: &str| {
        let name = name.to_ascii_uppercase();
        name.starts_with("MSVCR") && !name.starts_with("MSVCRT")
    };
    unsafe {
        if let Some(original) = hook_iat(module, dll_pat, "fopen", hooked_fopen as *const c_void)
            .expect("Failed to hook `fopen`")
        {
            _ = ORIGINAL_FOPEN.set(std::mem::transmute::<*const c_void, FopenFn>(original));
        }
        if let Some(original) = hook_iat(module, dll_pat, "_wfopen", hooked_wfopen as *const c_void)
            .expect("Failed to hook `_wfopen`")
        {
            _ = ORIGINAL_WFOPEN.set(std::mem::transmute::<*const c_void, WfopenFn>(original));
        }
        if let Some(original) =
            hook_iat(module, dll_pat, "_wfsopen", hooked_wfsopen as *const c_void)
                .expect("Failed to hook `_wfsopen`")
        {
            _ = ORIGINAL_WFSOPEN.set(std::mem::transmute::<*const c_void, WfsopenFn>(original));
        }
    }
}
