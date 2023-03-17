use windows::core::{implement, interface, IUnknown, IUnknown_Vtbl, GUID, HRESULT};
use windows::Win32::Foundation::{BOOL, E_FAIL, S_OK};
use windows::Win32::Media::Audio::XAudio2::{
    IXAudio2, IXAudio2MasteringVoice, IXAudio2SourceVoice, IXAudio2SubmixVoice, IXAudio2Voice,
    XAUDIO2_BUFFER, XAUDIO2_BUFFER_WMA, XAUDIO2_DEBUG_CONFIGURATION, XAUDIO2_DEFAULT_PROCESSOR,
    XAUDIO2_EFFECT_CHAIN, XAUDIO2_FILTER_PARAMETERS, XAUDIO2_LOG_ERRORS, XAUDIO2_LOG_WARNINGS,
    XAUDIO2_SEND_DESCRIPTOR, XAUDIO2_VOICE_SENDS, XAUDIO2_VOICE_STATE,
};
use windows::Win32::Media::Audio::{
    AudioCategory_GameMedia, XAudio2, WAVEFORMATEX, WAVEFORMATEXTENSIBLE, WAVE_FORMAT_PCM,
};
use windows::Win32::System::SystemInformation::NTDDI_WIN10;

use paste::paste;
use std::ffi::c_void;
use widestring::{WideCStr, WideChar};

use crate::udk_log;

fn debug_log(msg: std::fmt::Arguments) {
    let msg = std::fmt::format(msg);

    // Log to the UDK.
    udk_log::log(udk_log::LogType::Init, &msg);

    // Only bother logging when debug assertions are on.
    #[cfg(debug_assertions)]
    {
        use windows::core::PCSWSTR;
        use windows::Win32::System::Diagnostics::Debug::OutputDebugStringW;

        // OutputDebugString does not append newlines.
        let msg = format!("{msg}\n");
        let wstr = widestring::U16CString::from_str(&msg).unwrap();

        unsafe { OutputDebugStringW(PCWSTR(wstr.as_ptr())) }
    }
}

/// Initialize a wide string u16 array from a buffer
fn wstr_array<const N: usize>(src: &WideCStr) -> [u16; N] {
    assert!(src.len() < N);

    let mut a: [u16; N] = [0u16; N];
    a[..src.len()].copy_from_slice(src.as_slice());

    a
}

macro_rules! todo_log {
    () => {
        debug_log(std::format_args!(
            "HOOK: unimplemented: {}:{}",
            file!(),
            line!()
        ));
    };
    ($fmt:expr) => {
        debug_log(std::format_args!(
            concat!("HOOK: unimplemented: {}:{}: ", $fmt),
            file!(),
            line!(),
        ));
    };

    ($fmt:expr, $($args:tt),*) => {
        debug_log(std::format_args!(
            concat!("HOOK: unimplemented: {}:{}: ", $fmt),
            file!(),
            line!(),
            $($args),*
        ));
    };
}

// This macro is derived from the #[implement()] macro provided by the Windows crate, but modified
// for non-COM interfaces. Remove this when they fix the upstream macro.
macro_rules! impl_iface {
    ($implementation:ident, $iface:ident) => {
        paste! {
            #[repr(C)]
            struct [<$implementation _Impl>] {
                // This *MUST* be laid out as follows, due to reasons.
                // vtables: (*const <$iface as ::windows::core::Vtable>::Vtable,),
                this: $implementation,
                container: *const ::windows::core::ScopedHeap,
            }
            impl [<$implementation _Impl>] {
                /*
                const VTABLES: (<$iface as ::windows::core::Vtable>::Vtable,) = (
                    <$iface as ::windows::core::Vtable>::Vtable::new::<$implementation>(
                    ),
                );
                */
                fn new(this: $implementation) -> Self {
                    Self {
                        // vtables: (&Self::VTABLES.0,),
                        this,
                        container: ::core::ptr::null(),
                    }
                }
            }
            impl ::core::convert::From<$implementation> for $iface {
                fn from(this: $implementation) -> Self {
                    const VTABLE: <$iface as ::windows::core::Vtable>::Vtable =
                            <$iface as ::windows::core::Vtable>::Vtable::new::<$implementation>();

                    let this = [<$implementation _Impl>]::new(this);
                    let mut this = ::std::boxed::Box::into_raw(::std::boxed::Box::new(this));

                    let container = ::windows::core::ScopedHeap {
                        vtable: &VTABLE as *const _ as *const _,
                        this: this as *const _ as *const _,
                    };

                    let container = ::std::boxed::Box::into_raw(::std::boxed::Box::new(container));

                    unsafe {
                        // Add a reverse link from this to the container object.
                        (*this).container = container;

                        // Return the vtable pointer in our container.
                        let vtable_ptr = &(*container).vtable;
                        ::std::mem::transmute(vtable_ptr)
                    }
                }
            }
            impl ::windows::core::AsImpl<$implementation> for $iface {
                fn as_impl(&self) -> &$implementation {
                    let this = ::windows::core::Vtable::as_raw(self);
                    unsafe {
                        let this = (this as *mut *mut ::core::ffi::c_void).sub(0 + 0)
                            as *mut [<$implementation _Impl>];
                        &(*this).this
                    }
                }
            }
            impl ScopedDrop for $implementation {
                unsafe fn drop_in_place(&self) {
                    let this = (self as *const _ as *mut [<$implementation _Impl>]);
                    let container = (*this).container as *mut ::windows::core::ScopedHeap;

                    // Convert this back into a box and drop it.
                    let _ = ::std::boxed::Box::from_raw(this);
                    // Ditto for the container.
                    let _ = ::std::boxed::Box::from_raw(container);
                }
            }
        }
    };
}

trait ScopedDrop {
    /// Drop the interface in-place. Note that this is unsafe for obvious reasons,
    /// and you must take care that you do not access `self` whatsoever after calling
    /// this function.
    unsafe fn drop_in_place(&self);
}

#[repr(C, packed)]
pub struct XAudio27DeviceDetails {
    pub DeviceID: [WideChar; 256],
    pub DisplayName: [WideChar; 256],
    pub Role: u32,
    pub OutputFormat: WAVEFORMATEXTENSIBLE,
}

impl std::default::Default for XAudio27DeviceDetails {
    fn default() -> Self {
        Self {
            DeviceID: [0 as WideChar; 256],
            DisplayName: [0 as WideChar; 256],
            Role: Default::default(),
            OutputFormat: Default::default(),
        }
    }
}

/// Returned by IXAudio2Voice::GetVoiceDetails
#[repr(C, packed)]
pub struct XAudio27VoiceDetails {
    CreationFlags: u32,   // Flags the voice was created with.
    InputChannels: u32,   // Channels in the voice's input audio.
    InputSampleRate: u32, // Sample rate of the voice's input audio.
}

/// Used in XAUDIO2_VOICE_SENDS below
#[repr(C, packed)]
pub struct XAudio27SendDescriptor {
    Flags: u32,                           // Either 0 or XAUDIO2_SEND_USEFILTER.
    pOutputVoice: Option<IXAudio27Voice>, // This send's destination voice.
}

/// Used in the voice creation functions and in IXAudio2Voice::SetOutputVoices
#[repr(C, packed)]
pub struct XAudio27VoiceSends {
    SendCount: u32,                      // Number of sends from this voice.
    pSends: *mut XAudio27SendDescriptor, // Array of SendCount send descriptors.
}

#[interface]
pub unsafe trait IXAudio27Callbacks {
    fn OnProcessingPassStart(&self);
    fn OnProcessingPassEnd(&self);
    fn OnCriticalError(&self, error: HRESULT);
}

#[interface]
pub unsafe trait IXAudio27Voice {
    fn GetVoiceDetails(&self, details_out: *mut XAudio27VoiceDetails);
    fn SetOutputVoices(&self, send_list: *mut XAudio27VoiceSends) -> HRESULT;
    fn SetEffectChain(&self, effect_chain: *const XAUDIO2_EFFECT_CHAIN) -> HRESULT;
    fn EnableEffect(&self, effect_index: u32, operation_set: u32) -> HRESULT;
    fn DisableEffect(&self, effect_index: u32, operation_set: u32) -> HRESULT;
    fn GetEffectState(&self, effect_index: u32, enabled_out: *mut BOOL);
    fn SetEffectParameters(
        &self,
        effect_index: u32,
        parameters: *const c_void,
        parameters_len: u32,
        operation_set: u32,
    ) -> HRESULT;
    fn GetEffectParameters(
        &self,
        effect_index: u32,
        parameters_out: *mut c_void,
        parameters_len: u32,
    ) -> HRESULT;
    fn SetFilterParameters(
        &self,
        parameters: *const XAUDIO2_FILTER_PARAMETERS,
        operation_set: u32,
    ) -> HRESULT;
    fn GetFilterParameters(&self, parameters: *mut XAUDIO2_FILTER_PARAMETERS);
    fn SetOutputFilterParameters(
        &self,
        dest_voice: IXAudio27Voice,
        parameters: *const XAUDIO2_FILTER_PARAMETERS,
        operation_set: u32,
    ) -> HRESULT;
    fn GetOutputFilterParameters(
        &self,
        dest_voice: IXAudio27Voice,
        parameters: *mut XAUDIO2_FILTER_PARAMETERS,
    );
    fn SetVolume(&self, volume: f32, operation_set: u32) -> HRESULT;
    fn GetVolume(&self, volume: *mut f32);
    fn SetChannelVolumes(&self, channels: u32, volumes: *const f32, operation_set: u32) -> HRESULT;
    fn GetChannelVolumes(&self, channels: u32, volumes: *mut f32);
    fn SetOutputMatrix(
        &self,
        dest_voice: Option<IXAudio27Voice>,
        source_channels: u32,
        dest_channels: u32,
        level_matrix: *const f32,
        operation_set: u32,
    ) -> HRESULT;
    fn GetOutputMatrix(
        &self,
        dest_voice: Option<IXAudio27Voice>,
        source_channels: u32,
        dest_channels: u32,
        level_matrix: *mut f32,
    );
    fn DestroyVoice(&self);
}

// HACK: This derives from IXAudio27Voice but there isn't currently a way to represent that.
#[interface]
pub unsafe trait IXAudio27MasteringVoice {
    // IXAudio27Voice {
    fn GetVoiceDetails(&self, details_out: *mut XAudio27VoiceDetails);
    fn SetOutputVoices(&self, send_list: *mut XAudio27VoiceSends) -> HRESULT;
    fn SetEffectChain(&self, effect_chain: *const XAUDIO2_EFFECT_CHAIN) -> HRESULT;
    fn EnableEffect(&self, effect_index: u32, operation_set: u32) -> HRESULT;
    fn DisableEffect(&self, effect_index: u32, operation_set: u32) -> HRESULT;
    fn GetEffectState(&self, effect_index: u32, enabled_out: *mut BOOL);
    fn SetEffectParameters(
        &self,
        effect_index: u32,
        parameters: *const c_void,
        parameters_len: u32,
        operation_set: u32,
    ) -> HRESULT;
    fn GetEffectParameters(
        &self,
        effect_index: u32,
        parameters_out: *mut c_void,
        parameters_len: u32,
    ) -> HRESULT;
    fn SetFilterParameters(
        &self,
        parameters: *const XAUDIO2_FILTER_PARAMETERS,
        operation_set: u32,
    ) -> HRESULT;
    fn GetFilterParameters(&self, parameters: *mut XAUDIO2_FILTER_PARAMETERS);
    fn SetOutputFilterParameters(
        &self,
        dest_voice: IXAudio27Voice,
        parameters: *const XAUDIO2_FILTER_PARAMETERS,
        operation_set: u32,
    ) -> HRESULT;
    fn GetOutputFilterParameters(
        &self,
        dest_voice: IXAudio27Voice,
        parameters: *mut XAUDIO2_FILTER_PARAMETERS,
    );
    fn SetVolume(&self, volume: f32, operation_set: u32) -> HRESULT;
    fn GetVolume(&self, volume: *mut f32);
    fn SetChannelVolumes(&self, channels: u32, volumes: *const f32, operation_set: u32) -> HRESULT;
    fn GetChannelVolumes(&self, channels: u32, volumes: *mut f32);
    fn SetOutputMatrix(
        &self,
        dest_voice: Option<IXAudio27Voice>,
        source_channels: u32,
        dest_channels: u32,
        level_matrix: *const f32,
        operation_set: u32,
    ) -> HRESULT;
    fn GetOutputMatrix(
        &self,
        dest_voice: Option<IXAudio27Voice>,
        source_channels: u32,
        dest_channels: u32,
        level_matrix: *mut f32,
    );
    fn DestroyVoice(&self);
    // } (IXAudio27Voice)

    // No additional functions.
}

#[interface]
pub unsafe trait IXAudio27SubmixVoice {
    // IXAudio27Voice {
    fn GetVoiceDetails(&self, details_out: *mut XAudio27VoiceDetails);
    fn SetOutputVoices(&self, send_list: *mut XAudio27VoiceSends) -> HRESULT;
    fn SetEffectChain(&self, effect_chain: *const XAUDIO2_EFFECT_CHAIN) -> HRESULT;
    fn EnableEffect(&self, effect_index: u32, operation_set: u32) -> HRESULT;
    fn DisableEffect(&self, effect_index: u32, operation_set: u32) -> HRESULT;
    fn GetEffectState(&self, effect_index: u32, enabled_out: *mut BOOL);
    fn SetEffectParameters(
        &self,
        effect_index: u32,
        parameters: *const c_void,
        parameters_len: u32,
        operation_set: u32,
    ) -> HRESULT;
    fn GetEffectParameters(
        &self,
        effect_index: u32,
        parameters_out: *mut c_void,
        parameters_len: u32,
    ) -> HRESULT;
    fn SetFilterParameters(
        &self,
        parameters: *const XAUDIO2_FILTER_PARAMETERS,
        operation_set: u32,
    ) -> HRESULT;
    fn GetFilterParameters(&self, parameters: *mut XAUDIO2_FILTER_PARAMETERS);
    fn SetOutputFilterParameters(
        &self,
        dest_voice: IXAudio27Voice,
        parameters: *const XAUDIO2_FILTER_PARAMETERS,
        operation_set: u32,
    ) -> HRESULT;
    fn GetOutputFilterParameters(
        &self,
        dest_voice: IXAudio27Voice,
        parameters: *mut XAUDIO2_FILTER_PARAMETERS,
    );
    fn SetVolume(&self, volume: f32, operation_set: u32) -> HRESULT;
    fn GetVolume(&self, volume: *mut f32);
    fn SetChannelVolumes(&self, channels: u32, volumes: *const f32, operation_set: u32) -> HRESULT;
    fn GetChannelVolumes(&self, channels: u32, volumes: *mut f32);
    fn SetOutputMatrix(
        &self,
        dest_voice: Option<IXAudio27Voice>,
        source_channels: u32,
        dest_channels: u32,
        level_matrix: *const f32,
        operation_set: u32,
    ) -> HRESULT;
    fn GetOutputMatrix(
        &self,
        dest_voice: Option<IXAudio27Voice>,
        source_channels: u32,
        dest_channels: u32,
        level_matrix: *mut f32,
    );
    fn DestroyVoice(&self);
    // } (IXAudio27Voice)

    // No additional functions.
}

#[interface]
pub unsafe trait IXAudio27SourceVoice {
    // IXAudio27Voice {
    fn GetVoiceDetails(&self, details_out: *mut XAudio27VoiceDetails);
    fn SetOutputVoices(&self, send_list: *mut XAudio27VoiceSends) -> HRESULT;
    fn SetEffectChain(&self, effect_chain: *const XAUDIO2_EFFECT_CHAIN) -> HRESULT;
    fn EnableEffect(&self, effect_index: u32, operation_set: u32) -> HRESULT;
    fn DisableEffect(&self, effect_index: u32, operation_set: u32) -> HRESULT;
    fn GetEffectState(&self, effect_index: u32, enabled_out: *mut BOOL);
    fn SetEffectParameters(
        &self,
        effect_index: u32,
        parameters: *const c_void,
        parameters_len: u32,
        operation_set: u32,
    ) -> HRESULT;
    fn GetEffectParameters(
        &self,
        effect_index: u32,
        parameters_out: *mut c_void,
        parameters_len: u32,
    ) -> HRESULT;
    fn SetFilterParameters(
        &self,
        parameters: *const XAUDIO2_FILTER_PARAMETERS,
        operation_set: u32,
    ) -> HRESULT;
    fn GetFilterParameters(&self, parameters: *mut XAUDIO2_FILTER_PARAMETERS);
    fn SetOutputFilterParameters(
        &self,
        dest_voice: IXAudio27Voice,
        parameters: *const XAUDIO2_FILTER_PARAMETERS,
        operation_set: u32,
    ) -> HRESULT;
    fn GetOutputFilterParameters(
        &self,
        dest_voice: IXAudio27Voice,
        parameters: *mut XAUDIO2_FILTER_PARAMETERS,
    );
    fn SetVolume(&self, volume: f32, operation_set: u32) -> HRESULT;
    fn GetVolume(&self, volume: *mut f32);
    fn SetChannelVolumes(&self, channels: u32, volumes: *const f32, operation_set: u32) -> HRESULT;
    fn GetChannelVolumes(&self, channels: u32, volumes: *mut f32);
    fn SetOutputMatrix(
        &self,
        dest_voice: Option<IXAudio27Voice>,
        source_channels: u32,
        dest_channels: u32,
        level_matrix: *const f32,
        operation_set: u32,
    ) -> HRESULT;
    fn GetOutputMatrix(
        &self,
        dest_voice: Option<IXAudio27Voice>,
        source_channels: u32,
        dest_channels: u32,
        level_matrix: *mut f32,
    );
    fn DestroyVoice(&self);
    // } (IXAudio27Voice)

    fn Start(&self, flags: u32, operation_set: u32) -> HRESULT;
    fn Stop(&self, flags: u32, operation_set: u32) -> HRESULT;
    fn SubmitSourceBuffer(
        &self,
        buffer: *const XAUDIO2_BUFFER,
        buffer_wma: *const XAUDIO2_BUFFER_WMA,
    ) -> HRESULT;
    fn FlushSourceBuffers(&self) -> HRESULT;
    fn Discontinuity(&self) -> HRESULT;
    fn ExitLoop(&self, operation_set: u32) -> HRESULT;
    fn GetState(&self, voice_state: *mut XAUDIO2_VOICE_STATE);
    fn SetFrequencyRatio(&self, ratio: f32, operation_set: u32) -> HRESULT;
    fn GetFrequencyRatio(&self, ratio: *mut f32);
    fn SetSourceSampleRate(&self, new_sample_rate: u32) -> HRESULT;
}

/// Represents the VTable for XAudio 2.7's IXAudio2 object.
///
/// Do _NOT_ alter the ordering of these fields.
#[interface("8bcf1f58-9fe7-4583-8ac6-e2adc465c8bb")]
pub unsafe trait IXAudio27: IUnknown {
    fn GetDeviceCount(&self, count: *mut u32) -> HRESULT;
    fn GetDeviceDetails(&self, index: u32, details: *mut XAudio27DeviceDetails) -> HRESULT;
    fn Initialize(&self, flags: u32, processor: u32) -> HRESULT;
    fn RegisterForCallbacks(&self, callbacks: *mut IXAudio27Callbacks) -> HRESULT;
    fn UnregisterForCallbacks(&self, callbacks: *mut IXAudio27Callbacks) -> HRESULT;
    fn CreateSourceVoice(
        &self,
        source_voice: *mut IXAudio27SourceVoice,
        source_format: *const WAVEFORMATEX,
        flags: u32,
        max_frequency_ratio: f32,
        callback: *const (),
        send_list: *const XAudio27VoiceSends,
        effect_chain: *const XAUDIO2_EFFECT_CHAIN,
    ) -> HRESULT;
    fn CreateSubmixVoice(
        &self,
        submix_voice: *mut IXAudio27SubmixVoice,
        input_channels: u32,
        input_sample_rate: u32,
        flags: u32,
        processing_stage: u32,
        send_list: *const XAudio27VoiceSends,
        effect_chain: *const XAUDIO2_EFFECT_CHAIN,
    ) -> HRESULT;
    fn CreateMasteringVoice(
        &self,
        mastering_voice: *mut IXAudio27MasteringVoice,
        input_channels: u32,
        input_sample_rate: u32,
        flags: u32,
        device_index: u32,
        effect_chain: *const XAUDIO2_EFFECT_CHAIN,
    ) -> HRESULT;
    fn StartEngine(&self) -> HRESULT;
    fn StopEngine(&self);
    fn CommitChanges(&self, operation_set: u32) -> HRESULT;
    fn GetPerformanceData(&self, perf_data_out: usize);
    fn SetDebugConfiguration(
        &self,
        debug_configuration: *const XAudio2::XAUDIO2_DEBUG_CONFIGURATION,
        reserved: usize,
    ) -> HRESULT;
}

unsafe fn translate_voice(voice: Option<IXAudio27Voice>) -> Option<IXAudio2Voice> {
    voice.map(|voice| {
        // Cast the IXAudio27Voice to one of our wrapper structs and then to a IXAudio2Voice (2.9)
        //
        // SAFETY: We're casting the IXAudio27Voice to a generic XAudio27VoiceWrapper which should be compatible
        // with our specific voice wrappers (since the layout is identical and the vtables should be equivalent).
        let voice_impl = (*(voice.0.as_ptr() as *const ::windows::core::ScopedHeap)).this
            as *const XAudio27VoiceWrapper;
        let inner_voice = (*voice_impl).0.clone(); // Find our inner voice

        inner_voice
    })
}

unsafe fn translate_send_list(sends: *const XAudio27VoiceSends) -> Vec<XAUDIO2_SEND_DESCRIPTOR> {
    let sends = if sends.is_null() {
        &[]
    } else {
        std::slice::from_raw_parts((*sends).pSends, (*sends).SendCount as usize)
    };

    let mut sends_out = Vec::new();
    for send in sends {
        // Use some trickery to pull the voice field out of the packed struct.
        let voice = std::ptr::read_unaligned(std::ptr::addr_of!(send.pOutputVoice));

        // The voice can be null sometimes...
        if let Some(voice) = voice {
            // Cast the IXAudio27Voice to one of our wrapper structs and then to a IXAudio2Voice (2.9)
            //
            // SAFETY: We're casting the IXAudio27Voice to a generic XAudio27VoiceWrapper which should be compatible
            // with our specific voice wrappers (since the layout is identical and the vtables should be equivalent).
            let voice_impl = (*(voice.0.as_ptr() as *const ::windows::core::ScopedHeap)).this
                as *const XAudio27VoiceWrapper;
            let inner_voice = (*voice_impl).0.clone(); // Find our inner voice

            sends_out.push(XAUDIO2_SEND_DESCRIPTOR {
                Flags: send.Flags,
                pOutputVoice: Some(inner_voice),
            })
        } else {
            sends_out.push(XAUDIO2_SEND_DESCRIPTOR {
                Flags: send.Flags,
                pOutputVoice: None,
            })
        }
    }

    sends_out
}

#[implement(IXAudio27)]
pub struct XAudio27Wrapper(IXAudio2);

impl XAudio27Wrapper {
    pub fn new() -> windows::core::Result<XAudio27Wrapper> {
        let mut xaudio2_out = None;
        unsafe {
            XAudio2::XAudio2CreateWithVersionInfo(
                &mut xaudio2_out,
                0,
                XAUDIO2_DEFAULT_PROCESSOR,
                NTDDI_WIN10, // TODO: ?? ntddiversion?
            )?;
        }

        let xaudio2 = xaudio2_out.unwrap();

        unsafe {
            xaudio2.SetDebugConfiguration(
                Some(&XAUDIO2_DEBUG_CONFIGURATION {
                    TraceMask: XAUDIO2_LOG_ERRORS | XAUDIO2_LOG_WARNINGS,
                    BreakMask: 0,
                    LogThreadID: windows::Win32::Foundation::BOOL(0),
                    LogFileline: windows::Win32::Foundation::BOOL(1),
                    LogFunctionName: windows::Win32::Foundation::BOOL(1),
                    LogTiming: windows::Win32::Foundation::BOOL(0),
                }),
                None,
            );
        }

        Ok(Self(xaudio2))
    }
}

impl IXAudio27_Impl for XAudio27Wrapper {
    unsafe fn GetDeviceCount(&self, count: *mut u32) -> HRESULT {
        *count = 1;
        S_OK
    }

    unsafe fn GetDeviceDetails(
        &self,
        _index: u32,
        details_out: *mut XAudio27DeviceDetails,
    ) -> HRESULT {
        let f = || -> windows::core::Result<()> {
            let details = XAudio27DeviceDetails {
                DeviceID: wstr_array(widestring::u16cstr!("ABC1234")),
                DisplayName: wstr_array(widestring::u16cstr!("Virtual Audio Device")),
                Role: 0xF,
                OutputFormat: WAVEFORMATEXTENSIBLE {
                    Format: WAVEFORMATEX {
                        wFormatTag: WAVE_FORMAT_PCM as u16,
                        nChannels: 2,
                        nSamplesPerSec: 48000,
                        nAvgBytesPerSec: 48000 * 2,
                        nBlockAlign: 2 * 2,
                        wBitsPerSample: 16, // This is a guess (famous last words)
                        cbSize: 22,
                    },
                    Samples: windows::Win32::Media::Audio::WAVEFORMATEXTENSIBLE_0 {
                        wValidBitsPerSample: 16,
                    },
                    dwChannelMask: 0x3,
                    SubFormat: GUID::default(),
                },
            };

            details_out.write(details);
            Ok(())
        };

        match f() {
            Ok(_) => S_OK,
            Err(e) => e.code(),
        }
    }

    unsafe fn Initialize(&self, _flags: u32, _processor: u32) -> HRESULT {
        todo_log!();

        // Not present in newer versions of XAudio.
        S_OK
    }

    unsafe fn RegisterForCallbacks(&self, _callbacks: *mut IXAudio27Callbacks) -> HRESULT {
        todo_log!();
        E_FAIL
    }

    unsafe fn UnregisterForCallbacks(&self, _callbacks: *mut IXAudio27Callbacks) -> HRESULT {
        todo_log!();
        E_FAIL
    }

    unsafe fn CreateSourceVoice(
        &self,
        source_voice_out: *mut IXAudio27SourceVoice,
        source_format: *const WAVEFORMATEX,
        flags: u32,
        max_frequency_ratio: f32,
        callback: *const (),
        send_list: *const XAudio27VoiceSends,
        effect_chain: *const XAUDIO2_EFFECT_CHAIN,
    ) -> HRESULT {
        // todo_log!(
        //     "CreateSourceVoice({:08X}, {}, {:016X}, {:016X}, {:016X})",
        //     flags,
        //     max_frequency_ratio,
        //     (callback as usize),
        //     (send_list as usize),
        //     (effect_chain as usize)
        // );

        let f = || -> windows::core::Result<()> {
            let send_list = translate_send_list(send_list);
            let sends = (!send_list.is_empty()).then_some(XAUDIO2_VOICE_SENDS {
                SendCount: send_list.len() as u32,
                pSends: send_list.as_ptr() as *mut _,
            });

            let mut voice_out = None;
            self.0.CreateSourceVoice(
                &mut voice_out,
                source_format,
                flags & 0x0E,
                max_frequency_ratio,
                (!callback.is_null()).then_some(std::mem::transmute(&callback)), // SAFETY: The interface is compatible between 2.7 and 2.9.
                sends.as_ref().map(|x| x as *const _),
                (!effect_chain.is_null()).then_some(effect_chain), // SAFETY: The interface is compatible between 2.7 and 2.9.
            )?;

            let source_voice: IXAudio27SourceVoice =
                XAudio27SourceVoiceWrapper(voice_out.unwrap()).into();

            source_voice_out.write(source_voice);
            Ok(())
        };

        match f() {
            Ok(_) => S_OK,
            Err(e) => e.code(),
        }
    }

    unsafe fn CreateSubmixVoice(
        &self,
        submix_voice_out: *mut IXAudio27SubmixVoice,
        input_channels: u32,
        input_sample_rate: u32,
        flags: u32,
        processing_stage: u32,
        send_list: *const XAudio27VoiceSends,
        effect_chain: *const XAUDIO2_EFFECT_CHAIN,
    ) -> HRESULT {
        // todo_log!(
        //     "CreateSubmixVoice({}, {}, {:08X}, {}, {:016X}, {:016X})",
        //     input_channels,
        //     input_sample_rate,
        //     flags,
        //     processing_stage,
        //     (send_list as usize),
        //     (effect_chain as usize)
        // );

        let f = || -> windows::core::Result<()> {
            let send_list = translate_send_list(send_list);
            let sends = (!send_list.is_empty()).then_some(XAUDIO2_VOICE_SENDS {
                SendCount: send_list.len() as u32,
                pSends: send_list.as_ptr() as *mut _,
            });

            let mut voice_out = None;
            self.0.CreateSubmixVoice(
                &mut voice_out,
                input_channels,
                input_sample_rate,
                flags,
                processing_stage,
                sends.as_ref().map(|x| x as *const _),
                (!effect_chain.is_null()).then_some(effect_chain), // SAFETY: The interface is compatible between 2.7 and 2.9.
            )?;

            let submix_voice: IXAudio27SubmixVoice =
                XAudio27SubmixVoiceWrapper(voice_out.unwrap()).into();

            submix_voice_out.write(submix_voice);
            Ok(())
        };

        match f() {
            Ok(_) => S_OK,
            Err(e) => e.code(),
        }
    }

    unsafe fn CreateMasteringVoice(
        &self,
        mastering_voice_out: *mut IXAudio27MasteringVoice,
        input_channels: u32,
        input_sample_rate: u32,
        flags: u32,
        _device_index: u32,
        effect_chain: *const XAUDIO2_EFFECT_CHAIN,
    ) -> HRESULT {
        // todo_log!(
        //     "CreateMasteringVoice({}, {}, {}, {}, {:016X})",
        //     input_channels,
        //     input_sample_rate,
        //     flags,
        //     device_index,
        //     (effect_chain as usize)
        // );

        let f = || -> windows::core::Result<()> {
            let mut voice_out = None;
            self.0.CreateMasteringVoice(
                &mut voice_out,
                input_channels,
                input_sample_rate,
                flags,
                None,                                              // TODO
                (!effect_chain.is_null()).then_some(effect_chain), // SAFETY: The interface is compatible between 2.7 and 2.9.
                AudioCategory_GameMedia,
            )?;

            let mastering_voice: IXAudio27MasteringVoice =
                XAudio27MasteringVoiceWrapper(voice_out.unwrap()).into();

            mastering_voice_out.write(mastering_voice);
            Ok(())
        };

        match f() {
            Ok(_) => S_OK,
            Err(e) => e.code(),
        }
    }

    unsafe fn StartEngine(&self) -> HRESULT {
        self.0.StartEngine().into()
    }

    unsafe fn StopEngine(&self) {
        self.0.StopEngine()
    }

    unsafe fn CommitChanges(&self, operation_set: u32) -> HRESULT {
        self.0.CommitChanges(operation_set).into()
    }

    unsafe fn GetPerformanceData(&self, perf_data_out: usize) {
        // SAFETY: The structure's layout is identical between XAudio 2.7 and 2.9.
        self.0.GetPerformanceData(perf_data_out as *mut _)
    }

    unsafe fn SetDebugConfiguration(
        &self,
        _debug_configuration: *const XAudio2::XAUDIO2_DEBUG_CONFIGURATION,
        _reserved: usize,
    ) -> HRESULT {
        todo_log!();
        E_FAIL
    }
}

struct XAudio27VoiceWrapper(IXAudio2Voice);

struct XAudio27MasteringVoiceWrapper(IXAudio2MasteringVoice);

impl_iface!(XAudio27MasteringVoiceWrapper, IXAudio27MasteringVoice);

impl IXAudio27MasteringVoice_Impl for XAudio27MasteringVoiceWrapper {
    // impl IXAudio27Voice_Impl for XAudio27MasteringVoiceWrapper {
    unsafe fn GetVoiceDetails(&self, _details_out: *mut XAudio27VoiceDetails) {
        todo_log!();
    }

    unsafe fn SetOutputVoices(&self, _send_list: *mut XAudio27VoiceSends) -> HRESULT {
        // Invalid for mastering voices.
        E_FAIL
    }

    unsafe fn SetEffectChain(&self, effect_chain: *const XAUDIO2_EFFECT_CHAIN) -> HRESULT {
        // SAFETY: The interface is compatible between 2.7 and 2.9.
        self.0
            .SetEffectChain((!effect_chain.is_null()).then_some(effect_chain))
            .into()
    }

    unsafe fn EnableEffect(&self, effect_index: u32, operation_set: u32) -> HRESULT {
        self.0.EnableEffect(effect_index, operation_set).into()
    }

    unsafe fn DisableEffect(&self, effect_index: u32, operation_set: u32) -> HRESULT {
        self.0.DisableEffect(effect_index, operation_set).into()
    }

    unsafe fn GetEffectState(&self, effect_index: u32, enabled_out: *mut BOOL) {
        self.0.GetEffectState(effect_index, enabled_out).into()
    }

    unsafe fn SetEffectParameters(
        &self,
        effect_index: u32,
        parameters: *const c_void,
        parameters_len: u32,
        operation_set: u32,
    ) -> HRESULT {
        // NOTE: The parameters are identical between XAudio 2.7 and 2.9, so we can simply forward the call.
        self.0
            .SetEffectParameters(effect_index, parameters, parameters_len, operation_set)
            .into()
    }

    unsafe fn GetEffectParameters(
        &self,
        effect_index: u32,
        parameters_out: *mut c_void,
        parameters_len: u32,
    ) -> HRESULT {
        // NOTE: The parameters are identical between XAudio 2.7 and 2.9, so we can simply forward the call.
        self.0
            .GetEffectParameters(effect_index, parameters_out, parameters_len)
            .into()
    }

    unsafe fn SetFilterParameters(
        &self,
        parameters: *const XAUDIO2_FILTER_PARAMETERS,
        operation_set: u32,
    ) -> HRESULT {
        self.0.SetFilterParameters(parameters, operation_set).into()
    }

    unsafe fn GetFilterParameters(&self, parameters: *mut XAUDIO2_FILTER_PARAMETERS) {
        self.0.GetFilterParameters(parameters)
    }

    unsafe fn SetOutputFilterParameters(
        &self,
        _dest_voice: IXAudio27Voice,
        _parameters: *const XAUDIO2_FILTER_PARAMETERS,
        _operation_set: u32,
    ) -> HRESULT {
        todo_log!();
        E_FAIL
    }

    unsafe fn GetOutputFilterParameters(
        &self,
        _dest_voice: IXAudio27Voice,
        _parameters: *mut XAUDIO2_FILTER_PARAMETERS,
    ) {
        todo_log!();
    }

    unsafe fn SetVolume(&self, volume: f32, operation_set: u32) -> HRESULT {
        self.0.SetVolume(volume, operation_set).into()
    }

    unsafe fn GetVolume(&self, volume: *mut f32) {
        self.0.GetVolume(volume)
    }

    unsafe fn SetChannelVolumes(
        &self,
        channels: u32,
        volumes: *const f32,
        operation_set: u32,
    ) -> HRESULT {
        self.0
            .SetChannelVolumes(
                std::slice::from_raw_parts(volumes, channels as usize),
                operation_set,
            )
            .into()
    }

    unsafe fn GetChannelVolumes(&self, channels: u32, volumes: *mut f32) {
        self.0
            .GetChannelVolumes(std::slice::from_raw_parts_mut(volumes, channels as usize))
    }

    unsafe fn SetOutputMatrix(
        &self,
        _dest_voice: Option<IXAudio27Voice>,
        _source_channels: u32,
        _dest_channels: u32,
        _level_matrix: *const f32,
        _operation_set: u32,
    ) -> HRESULT {
        todo_log!();
        E_FAIL
    }

    unsafe fn GetOutputMatrix(
        &self,
        _dest_voice: Option<IXAudio27Voice>,
        _source_channels: u32,
        _dest_channels: u32,
        _level_matrix: *mut f32,
    ) {
        todo_log!();
    }

    unsafe fn DestroyVoice(&self) {
        self.0.DestroyVoice();
        self.drop_in_place();
    }
    //} (IXAudio27Voice)
}

struct XAudio27SubmixVoiceWrapper(IXAudio2SubmixVoice);

impl_iface!(XAudio27SubmixVoiceWrapper, IXAudio27SubmixVoice);

impl IXAudio27SubmixVoice_Impl for XAudio27SubmixVoiceWrapper {
    //impl IXAudio27Voice_Impl for XAudio27SubmixVoiceWrapper {
    unsafe fn GetVoiceDetails(&self, _details_out: *mut XAudio27VoiceDetails) {
        todo_log!();
    }

    unsafe fn SetOutputVoices(&self, _send_list: *mut XAudio27VoiceSends) -> HRESULT {
        todo_log!();
        E_FAIL
    }

    unsafe fn SetEffectChain(&self, effect_chain: *const XAUDIO2_EFFECT_CHAIN) -> HRESULT {
        // SAFETY: The interface is compatible between 2.7 and 2.9.
        self.0
            .SetEffectChain((!effect_chain.is_null()).then_some(effect_chain))
            .into()
    }

    unsafe fn EnableEffect(&self, effect_index: u32, operation_set: u32) -> HRESULT {
        self.0.EnableEffect(effect_index, operation_set).into()
    }

    unsafe fn DisableEffect(&self, effect_index: u32, operation_set: u32) -> HRESULT {
        self.0.DisableEffect(effect_index, operation_set).into()
    }

    unsafe fn GetEffectState(&self, effect_index: u32, enabled_out: *mut BOOL) {
        self.0.GetEffectState(effect_index, enabled_out).into()
    }

    unsafe fn SetEffectParameters(
        &self,
        effect_index: u32,
        parameters: *const c_void,
        parameters_len: u32,
        operation_set: u32,
    ) -> HRESULT {
        // NOTE: The parameters are identical between XAudio 2.7 and 2.9, so we can simply forward the call.
        self.0
            .SetEffectParameters(effect_index, parameters, parameters_len, operation_set)
            .into()
    }

    unsafe fn GetEffectParameters(
        &self,
        effect_index: u32,
        parameters_out: *mut c_void,
        parameters_len: u32,
    ) -> HRESULT {
        // NOTE: The parameters are identical between XAudio 2.7 and 2.9, so we can simply forward the call.
        self.0
            .GetEffectParameters(effect_index, parameters_out, parameters_len)
            .into()
    }

    unsafe fn SetFilterParameters(
        &self,
        parameters: *const XAUDIO2_FILTER_PARAMETERS,
        operation_set: u32,
    ) -> HRESULT {
        self.0.SetFilterParameters(parameters, operation_set).into()
    }

    unsafe fn GetFilterParameters(&self, parameters: *mut XAUDIO2_FILTER_PARAMETERS) {
        self.0.GetFilterParameters(parameters)
    }

    unsafe fn SetOutputFilterParameters(
        &self,
        _dest_voice: IXAudio27Voice,
        _parameters: *const XAUDIO2_FILTER_PARAMETERS,
        _operation_set: u32,
    ) -> HRESULT {
        todo_log!();
        E_FAIL
    }

    unsafe fn GetOutputFilterParameters(
        &self,
        _dest_voice: IXAudio27Voice,
        _parameters: *mut XAUDIO2_FILTER_PARAMETERS,
    ) {
        todo_log!();
    }

    unsafe fn SetVolume(&self, volume: f32, operation_set: u32) -> HRESULT {
        self.0.SetVolume(volume, operation_set).into()
    }

    unsafe fn GetVolume(&self, volume: *mut f32) {
        self.0.GetVolume(volume)
    }

    unsafe fn SetChannelVolumes(
        &self,
        channels: u32,
        volumes: *const f32,
        operation_set: u32,
    ) -> HRESULT {
        self.0
            .SetChannelVolumes(
                std::slice::from_raw_parts(volumes, channels as usize),
                operation_set,
            )
            .into()
    }

    unsafe fn GetChannelVolumes(&self, channels: u32, volumes: *mut f32) {
        self.0
            .GetChannelVolumes(std::slice::from_raw_parts_mut(volumes, channels as usize))
    }

    unsafe fn SetOutputMatrix(
        &self,
        dest_voice: Option<IXAudio27Voice>,
        source_channels: u32,
        dest_channels: u32,
        level_matrix: *const f32,
        operation_set: u32,
    ) -> HRESULT {
        let f = || -> windows::core::Result<()> {
            let dest_voice = translate_voice(dest_voice);

            self.0.SetOutputMatrix(
                dest_voice.as_ref(),
                source_channels,
                dest_channels,
                level_matrix,
                operation_set,
            )
        };

        match f() {
            Ok(_) => S_OK,
            Err(e) => e.code(),
        }
    }

    unsafe fn GetOutputMatrix(
        &self,
        _dest_voice: Option<IXAudio27Voice>,
        _source_channels: u32,
        _dest_channels: u32,
        _level_matrix: *mut f32,
    ) {
        todo_log!();
    }

    unsafe fn DestroyVoice(&self) {
        self.0.DestroyVoice();
        self.drop_in_place();
    }
    // } IXAudio27Voice
}

struct XAudio27SourceVoiceWrapper(IXAudio2SourceVoice);

impl_iface!(XAudio27SourceVoiceWrapper, IXAudio27SourceVoice);

impl IXAudio27SourceVoice_Impl for XAudio27SourceVoiceWrapper {
    // impl IXAudio27Voice_Impl for XAudio27SourceVoiceWrapper {
    unsafe fn GetVoiceDetails(&self, _details_out: *mut XAudio27VoiceDetails) {
        todo_log!();
    }

    unsafe fn SetOutputVoices(&self, _send_list: *mut XAudio27VoiceSends) -> HRESULT {
        todo_log!();
        E_FAIL
    }

    unsafe fn SetEffectChain(&self, effect_chain: *const XAUDIO2_EFFECT_CHAIN) -> HRESULT {
        // SAFETY: The interface is compatible between 2.7 and 2.9.
        self.0
            .SetEffectChain((!effect_chain.is_null()).then_some(effect_chain))
            .into()
    }

    unsafe fn EnableEffect(&self, effect_index: u32, operation_set: u32) -> HRESULT {
        self.0.EnableEffect(effect_index, operation_set).into()
    }

    unsafe fn DisableEffect(&self, effect_index: u32, operation_set: u32) -> HRESULT {
        self.0.DisableEffect(effect_index, operation_set).into()
    }

    unsafe fn GetEffectState(&self, effect_index: u32, enabled_out: *mut BOOL) {
        self.0.GetEffectState(effect_index, enabled_out).into()
    }

    unsafe fn SetEffectParameters(
        &self,
        effect_index: u32,
        parameters: *const c_void,
        parameters_len: u32,
        operation_set: u32,
    ) -> HRESULT {
        // NOTE: The parameters are identical between XAudio 2.7 and 2.9, so we can simply forward the call.
        self.0
            .SetEffectParameters(effect_index, parameters, parameters_len, operation_set)
            .into()
    }

    unsafe fn GetEffectParameters(
        &self,
        effect_index: u32,
        parameters_out: *mut c_void,
        parameters_len: u32,
    ) -> HRESULT {
        // NOTE: The parameters are identical between XAudio 2.7 and 2.9, so we can simply forward the call.
        self.0
            .GetEffectParameters(effect_index, parameters_out, parameters_len)
            .into()
    }

    unsafe fn SetFilterParameters(
        &self,
        parameters: *const XAUDIO2_FILTER_PARAMETERS,
        operation_set: u32,
    ) -> HRESULT {
        self.0.SetFilterParameters(parameters, operation_set).into()
    }

    unsafe fn GetFilterParameters(&self, parameters: *mut XAUDIO2_FILTER_PARAMETERS) {
        self.0.GetFilterParameters(parameters)
    }

    unsafe fn SetOutputFilterParameters(
        &self,
        _dest_voice: IXAudio27Voice,
        _parameters: *const XAUDIO2_FILTER_PARAMETERS,
        _operation_set: u32,
    ) -> HRESULT {
        todo_log!();
        E_FAIL
    }

    unsafe fn GetOutputFilterParameters(
        &self,
        _dest_voice: IXAudio27Voice,
        _parameters: *mut XAUDIO2_FILTER_PARAMETERS,
    ) {
        todo_log!();
    }

    unsafe fn SetVolume(&self, volume: f32, operation_set: u32) -> HRESULT {
        self.0.SetVolume(volume, operation_set).into()
    }

    unsafe fn GetVolume(&self, volume: *mut f32) {
        self.0.GetVolume(volume)
    }

    unsafe fn SetChannelVolumes(
        &self,
        channels: u32,
        volumes: *const f32,
        operation_set: u32,
    ) -> HRESULT {
        self.0
            .SetChannelVolumes(
                std::slice::from_raw_parts(volumes, channels as usize),
                operation_set,
            )
            .into()
    }

    unsafe fn GetChannelVolumes(&self, channels: u32, volumes: *mut f32) {
        self.0
            .GetChannelVolumes(std::slice::from_raw_parts_mut(volumes, channels as usize))
    }

    unsafe fn SetOutputMatrix(
        &self,
        dest_voice: Option<IXAudio27Voice>,
        source_channels: u32,
        dest_channels: u32,
        level_matrix: *const f32,
        operation_set: u32,
    ) -> HRESULT {
        let dest_voice = translate_voice(dest_voice);

        self.0
            .SetOutputMatrix(
                dest_voice.as_ref(),
                source_channels,
                dest_channels,
                level_matrix,
                operation_set,
            )
            .into()
    }

    unsafe fn GetOutputMatrix(
        &self,
        _dest_voice: Option<IXAudio27Voice>,
        _source_channels: u32,
        _dest_channels: u32,
        _level_matrix: *mut f32,
    ) {
        todo_log!();
    }

    unsafe fn DestroyVoice(&self) {
        self.0.DestroyVoice();
        self.drop_in_place();
    }
    // } (IXAudio27Voice)

    unsafe fn Start(&self, flags: u32, operation_set: u32) -> HRESULT {
        self.0.Start(flags, operation_set).into()
    }

    unsafe fn Stop(&self, flags: u32, operation_set: u32) -> HRESULT {
        self.0.Stop(flags, operation_set).into()
    }

    unsafe fn SubmitSourceBuffer(
        &self,
        buffer: *const XAUDIO2_BUFFER,
        buffer_wma: *const XAUDIO2_BUFFER_WMA,
    ) -> HRESULT {
        self.0
            .SubmitSourceBuffer(buffer, (!buffer_wma.is_null()).then_some(buffer_wma))
            .into()
    }

    unsafe fn FlushSourceBuffers(&self) -> HRESULT {
        self.0.FlushSourceBuffers().into()
    }

    unsafe fn Discontinuity(&self) -> HRESULT {
        self.0.Discontinuity().into()
    }

    unsafe fn ExitLoop(&self, operation_set: u32) -> HRESULT {
        self.0.ExitLoop(operation_set).into()
    }

    unsafe fn GetState(&self, voice_state: *mut XAUDIO2_VOICE_STATE) {
        self.0.GetState(voice_state, 0)
    }

    unsafe fn SetFrequencyRatio(&self, ratio: f32, operation_set: u32) -> HRESULT {
        self.0.SetFrequencyRatio(ratio, operation_set).into()
    }

    unsafe fn GetFrequencyRatio(&self, ratio: *mut f32) {
        self.0.GetFrequencyRatio(ratio)
    }

    unsafe fn SetSourceSampleRate(&self, new_sample_rate: u32) -> HRESULT {
        self.0.SetSourceSampleRate(new_sample_rate).into()
    }
}
