fn main() {
    windows::build! {
        Windows::Win32::Foundation::*,
        Windows::Win32::System::{
            SystemServices::*,
            LibraryLoader::*,
            Threading::*,
            ProcessStatus::*,
        }
    };
}
