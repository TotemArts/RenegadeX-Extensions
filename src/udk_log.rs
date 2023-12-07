//! This module contains functionality relevant to UDK logging.
use crate::dll::get_udk_ptr;

/// Offset from the beginning of UDK64.exe to the debug log object.
#[cfg(target_arch = "x86_64")]
const DEBUG_LOG_OFFSET: usize = 0x0355_1720;
/// Address of UDK's log function.
#[cfg(target_arch = "x86_64")]
const DEBUG_FN_OFFSET: usize = 0x0024_6A20;

/// Offset from the beginning of UDK64.exe to the debug log object.
#[cfg(target_arch = "x86")]
const DEBUG_LOG_OFFSET: usize = 0x029a_31a8;
/// Address of UDK's log function.
#[cfg(target_arch = "x86")]
const DEBUG_FN_OFFSET: usize = 0x0002_1c500;

/// This is the type signature of UDK's log function.
type UDKLogFn = unsafe extern "C" fn(usize, u32, *const widestring::WideChar);

/// This enum represents the UDK message types.
#[repr(u32)]
pub enum LogType {
    Init = 0x2fa,
    //Debug = 0x36c,
    //Log = 0x2f8,
    Warning = 0x2ff,
    //Error = 0x315,
    //Critical = 0x2f9,
}

/// Log a message via the UDK logging framework.
pub fn log(typ: LogType, msg: &str) {
    let udk_ptr = get_udk_ptr();
    let log_obj = unsafe { udk_ptr.add(DEBUG_LOG_OFFSET) };
    let log_fn: UDKLogFn = unsafe { std::mem::transmute(udk_ptr.add(DEBUG_FN_OFFSET)) };

    // Convert the UTF-8 Rust string into an OS wide string.
    let wmsg: widestring::U16CString = widestring::WideCString::from_str(format!("TotemArts Extensions: {}", msg)).unwrap();

    unsafe {
        (log_fn)(log_obj as usize, typ as u32, wmsg.as_ptr());
    }
}
