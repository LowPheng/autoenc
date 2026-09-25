use std::ffi::c_void;

use pelite::pe::{Pe, PeView, imports::Import};
use windows_sys::Win32::System::Memory::{PAGE_READWRITE, VirtualProtect};

pub unsafe fn hook_iat(
    module_base: *const u8,
    dll_pat: impl Fn(&str) -> bool,
    name: &str,
    func: *const c_void,
) -> pelite::Result<Option<*const c_void>> {
    let entry_addr = {
        let view = unsafe { PeView::module(module_base) };
        view.imports()?
            .into_iter()
            .find_map(|desc| {
                (|| {
                    let imported_dll = desc.dll_name()?.to_str()?;
                    if !dll_pat(imported_dll) {
                        return Ok(None);
                    }

                    for (iat_entry, import) in desc.iat()?.zip(desc.int()?) {
                        if matches!(import?, Import::ByName { name: import_name, .. } if import_name == name) {
                            return Ok(Some((iat_entry as *const u32).addr()));
                        }
                    }
                    pelite::Result::Ok(None)
                })()
                .transpose()
            })
            .transpose()?
    };
    let Some(entry_addr) = entry_addr else {
        return Ok(None);
    };
    let entry = module_base.with_addr(entry_addr).cast_mut().cast::<u32>();

    let original = unsafe { std::ptr::read(entry) };

    let mut old_protect = 0;
    if unsafe {
        VirtualProtect(
            entry.cast(),
            size_of::<u32>(),
            PAGE_READWRITE,
            &mut old_protect,
        )
    } == 0
    {
        let error = std::io::Error::last_os_error();
        panic!("VirtualProtect Error: {error}");
    }

    unsafe {
        std::ptr::write(entry, func as usize as u32);
    }

    let mut unused = 0;
    if unsafe { VirtualProtect(entry.cast(), size_of::<u32>(), old_protect, &mut unused) } == 0 {
        let error = std::io::Error::last_os_error();
        panic!("VirtualProtect restore Error: {error}");
    }

    Ok(Some(original as usize as *const c_void))
}
