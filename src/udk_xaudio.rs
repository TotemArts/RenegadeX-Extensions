//! This module contains functionality related to UDK XAudio hooks.
use anyhow::Context;
use detour::static_detour;

use crate::get_udk_slice;
use crate::udk_log::{log, LogType};
use crate::udk_offsets::UDK_XAUDIO2CREATE_OFFSET;
use crate::xaudio27::{IXAudio27, XAudio27Wrapper};

use windows::core::HRESULT;
use windows::Win32::Foundation::S_OK;

static_detour! {
    static XAudio2CreateHook: extern "C" fn(*mut IXAudio27, u32, u32) -> HRESULT;
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
        // Comment this out so we can fix the release version of renx temporarily
        // XAudio2CreateHook.enable()?;
    }

    Ok(())
}
