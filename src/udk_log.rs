//! This module contains functionality relevant to UDK logging.
use crate::get_udk_slice;

/// Offset from the beginning of UDK64.exe to the debug log object.
const DEBUG_LOG_OFFSET: usize = 0x0355_1720;
const DEBUG_FN_OFFSET: usize = 0x0024_6A20;

type UDKLogFn = unsafe extern "C" fn(usize, u32, *const widestring::WideChar);

#[repr(u32)]
pub enum LogType {
    Init = 762,
    Warning = 767,
}

/// Log a message via the UDK logging framework.
pub fn log(typ: LogType, msg: &str) {
    let udk_slice = get_udk_slice();
    let log_obj = unsafe { udk_slice.as_ptr().add(DEBUG_LOG_OFFSET) };
    let log_fn: UDKLogFn = unsafe { std::mem::transmute(udk_slice.as_ptr().add(DEBUG_FN_OFFSET)) };

    let wmsg = widestring::WideCString::from_str(&msg).unwrap();

    unsafe {
        (log_fn)(log_obj as usize, typ as u32, wmsg.as_ptr());
    }
}
