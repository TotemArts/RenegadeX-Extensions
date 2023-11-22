//! This module handles functionality related to the original dinput8.dll
use windows::{
    core::{IUnknown, ComInterface, GUID, HRESULT},
    Win32::{
        Devices::HumanInterfaceDevice::{CLSID_DirectInput8, IDirectInput8A, IDirectInput8W},
        Foundation::{E_NOINTERFACE, HINSTANCE, S_OK},
        System::Com::{CoCreateInstance, CLSCTX_INPROC_SERVER},
    },
};

/// This function simply redirects the one and only DirectInput8Create call to the real dinput8 DLL.
#[no_mangle]
#[export_name = "DirectInput8Create"]
pub unsafe extern "C" fn directinput8_create(
    hinst: HINSTANCE,
    dwversion: u32,
    riid: *const GUID,
    out: *mut Option<IUnknown>,
    outer: Option<IUnknown>,
) -> HRESULT {
    // Instead of trying to load the original dinput8.dll and calling the original `DirectInput8Create`,
    // we can simply load the dinput8 interface via COM and return it up to our caller. This is basically
    // what DirectInput8Create does noawadays anyway.
    //
    // Reference: https://learn.microsoft.com/en-us/previous-versions/windows/desktop/ee416756(v=vs.85)
    let f = || -> Result<IUnknown, windows::core::Error> {
        // Initialize COM with the default apartment type.
        // NOTE: Disabled for now. The documentation does not really specify what the original `DirectInput8Create`
        // does in this case, and I'm too lazy to disassemble dinput8.dll to figure it out.
        // unsafe { CoInitializeEx(None, COINIT_MULTITHREADED)? };

        match unsafe { riid.as_ref() } {
            Some(&IDirectInput8A::IID) => {
                let dinput: IDirectInput8A =
                    CoCreateInstance(&CLSID_DirectInput8, outer.as_ref(), CLSCTX_INPROC_SERVER)?;

                // Per the documentation, if pUnkOuter != NULL then the resulting object must be initialized manually.
                if outer.is_none() {
                    dinput.Initialize(hinst, dwversion)?;
                }
                Ok(dinput.cast()?)
            }
            Some(&IDirectInput8W::IID) => {
                let dinput: IDirectInput8W =
                    CoCreateInstance(&CLSID_DirectInput8, outer.as_ref(), CLSCTX_INPROC_SERVER)?;

                // Per the documentation, if pUnkOuter != NULL then the resulting object must be initialized manually.
                if outer.is_none() {
                    dinput.Initialize(hinst, dwversion)?;
                }
                Ok(dinput.cast()?)
            }
            _ => return Err(E_NOINTERFACE.into()),
        }
    };

    *out = match f() {
        Ok(v) => Some(v),
        Err(e) => return e.code(),
    };

    S_OK
}
