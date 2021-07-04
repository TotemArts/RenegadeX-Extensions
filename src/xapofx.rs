//! This module handles functionality related to the original XAPOFX1_5.dll
use lazy_static::lazy_static;
use libloading::Library;

lazy_static! {
    static ref XAPO_DLL: Library = unsafe { Library::new("xapofx1_5.dll") }.unwrap();
    static ref CREATEFX_PTR: libloading::Symbol<'static, unsafe extern "C" fn(usize, usize, usize, u32) -> u32> =
        unsafe { XAPO_DLL.get(b"CreateFX\0") }.unwrap();
}

/// This function simply redirects the one and only CreateFX call to the real xapofx DLL.
#[no_mangle]
pub unsafe extern "C" fn CreateFX(
    clsid: usize,
    effect_out_ptr: usize,
    init_data: usize,
    init_size: u32,
) -> u32 {
    (CREATEFX_PTR)(clsid, effect_out_ptr, init_data, init_size)
}
