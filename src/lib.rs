mod dinput8;
#[allow(non_camel_case_types)]
#[allow(non_snake_case)]
mod xaudio27;

mod udk_log;
mod udk_offsets;
mod udk_xaudio;

#[cfg(target_arch = "x86_64")]
const UDK_KNOWN_HASH: [u8; 32] = [
    0x0D, 0xE6, 0x90, 0x31, 0xEA, 0x41, 0x01, 0xF2, 0x18, 0xB6, 0x61, 0x27, 0xFD, 0x14, 0x3A, 0x8E,
    0xC3, 0xF7, 0x48, 0x3E, 0x31, 0x9C, 0x3D, 0x8D, 0xD5, 0x1F, 0xA2, 0x8D, 0x7C, 0xBF, 0x08, 0xF5,
];

#[cfg(target_arch = "x86")]
const UDK_KNOWN_HASH: [u8; 32] = [
    0xEF, 0xAF, 0xBA, 0x91, 0xD3, 0x05, 0x2D, 0x07, 0x07, 0xDD, 0xF2, 0xF2, 0x14, 0x15, 0x00, 0xFA,
    0x6C, 0x1E, 0x8F, 0x9E, 0xF0, 0x70, 0x40, 0xB8, 0xF9, 0x96, 0x73, 0x8A, 0x00, 0xFB, 0x90, 0x07,
];

use anyhow::Context;
use sha2::{Digest, Sha256};

use windows::core::PCWSTR;
use windows::Win32::System::Com::{CoInitializeEx, COINIT_MULTITHREADED};
use windows::Win32::{
    Foundation::{HANDLE, HINSTANCE},
    System::{
        Diagnostics::Debug::OutputDebugStringW,
        LibraryLoader::GetModuleHandleA,
        ProcessStatus::{K32GetModuleFileNameExW, K32GetModuleInformation, MODULEINFO},
        SystemServices::{
            DLL_PROCESS_ATTACH, DLL_PROCESS_DETACH, DLL_THREAD_ATTACH, DLL_THREAD_DETACH,
        },
        Threading::GetCurrentProcess,
    },
};

/// Cached slice of UDK.exe. This is only touched once upon init, and
/// never written again.
// FIXME: The slice is actually unsafe to access; sections of memory may be unmapped!
// We should use a raw pointer slice instead (if ergonomics permit doing so).
static mut UDK_SLICE: Option<&'static [u8]> = None;

/// Return a slice of UDK.exe
pub fn get_udk_slice() -> &'static [u8] {
    // SAFETY: This is only touched once in DllMain.
    unsafe { UDK_SLICE.unwrap() }
}

/// Wrapped version of the Win32 OutputDebugString.
fn output_debug_string(s: &str) {
    let ws = widestring::WideCString::from_str(s).unwrap();

    unsafe {
        OutputDebugStringW(PCWSTR(ws.as_ptr()));
    }
}

/// Wrapped version of the Win32 GetModuleFileName.
fn get_module_filename(process: HANDLE, module: HINSTANCE) -> windows::core::Result<String> {
    // Use a temporary buffer the size of MAX_PATH for now.
    // TODO: Dynamic allocation for longer filenames. As of now, this will truncate longer filenames.
    let mut buf = [0u16; 256];

    let len = unsafe { K32GetModuleFileNameExW(process, module, &mut buf) } as usize;

    if len == 0 {
        // Function failed.
        return Err(windows::core::Error::from_win32());
    }

    Ok(String::from_utf16_lossy(&buf[..len]))
}

/// Wrapped version of the Win32 GetModuleInformation.
fn get_module_information(process: HANDLE, module: HINSTANCE) -> windows::core::Result<MODULEINFO> {
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
        false => Err(windows::core::Error::from_win32()),
    }
}

/// Create a raw slice from a MODULEINFO structure.
fn get_module_slice(info: &MODULEINFO) -> *const [u8] {
    core::ptr::slice_from_raw_parts(info.lpBaseOfDll as *const u8, info.SizeOfImage as usize)
}

/// Called upon DLL attach. This function verifies the UDK and initializes
/// hooks if the UDK matches our known hash.
fn dll_attach() -> anyhow::Result<()> {
    // Ensure that COM is initialized.
    unsafe { CoInitializeEx(None, COINIT_MULTITHREADED) }.context("failed to initialize COM")?;

    let process = unsafe { GetCurrentProcess() };
    let module = unsafe { GetModuleHandleA(None) }.context("failed to get module handle")?;

    let exe_slice = get_module_slice(
        &get_module_information(process, module.into()).expect("Failed to get module information for UDK"),
    );

    // Now that we're attached, let's hash the UDK executable.
    // If the hash does not match what we think it should be, do not attach detours.
    let exe_filename = get_module_filename(process, module.into())?;

    let mut exe = std::fs::File::open(exe_filename)?;
    let hash = {
        let mut sha = Sha256::new();
        std::io::copy(&mut exe, &mut sha)?;
        sha.finalize()
    };

    // Ensure the hash matches a known hash.
    if hash[..] != UDK_KNOWN_HASH {
        output_debug_string(&format!("Hash: {:02X?}\n", hash));
        output_debug_string(&format!("Expected: {:02X?}\n", UDK_KNOWN_HASH));
        anyhow::bail!("Unknown UDK hash");
    }

    // Cache the UDK slice.
    unsafe {
        UDK_SLICE = Some(exe_slice.as_ref().unwrap());
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
            if let Err(e) = dll_attach() {
                // Print a debug message for anyone who's listening.
                output_debug_string(&format!("{:?}\n", e));
                eprintln!("{:?}", e);
            }
        }
        DLL_PROCESS_DETACH => {}

        DLL_THREAD_ATTACH => {}
        DLL_THREAD_DETACH => {}

        _ => return 0,
    }

    return 1;
}
