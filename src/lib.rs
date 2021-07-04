#![feature(asm, naked_functions)]

mod xapofx;

mod udk_xaudio;

use sha2::{Digest, Sha256};

use winbindings::Windows::Win32::{
    Foundation::{HANDLE, HINSTANCE},
    System::{
        LibraryLoader::GetModuleHandleA,
        ProcessStatus::{K32GetModuleInformation, MODULEINFO},
        SystemServices::{
            DLL_PROCESS_ATTACH, DLL_PROCESS_DETACH, DLL_THREAD_ATTACH, DLL_THREAD_DETACH,
        },
        Threading::GetCurrentProcess,
    },
};

fn get_module_information(process: HANDLE, module: HINSTANCE) -> Result<MODULEINFO, ()> {
    let mut module_info = MODULEINFO {
        ..Default::default()
    };

    match unsafe {
        K32GetModuleInformation(
            process,
            module,
            &mut module_info,
            std::mem::size_of::<MODULEINFO>() as u32,
        )
        .as_bool()
    } {
        true => Ok(module_info),
        false => Err(()),
    }
}

/// Get a (read-only) memory slice corresponding to the loaded EXE.
fn get_process_slice() -> &'static [u8] {
    let module = unsafe { GetModuleHandleA(None) };
    match get_module_information(unsafe { GetCurrentProcess() }, module) {
        Ok(mi) => unsafe {
            std::slice::from_raw_parts(mi.lpBaseOfDll as *const u8, mi.SizeOfImage as usize)
        },
        Err(_) => panic!("Failed to get module information for UDK EXE"),
    }
}

#[no_mangle]
pub extern "stdcall" fn DllMain(
    _hinst_dll: HINSTANCE,
    fdw_reason: u32,
    _lpv_reserved: usize,
) -> i32 {
    match fdw_reason {
        DLL_PROCESS_ATTACH => {
            // Now that we're attached, let's hash the UDK executable.
            // If the hash does not match what we think it should be, do not attach detours.
            let exe = get_process_slice();

            let hash = {
                let mut sha = Sha256::new();
                sha.update(exe);
                sha.finalize()
            };
        }
        DLL_PROCESS_DETACH => {}

        DLL_THREAD_ATTACH => {}
        DLL_THREAD_DETACH => {}

        _ => return 0,
    }

    return 1;
}
