//! This module contains functionality related to UDK XAudio hooks.
use std::ffi::c_void;

use anyhow::Context;
use detour::static_detour;
use widestring::WideChar;
use winbindings::Guid;
use winbindings::Windows::Win32::Foundation::BOOL;

use crate::get_udk_slice;
use crate::udk_log;

use winbindings::Windows::Win32::Media::{
    Audio::{CoreAudio, XAudio2},
    Multimedia::{self, WAVEFORMATEX, WAVEFORMATEXTENSIBLE, WAVEFORMATEXTENSIBLE_0},
};
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
    RegisterForCallbacks: Option<extern "C" fn(*mut IXAudio27, *mut c_void) -> HRESULT>,
    UnregisterForCallbacks: Option<extern "C" fn(*mut IXAudio27, *mut c_void) -> HRESULT>,

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
    SetDebugConfiguration: Option<extern "C" fn(*mut IXAudio27, usize, usize) -> HRESULT>,
}

static XAUDIO27_VTABLE: IXAudio27Vtable = IXAudio27Vtable {
    QueryInterface: None,
    AddRef: None,
    Release: None,
    GetDeviceCount: Some(XAudio27_GetDeviceCount),
    GetDeviceDetails: Some(XAudio27_GetDeviceDetails),
    Initialize: None,
    RegisterForCallbacks: None,
    UnregisterForCallbacks: None,

    CreateSourceVoice: Some(XAudio27_CreateSourceVoice),
    CreateSubmixVoice: Some(XAudio27_CreateSubmixVoice),
    CreateMasteringVoice: Some(XAudio27_CreateMasteringVoice),

    StartEngine: None,
    StopEngine: None,
    CommitChanges: None,
    GetPerformanceData: None,
    SetDebugConfiguration: None,
};

extern "C" fn XAudio27_GetDeviceCount(_this: *mut IXAudio27, count: *mut u32) -> HRESULT {
    unsafe {
        *count = 1;
    }

    HRESULT::from_win32(0)
}

extern "C" fn XAudio27_GetDeviceDetails(
    _this: *mut IXAudio27,
    _idx: u32,
    details_ptr: *mut XAudio27DeviceDetails,
) -> HRESULT {
    let display_name = widestring::WideCString::from_str("Virtual Audio Device").unwrap();

    let details = {
        let mut details = XAudio27DeviceDetails {
            DeviceID: [WideChar::from(0u16); 256],
            DisplayName: [WideChar::from(0u16); 256],
            Role: 0xF, // GlobalDefaultDevice
            OutputFormat: WAVEFORMATEXTENSIBLE {
                Format: WAVEFORMATEX {
                    wFormatTag: Multimedia::WAVE_FORMAT_PCM as u16,
                    nChannels: 2,
                    nSamplesPerSec: 44100,
                    nAvgBytesPerSec: 65536,
                    nBlockAlign: 4,
                    wBitsPerSample: 16,
                    cbSize: 0,
                },

                Samples: WAVEFORMATEXTENSIBLE_0 { wReserved: 0 },
                dwChannelMask: 0x03,
                SubFormat: Guid::new().unwrap(), // Hopefully they don't check this?
            },
        };

        details.DisplayName[..display_name.len()].copy_from_slice(display_name.as_slice());
        details
    };

    unsafe {
        *details_ptr = details;
    }

    HRESULT::from_win32(0)
}

extern "C" fn XAudio27_CreateMasteringVoice(
    this: *mut IXAudio27,
    voice_out: *mut usize,
    input_channels: u32,
    input_sample_rate: u32,
    flags: u32,
    _device_index: u32,
    effect_chain: usize,
) -> HRESULT {
    let this = unsafe { this.as_mut() }.unwrap();

    match unsafe {
        this.xaudio.CreateMasteringVoice(
            std::mem::transmute(voice_out),
            input_channels,
            input_sample_rate,
            flags,
            None,
            std::mem::transmute(effect_chain),
            CoreAudio::AudioCategory_GameMedia,
        )
    } {
        Ok(_) => HRESULT::from_win32(0),
        Err(e) => e.code(),
    }
}

extern "C" fn XAudio27_CreateSubmixVoice(
    this: *mut IXAudio27,
    voice_out: *mut usize,
    input_channels: u32,
    input_sample_rate: u32,
    flags: u32,
    processing_stage: u32,
    send_list: usize,
    effect_chain: usize,
) -> HRESULT {
    let this = unsafe { this.as_mut() }.unwrap();

    match unsafe {
        this.xaudio.CreateSubmixVoice(
            std::mem::transmute(voice_out),
            input_channels,
            input_sample_rate,
            flags,
            processing_stage,
            std::mem::transmute(send_list),
            std::mem::transmute(effect_chain),
        )
    } {
        Ok(_) => HRESULT::from_win32(0),
        Err(e) => e.code(),
    }
}

extern "C" fn XAudio27_CreateSourceVoice(
    this: *mut IXAudio27,
    voice_out: *mut usize,
    format: usize,
    flags: u32,
    max_frequency_ratio: f32,
    callback: usize,
    send_list: usize,
    effect_chain: usize,
) -> HRESULT {
    let this = unsafe { this.as_mut() }.unwrap();

    match unsafe {
        this.xaudio.CreateSourceVoice(
            std::mem::transmute(voice_out),
            std::mem::transmute(format),
            flags,
            max_frequency_ratio,
            std::mem::transmute::<usize, XAudio2::IXAudio2VoiceCallback>(callback),
            std::mem::transmute(send_list),
            std::mem::transmute(effect_chain),
        )
    } {
        Ok(_) => HRESULT::from_win32(0),
        Err(e) => e.code(),
    }
}

#[repr(C)]
struct IXAudio27 {
    vtable: *const IXAudio27Vtable,
    xaudio: XAudio2::IXAudio2,
}

fn xaudio2create_hook(xaudio2_out: *mut *mut IXAudio27, flags: u32, processor: u32) -> HRESULT {
    let mut xaudio = None;
    match unsafe {
        XAudio2::XAudio2CreateWithVersionInfo(&mut xaudio, flags, processor, 0x0A000000)
    } {
        Ok(_) => {}
        Err(e) => return e.code(),
    }

    let xaudio = xaudio.unwrap();
    let config = XAudio2::XAUDIO2_DEBUG_CONFIGURATION {
        TraceMask: XAudio2::XAUDIO2_LOG_DETAIL
            | XAudio2::XAUDIO2_LOG_WARNINGS
            | XAudio2::XAUDIO2_LOG_API_CALLS,
        BreakMask: XAudio2::XAUDIO2_LOG_ERRORS,
        LogThreadID: BOOL::from(false),
        LogFileline: BOOL::from(true),
        LogFunctionName: BOOL::from(true),
        LogTiming: BOOL::from(true),
    };

    unsafe {
        xaudio.SetDebugConfiguration(&config, std::ptr::null_mut());
        
        *xaudio2_out = Box::into_raw(Box::new(IXAudio27 {
            vtable: &XAUDIO27_VTABLE,
            xaudio: xaudio,
        }));
    }

    udk_log::log(
        udk_log::LogType::Init,
        "Hooked XAudio2Create and loaded XAudio 2.8.",
    );
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
