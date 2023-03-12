# Renegade X UDK Extensions
This repository hosts the code for extensions made to the UDK for Renegade X.

When compiled, this repository will produce a fake `dinput8.dll` intended to live alongside `UDK.exe`.
Installation simply involves copying our DLL into the game's binary directory. No extra steps need to be taken to load the extensions.

## Layout
 * `src/`
   * `dinput8.rs` - redirected dinput8 API
   * `lib.rs` - initialization code
   * `udk_log.rs` - UDK logging FFI
   * `udk_offsets.rs` - Constants describing important offsets in the UDK binary
   * `udk_xaudio.rs` - UDK XAudio FFI and detours
   * `xaudio27.rs` - XAudio2.7 -> 2.9 compatibility layer
 * `winbindings/` - Windows API Rust bindings.

## Loading the extensions
When the system loads the UDK, it will load all DLL dependencies alongside the UDK before executing any game code.

When searching for dependencies, the first path the system will look is in the same directory as the executable binary.
In addition, every DLL binary can have an entry-point function (`DllMain`) that the system's loader will invoke after loading it.

As such, we can create our own DLL and put it in place of one of the UDK's dependencies.
The system will load and run our code while starting up the game, but _before_ executing any game code.

So, we chose a random dependency that just happened to export only a single function (`dinput8.dll`, which exposes `DirectInputCreate8`) and replaced it with our own code.
And just like that, we get the opportunity to install hooks in the game, without the cost of designing our own loader.