//! This module contains functionality relevant to UDK logging.
use crate::get_udk_slice;

use crate::udk_offsets::{DEBUG_LOG_OFFSET, DEBUG_FN_OFFSET};

/// This is the type signature of UDK's log function.
type UDKLogFn = unsafe extern "C" fn(usize, u32, *const widestring::WideChar);

/// This enum represents the UDK message types.
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

    // Convert the UTF-8 Rust string into an OS wide string.
    let wmsg = widestring::WideCString::from_str(&msg).unwrap();

    unsafe {
        (log_fn)(log_obj as usize, typ as u32, wmsg.as_ptr());
    }
}
