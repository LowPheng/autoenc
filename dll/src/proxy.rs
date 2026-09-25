use std::{ffi::CStr, os::raw::c_void, sync::OnceLock};

use windows_sys::{
    Win32::{
        Foundation::HMODULE,
        System::{
            LibraryLoader::{GetProcAddress, LoadLibraryW},
            SystemInformation::GetSystemDirectoryW,
        },
    },
    core::{GUID, HRESULT},
};

use crate::utils;

static DSOUND: OnceLock<usize> = OnceLock::new();

fn dsound() -> HMODULE {
    *DSOUND.get_or_init(|| {
        let path = utils::get_directory_wide(|ptr, len| unsafe { GetSystemDirectoryW(ptr, len) });
        let dll = path.join("dsound.dll");
        let dll = utils::path_to_wide(dll);
        let module = unsafe { LoadLibraryW(dll.as_ptr()) };
        if module.is_null() {
            let error = std::io::Error::last_os_error();
            panic!("LoadLibraryW Error: {error}");
        }
        module as usize
    }) as HMODULE
}

static EXPORTS: OnceLock<ProxyExports> = OnceLock::new();

fn exports() -> &'static ProxyExports {
    EXPORTS.get_or_init(ProxyExports::init)
}

macro_rules! c_stringify {
    ($($t:tt)*) => {
        CStr::from_bytes_with_nul_unchecked(
            concat!(stringify!($($t)*), "\0").as_bytes()
        )
    };
}

macro_rules! proxy_exports {
    ($($func:ident($($param:ident: $paramtype:ty),*) -> $ret:ty;)+) => {
        #[allow(nonstandard_style)]
        struct ProxyExports {
            $(
                $func: unsafe extern "system" fn($($param: $paramtype),*) -> $ret
            ),*
        }

        unsafe impl Send for ProxyExports {}
        unsafe impl Sync for ProxyExports {}

        impl ProxyExports {
            #[allow(clippy::missing_transmute_annotations)]
            pub fn init() -> Self {
                let module = dsound();
                Self {
                    $(
                        $func: {
                            unsafe {
                                let proc = GetProcAddress(
                                    module,
                                    c_stringify!($func).as_ptr() as *const u8
                                )
                                .expect(concat!("Function not found: ", stringify!($func)));
                                std::mem::transmute(proc)
                            }
                        }
                    ),*
                }
            }
        }

        $(
            #[unsafe(no_mangle)]
            unsafe extern "system" fn $func($($param: $paramtype),*) -> $ret {
                unsafe { (exports().$func)($($param),*) }
            }
        )*
    };
}

proxy_exports! {
    DirectSoundCreate(
        guid: *const GUID,
        out: *mut *mut c_void,
        outer: *mut c_void
    ) -> HRESULT;

    DirectSoundEnumerateA(
        callback: Option<
            unsafe extern "system" fn(
                guid: *mut GUID,
                description: *const u8,
                module: *const u8,
                context: *mut c_void,
            ) -> i32
        >,
        context: *mut c_void
    ) -> HRESULT;

    DirectSoundEnumerateW(
        callback: Option<
            unsafe extern "system" fn(
                guid: *mut GUID,
                description: *const u16,
                module: *const u16,
                context: *mut c_void,
            ) -> i32
        >,
        context: *mut c_void
    ) -> HRESULT;

    DllCanUnloadNow() -> HRESULT;

    DllGetClassObject(
        clsid: *const GUID,
        iid: *const GUID,
        out: *mut *mut c_void
    ) -> HRESULT;

    DirectSoundCaptureCreate(
        guid: *const GUID,
        out: *mut *mut c_void,
        outer: *mut c_void
    ) -> HRESULT;

    DirectSoundCaptureEnumerateA(
        callback: Option<
            unsafe extern "system" fn(
                guid: *mut GUID,
                description: *const u8,
                module: *const u8,
                context: *mut c_void,
            ) -> i32
        >,
        context: *mut c_void
    ) -> HRESULT;

    DirectSoundCaptureEnumerateW(
        callback: Option<
            unsafe extern "system" fn(
                guid: *mut GUID,
                description: *const u16,
                module: *const u16,
                context: *mut c_void,
            ) -> i32
        >,
        context: *mut c_void
    ) -> HRESULT;

    GetDeviceID(
        src: *const GUID,
        dest: *mut GUID
    ) -> HRESULT;

    DirectSoundFullDuplexCreate(
        capture_device: *const GUID,
        render_device: *const GUID,
        capture_buffer_desc: *const c_void,
        render_buffer_desc: *const c_void,
        hwnd: *mut c_void,
        level: u32,
        full_duplex: *mut *mut c_void,
        capture_buffer: *mut *mut c_void,
        render_buffer: *mut *mut c_void,
        outer: *mut c_void
    ) -> HRESULT;

    DirectSoundCreate8(
        guid: *const GUID,
        out: *mut *mut c_void,
        outer: *mut c_void
    ) -> HRESULT;

    DirectSoundCaptureCreate8(
        guid: *const GUID,
        out: *mut *mut c_void,
        outer: *mut c_void
    ) -> HRESULT;
}
