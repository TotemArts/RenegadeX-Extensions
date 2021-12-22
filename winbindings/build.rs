fn main() {
    windows::build! {
        Windows::Win32::{
            Foundation::{BOOL, HANDLE, HINSTANCE},
            System::{
                Diagnostics::Debug::OutputDebugStringW,
                SystemServices::{DLL_PROCESS_ATTACH, DLL_PROCESS_DETACH, DLL_THREAD_ATTACH, DLL_THREAD_DETACH},
                LibraryLoader::GetModuleHandleA,
                Threading::GetCurrentProcess,
                ProcessStatus::{K32GetModuleInformation, K32GetModuleFileNameExW, MODULEINFO},
            },
            Media::Audio::XAudio2::{XAUDIO2_DEBUG_CONFIGURATION, XAUDIO2_LOG_DETAIL, XAUDIO2_LOG_WARNINGS, XAUDIO2_LOG_API_CALLS},
            Media::Multimedia::WAVEFORMATEXTENSIBLE,
        },

    };
}
