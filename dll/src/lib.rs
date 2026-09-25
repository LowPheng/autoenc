use std::os::raw::c_void;

use widestring::U16CString;
use windows_sys::Win32::{
    Foundation::HINSTANCE,
    System::{LibraryLoader::DisableThreadLibraryCalls, SystemServices::DLL_PROCESS_ATTACH},
    UI::WindowsAndMessaging::{MB_ICONERROR, MessageBoxW},
};

pub(crate) mod cache;
pub(crate) mod config;
pub(crate) mod hook;
pub(crate) mod logging;
pub(crate) mod runtime;
pub(crate) mod transform;
pub(crate) mod utils;

mod fopen;
mod proxy;

#[unsafe(no_mangle)]
unsafe extern "system" fn DllMain(
    hinst_dll: HINSTANCE,
    fdw_reason: u32,
    _lpv_reserved: *mut c_void,
) -> bool {
    if fdw_reason == DLL_PROCESS_ATTACH {
        unsafe {
            DisableThreadLibraryCalls(hinst_dll);
        }
        std::panic::set_hook(Box::new(|panic| {
            let msg = U16CString::from_str_truncate(
                panic.payload_as_str().unwrap_or("unknown panic payload"),
            );
            unsafe {
                MessageBoxW(
                    std::ptr::null_mut(),
                    msg.as_ptr(),
                    windows_sys::w!("Autoenc Panic"),
                    MB_ICONERROR,
                );
            }
        }));

        fopen::install_fopen_hooks();
    }
    true
}
