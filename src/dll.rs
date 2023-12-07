use std::os::windows::fs::FileExt;
use std::{ops::Range, fs::File};
use std::sync::OnceLock;

use sha2::{Digest, Sha256};

use windows::{
    Win32::{
        Foundation::{HANDLE, HINSTANCE},
        System::{
            LibraryLoader::GetModuleHandleA,
            ProcessStatus::{K32GetModuleInformation, MODULEINFO},
            SystemServices::{
                DLL_PROCESS_ATTACH, DLL_PROCESS_DETACH, DLL_THREAD_ATTACH, DLL_THREAD_DETACH
            },
            Threading::GetCurrentProcess,
        },
    },
    core::Error,
};

pub fn dll_main(_hinst_dll: HINSTANCE, fdw_reason: u32, _lpv_reserved: usize) -> i32 {
    match fdw_reason {
        DLL_PROCESS_ATTACH => {
            dll_attach()
        }
        DLL_PROCESS_DETACH => {}

        DLL_THREAD_ATTACH => {}
        DLL_THREAD_DETACH => {}

        _ => return 0,
    }

    1
}

/// Called upon DLL attach. This function verifies the UDK and initializes
/// hooks if the UDK matches our known hash.
fn dll_attach() {
    let process = unsafe { GetCurrentProcess() };
    let module: windows::Win32::Foundation::HMODULE = unsafe { GetModuleHandleA(None) }.expect("Couldn't get Module Handle for the UDK process");

    let exe_information = get_module_information(process, module.into()).expect("Failed to get module information for UDK");
    let udk_range = Range {
        start: exe_information.lpBaseOfDll as usize,
        end: exe_information.lpBaseOfDll as usize + exe_information.SizeOfImage as usize,
    };

    // Now that we're attached, let's hash the UDK executable.
    // If the hash does not match what we think it should be, do not attach detours.
    let exe_filename = std::env::current_exe().unwrap();

	let filemap = pelite::FileMap::open(&exe_filename).unwrap();
	let pefile = pelite::PeFile::from_bytes(&filemap).unwrap();
	let section = pefile.section_headers().by_name(".text").unwrap();
    let range = section.file_range();

    let f = File::open(exe_filename).unwrap();
    let mut buf = vec![0; (range.end - range.start) as usize];
    f.seek_read(&mut buf, range.start as u64).unwrap();

    let hash = {
        let mut sha = Sha256::new();
        sha.update(&buf);
        sha.finalize()
    };

    // Ensure the hash matches a known hash.
    if hash[..] != UDK_KNOWN_HASH {
        panic!("Unknown UDK hash");
    }

    // Cache the UDK slice.
    UDK_RANGE.set(udk_range).unwrap();
}

#[cfg(target_arch = "x86_64")]
const UDK_KNOWN_HASH: [u8; 32] = [
    0xF0, 0x2F, 0x13, 0x1E, 0xF2, 0xE, 0xA3, 0xCE, 0xD1, 0xCE, 0x93, 0x14, 0x53, 0xDE, 0x37, 0xB9,
    0x51, 0x1B, 0x92, 0xD0, 0xBA, 0x7C, 0x7, 0x27, 0x5B, 0xA0, 0xAE, 0xFB, 0x7D, 0xFB, 0xE3, 0xE3
];

#[cfg(target_arch = "x86")]
const UDK_KNOWN_HASH: [u8; 32] = [
    0x70, 0xC2, 0x91, 0x73, 0xE0, 0x0F, 0x2F, 0xCA, 0x5E, 0xBB, 0x92, 0x76, 0x00, 0x43, 0xDF, 0x70,
    0xE0, 0xC0, 0x16, 0xFA, 0xB2, 0x80, 0xF8, 0x20, 0x88, 0x31, 0xD9, 0x99, 0xFE, 0xF0, 0xFF, 0x33
];

/// Cached memory range for UDK.exe
pub static UDK_RANGE: OnceLock<Range<usize>> = OnceLock::new();

/// Return the base pointer for UDK.exe
pub fn get_udk_ptr() -> *const u8 {
    let range = UDK_RANGE.get().unwrap();

    // TODO: Once Rust gets better raw slice support, we should return a `*const [u8]` instead.
    range.start as *const u8
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
        false => Err(Error::from_win32()),
    }
}