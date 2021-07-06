//! This module contains functionality related to UDK XAudio hooks.
use anyhow::Context;
use detour::static_detour;
use widestring::WideChar;

use crate::get_udk_slice;
use crate::udk_log::{log, LogType};

use winbindings::Windows::Win32::Foundation::BOOL;
use winbindings::Windows::Win32::Media::{Audio::XAudio2, Multimedia::WAVEFORMATEXTENSIBLE};
use winbindings::HRESULT;

// const UDK_INITHW_OFFSET: usize = 0x0171_1ED0;
// const UDK_XAUDIO2_OFFSET: usize = 0x036C_90F8;
const UDK_XAUDIO2CREATE_OFFSET: usize = 0x0170_F4D0;

static_detour! {
    static XAudio2CreateHook: extern "C" fn(*mut *mut IXAudio27, u32, u32) -> HRESULT;
}

#[repr(C)]
#[allow(non_snake_case)]
struct XAudio27DeviceDetails {
    DeviceID: [WideChar; 256],
    DisplayName: [WideChar; 256],
    Role: u32,
    OutputFormat: WAVEFORMATEXTENSIBLE,
}

/// Represents the VTable for XAudio 2.7's IXAudio2EngineCallback object.
///
/// Do _NOT_ alter the ordering of these fields.
#[repr(C)]
#[allow(non_snake_case)]
struct XAudio27CallbacksVtable {
    OnProcessingPassStart: Option<extern "C" fn(*mut XAudio27Callbacks)>,
    OnProcessingPassEnd: Option<extern "C" fn(*mut XAudio27Callbacks)>,
    OnCriticalError: Option<extern "C" fn(*mut XAudio27Callbacks, HRESULT)>,
}

#[repr(C)]
struct XAudio27Callbacks {
    vtable: *const XAudio27CallbacksVtable,
}

impl XAudio27Callbacks {
    pub fn new() -> Self {
        Self {
            vtable: &XAUDIO27CALLBACKS_VTABLE,
        }
    }

    extern "C" fn on_processing_pass_start(_this: *mut Self) {}
    extern "C" fn on_processing_pass_end(_this: *mut Self) {}

    extern "C" fn on_critical_error(_this: *mut Self, err: HRESULT) {
        log(
            LogType::Warning,
            &format!(
                "XAudio2.7 indicated a critical error: {} ({:08X})",
                err.message(),
                err.0
            ),
        );
    }
}

static XAUDIO27CALLBACKS_VTABLE: XAudio27CallbacksVtable = XAudio27CallbacksVtable {
    OnProcessingPassStart: Some(XAudio27Callbacks::on_processing_pass_start),
    OnProcessingPassEnd: Some(XAudio27Callbacks::on_processing_pass_end),

    OnCriticalError: Some(XAudio27Callbacks::on_critical_error),
};

/// Represents the VTable for XAudio 2.7's IXAudio2 object.
///
/// Do _NOT_ alter the ordering of these fields.
#[repr(C)]
#[allow(non_snake_case)]
struct IXAudio27Vtable {
    QueryInterface: Option<extern "C" fn() -> HRESULT>,
    AddRef: Option<extern "C" fn(*mut IXAudio27) -> HRESULT>,
    Release: Option<extern "C" fn(*mut IXAudio27) -> HRESULT>,
    GetDeviceCount: Option<extern "C" fn(*mut IXAudio27, *mut u32) -> HRESULT>,
    GetDeviceDetails:
        Option<extern "C" fn(*mut IXAudio27, u32, *mut XAudio27DeviceDetails) -> HRESULT>,
    Initialize: Option<extern "C" fn(*mut IXAudio27, u32, u32) -> HRESULT>,
    RegisterForCallbacks: Option<extern "C" fn(*mut IXAudio27, *mut XAudio27Callbacks) -> HRESULT>,
    UnregisterForCallbacks:
        Option<extern "C" fn(*mut IXAudio27, *mut XAudio27Callbacks) -> HRESULT>,

    CreateSourceVoice: Option<
        extern "C" fn(*mut IXAudio27, *mut usize, usize, u32, f32, usize, usize, usize) -> HRESULT,
    >,
    CreateSubmixVoice: Option<
        extern "C" fn(*mut IXAudio27, *mut usize, u32, u32, u32, u32, usize, usize) -> HRESULT,
    >,
    CreateMasteringVoice:
        Option<extern "C" fn(*mut IXAudio27, *mut usize, u32, u32, u32, u32, usize) -> HRESULT>,

    StartEngine: Option<extern "C" fn(*mut IXAudio27) -> HRESULT>,
    StopEngine: Option<extern "C" fn(*mut IXAudio27) -> HRESULT>,
    CommitChanges: Option<extern "C" fn(*mut IXAudio27, u32) -> HRESULT>,
    GetPerformanceData: Option<extern "C" fn(*mut IXAudio27, usize) -> HRESULT>,
    SetDebugConfiguration: Option<
        extern "C" fn(
            *mut IXAudio27,
            *const XAudio2::XAUDIO2_DEBUG_CONFIGURATION,
            usize,
        ) -> HRESULT,
    >,
}

#[repr(C)]
struct IXAudio27 {
    vtable: *const IXAudio27Vtable,
}

/// This function is invoked when the game calls `XAudio2Create`.
fn xaudio2create_hook(xaudio2_out: *mut *mut IXAudio27, flags: u32, processor: u32) -> HRESULT {
    // Call the original `XAudio2Create`.
    match unsafe { XAudio2CreateHook.call(xaudio2_out, flags, processor) }.ok() {
        Ok(_) => {}
        Err(e) => return e.code(),
    }

    // Grab a mutable reference to XAudio2 so we can call functions.
    let xaudio2 = unsafe { (*xaudio2_out).as_mut() }.unwrap();

    // Leak the callbacks, I guess...
    let callbacks = Box::into_raw(Box::new(XAudio27Callbacks::new()));

    // Register ourselves for XAudio2 callbacks.
    unsafe {
        (xaudio2.vtable.as_ref().unwrap())
            .RegisterForCallbacks
            .unwrap()(xaudio2, callbacks)
        .unwrap();
    }

    let config = XAudio2::XAUDIO2_DEBUG_CONFIGURATION {
        TraceMask: XAudio2::XAUDIO2_LOG_DETAIL
            | XAudio2::XAUDIO2_LOG_WARNINGS
            | XAudio2::XAUDIO2_LOG_API_CALLS,
        BreakMask: 0,
        LogThreadID: BOOL::from(false),
        LogFileline: BOOL::from(true),
        LogFunctionName: BOOL::from(true),
        LogTiming: BOOL::from(true),
    };

    // Also, turn on XAudio2 debugging.
    unsafe {
        (xaudio2.vtable.as_ref().unwrap())
            .SetDebugConfiguration
            .unwrap()(xaudio2, &config, 0)
        .unwrap();
    }

    log(LogType::Init, "Hooked XAudio2Create and loaded XAudio 2.7");
    HRESULT::from_win32(0)
}

pub fn init() -> anyhow::Result<()> {
    let udk = get_udk_slice();

    // SAFETY: This is only safe if the UDK binary matches what we expect.
    unsafe {
        XAudio2CreateHook
            .initialize(
                std::mem::transmute(udk.as_ptr().add(UDK_XAUDIO2CREATE_OFFSET)),
                xaudio2create_hook,
            )
            .context("Failed to setup InitializeHardware hook")?;

        XAudio2CreateHook.enable()?;
    }

    Ok(())
}
