//! This file contains functionality and constants related to tracking offsets in UDK binaries.

// Logging offsets.
/// Offset from the beginning of UDK64.exe to the debug log object.
pub const DEBUG_LOG_OFFSET: usize = 0x0355_1720;
/// Address of UDK's log function.
pub const DEBUG_FN_OFFSET: usize = 0x0024_6A20;

// XAudio2 offsets.
// pub const UDK_INITHW_OFFSET: usize = 0x0171_1ED0;
// pub const UDK_XAUDIO2_OFFSET: usize = 0x036C_90F8;
pub const UDK_XAUDIO2CREATE_OFFSET: usize = 0x0170_F4D0;
