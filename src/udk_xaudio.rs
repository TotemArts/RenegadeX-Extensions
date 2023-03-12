//! This module contains functionality related to UDK XAudio hooks.
use anyhow::Context;
use detour::static_detour;

use crate::get_udk_slice;
use crate::udk_log::{log, LogType};
use crate::udk_offsets::{UDK_CREATEFX_PTR_OFFSET, UDK_XAUDIO2CREATE_OFFSET};
use crate::xaudio27::{IXAudio27, XAudio27Wrapper};

use windows::core::{GUID, HRESULT};
use windows::Win32::Foundation::{E_FAIL, S_OK};

static_detour! {
    static XAudio2CreateHook: extern "C" fn(*mut IXAudio27, u32, u32) -> HRESULT;
}

// FX_API_(HRESULT) CreateFX (REFCLSID clsid, __deref_out IUnknown** pEffect);
fn createfx_hook(uuid: *const GUID, p_effect: *mut Option<windows::core::IUnknown>) -> HRESULT {
    const OLD_FXEQ: GUID = GUID::from_values(
        0xA90BC001,
        0xE897,
        0xE897,
        [0x74, 0x39, 0x43, 0x55, 0x00, 0x00, 0x00, 0x00],
    );
    const OLD_FXMASTERINGLIMITER: GUID = GUID::from_values(
        0xA90BC001,
        0xE897,
        0xE897,
        [0x74, 0x39, 0x43, 0x55, 0x00, 0x00, 0x00, 0x01],
    );
    const OLD_FXREVERB: GUID = GUID::from_values(
        0xA90BC001,
        0xE897,
        0xE897,
        [0x74, 0x39, 0x43, 0x55, 0x00, 0x00, 0x00, 0x02],
    );
    const OLD_FXECHO: GUID = GUID::from_values(
        0xA90BC001,
        0xE897,
        0xE897,
        [0x74, 0x39, 0x43, 0x55, 0x00, 0x00, 0x00, 0x03],
    );

    // Translate GUID from XAudio 2.7 to XAudio 2.9.
    let uuid = match unsafe { *uuid } {
        // FXEQ
        OLD_FXEQ => GUID::from_values(
            0xF5E01117,
            0xD6C4,
            0x485A,
            [0xA3, 0xF5, 0x69, 0x51, 0x96, 0xF3, 0xDB, 0xFA],
        ),
        // FXMasteringLimiter
        OLD_FXMASTERINGLIMITER => GUID::from_values(
            0xC4137916,
            0x2BE1,
            0x46FD,
            [0x85, 0x99, 0x44, 0x15, 0x36, 0xF4, 0x98, 0x56],
        ),
        // FXReverb
        OLD_FXREVERB => GUID::from_values(
            0x7D9ACA56,
            0xCB68,
            0x4807,
            [0xB6, 0x32, 0xB1, 0x37, 0x35, 0x2E, 0x85, 0x96],
        ),
        // FXEcho
        OLD_FXECHO => GUID::from_values(
            0x5039D740,
            0xF736,
            0x449A,
            [0x84, 0xD3, 0xA5, 0x62, 0x02, 0x55, 0x7B, 0x87],
        ),
        _ => return E_FAIL,
    };

    // Call XAudio2.9 CreateFX.
    unsafe {
        windows::Win32::Media::Audio::XAudio2::CreateFX(&uuid, p_effect, None, 0)
            .map_or_else(|e| e.code(), |_| S_OK)
    }
}

/// This function is invoked when the game calls `XAudio2Create`.
fn xaudio2create_hook(xaudio2_out: *mut IXAudio27, _flags: u32, _processor: u32) -> HRESULT {
    let object: IXAudio27 = match XAudio27Wrapper::new() {
        Ok(d) => d.into(),
        Err(e) => return e.code(),
    };

    unsafe { xaudio2_out.write(object) };

    log(
        LogType::Init,
        "Hooked XAudio2Create and loaded XAudio 2.7 detours",
    );
    S_OK
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

        // Enable RW access to the CreateFX pointer.
        let _guard = region::protect_with_handle(
            udk.as_ptr().add(UDK_CREATEFX_PTR_OFFSET),
            8,
            region::Protection::READ_WRITE,
        )
        .context("failed to adjust memory protection for CreateFX")?;

        // Overwrite xapofx!CreateFX pointer with our hook.
        (udk.as_ptr().add(UDK_CREATEFX_PTR_OFFSET) as *mut usize).write(createfx_hook as usize);
    }

    Ok(())
}
