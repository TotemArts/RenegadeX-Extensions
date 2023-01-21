//! This module contains functionality relevant to UDK logging.
use widestring::WideCStr;

use crate::get_udk_slice;

use crate::udk_offsets::{DEBUG_FN_OFFSET, DEBUG_LOG_OFFSET};

/// This is the type signature of UDK's log function.
type UDKLogFn = unsafe extern "C" fn(usize, u32, *const widestring::WideChar);

/// This enum represents the UDK message types.
#[repr(u32)]
pub enum LogType {
    Init = 762,
    Warning = 767,
}

pub fn log_wide(typ: LogType, wmsg: &WideCStr) {
    let udk_slice = get_udk_slice();
    let log_obj = unsafe { udk_slice.as_ptr().add(DEBUG_LOG_OFFSET) };
    let log_fn: UDKLogFn = unsafe { std::mem::transmute(udk_slice.as_ptr().add(DEBUG_FN_OFFSET)) };

    unsafe {
        (log_fn)(log_obj as usize, typ as u32, wmsg.as_ptr());
    }
}

/// Log a message via the UDK logging framework.
pub fn log(typ: LogType, msg: &str) {
    // Convert the UTF-8 Rust string into an OS wide string.
    let wmsg = widestring::WideCString::from_str(&msg).unwrap();

    log_wide(typ, &wmsg)
}
