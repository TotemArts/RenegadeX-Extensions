//! This module handles functionality related to the original dinput8.dll
use lazy_static::lazy_static;
use libloading::Library;

lazy_static! {
    static ref DINPUT8_DLL: Library = unsafe { Library::new("C:\\Windows\\System32\\dinput8.dll") }.unwrap();
    static ref DINPUT8CREATE_PTR: libloading::Symbol<'static, unsafe extern "C" fn(usize, u32, usize, usize, usize) -> u32> =
        unsafe { DINPUT8_DLL.get(b"DirectInput8Create\0") }.unwrap();
}

/// This function simply redirects the one and only DirectInput8Create call to the real dinput8 DLL.
#[no_mangle]
#[export_name = "DirectInput8Create"]
pub unsafe extern "C" fn directinput8_create(
    hinst: usize,
    dwversion: u32,
    riid: usize,
    out: usize,
    outer: usize,
) -> u32 {
    (DINPUT8CREATE_PTR)(hinst, dwversion, riid, out, outer)
}
