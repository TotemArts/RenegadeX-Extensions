fn main() {
    windows::build! {
        Windows::Win32::{
            Foundation::*,
            System::{
                SystemServices::*,
                LibraryLoader::*,
                Threading::*,
                ProcessStatus::*,
            },
            Media::Audio::XAudio2::*,
            Media::Multimedia::{WAVEFORMATEX, WAVEFORMATEXTENSIBLE, WAVE_FORMAT_PCM},
        }
    };
}
