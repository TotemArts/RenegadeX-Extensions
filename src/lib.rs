mod dinput8;
#[allow(non_camel_case_types)]
#[allow(non_snake_case)]
mod xaudio27;

mod dll;
mod udk_log;
mod udk_xaudio;

pub fn post_udk_init() -> anyhow::Result<()> {
    udk_xaudio::init()?;
    Ok(())
}