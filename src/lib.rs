#![feature(asm, naked_functions)]

mod dinput8;

mod udk_log;
mod udk_xaudio;

const UDK_KNOWN_HASH: [u8; 32] = [
    0x5a, 0xd9, 0xff, 0xb3, 0x34, 0x6e, 0xfa, 0xba, 0xf1, 0x05, 0x8d, 0x8c, 0x09, 0x7d, 0x30, 0x5c,
    0xec, 0xaa, 0x55, 0x62, 0xf9, 0x28, 0x9d, 0x79, 0x91, 0x6d, 0x5f, 0xea, 0xa7, 0x6a, 0xed, 0x0a,
];

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

static mut UDK_SLICE: Option<&'static [u8]> = None;

pub fn get_udk_slice() -> &'static [u8] {
    // SAFETY: This is only touched once in DllMain.
    unsafe { UDK_SLICE.unwrap() }
}

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

fn dll_attach() -> anyhow::Result<()> {
    // Now that we're attached, let's hash the UDK executable.
    // If the hash does not match what we think it should be, do not attach detours.
    let exe = get_process_slice();

    let hash = {
        let mut sha = Sha256::new();
        sha.update(&exe[..256]);
        sha.finalize()
    };

    // Ensure the hash matches a known hash.
    if hash[..] != UDK_KNOWN_HASH {
        anyhow::bail!("Unknown UDK hash");
    }

    // Cache the UDK slice.
    unsafe {
        UDK_SLICE = Some(exe);
    }

    // Initialize detours.
    udk_xaudio::init()?;

    Ok(())
}

#[no_mangle]
pub extern "stdcall" fn DllMain(
    _hinst_dll: HINSTANCE,
    fdw_reason: u32,
    _lpv_reserved: usize,
) -> i32 {
    match fdw_reason {
        DLL_PROCESS_ATTACH => {
            dll_attach().unwrap();
        }
        DLL_PROCESS_DETACH => {}

        DLL_THREAD_ATTACH => {}
        DLL_THREAD_DETACH => {}

        _ => return 0,
    }

    return 1;
}
