//! Operating-system tuning so a game and the microphone can share a mid-range PC.

/// Windows: register the calling thread with the multimedia scheduler as "Pro Audio", the way audio
/// software does. It then gets its slice on time even when a game has every core busy.
#[cfg(windows)]
pub fn boost_audio_thread() {
    use windows_sys::Win32::System::Threading::AvSetMmThreadCharacteristicsW;
    let name: Vec<u16> = "Pro Audio\0".encode_utf16().collect();
    let mut index = 0u32;
    unsafe {
        AvSetMmThreadCharacteristicsW(name.as_ptr(), &mut index);
    }
}

/// Windows: run the rest of the app (window, settings) behind the game.
#[cfg(windows)]
pub fn lower_process_priority() {
    use windows_sys::Win32::System::Threading::{GetCurrentProcess, SetPriorityClass, BELOW_NORMAL_PRIORITY_CLASS};
    unsafe {
        SetPriorityClass(GetCurrentProcess(), BELOW_NORMAL_PRIORITY_CLASS);
    }
}

#[cfg(not(windows))]
pub fn boost_audio_thread() {}

#[cfg(not(windows))]
pub fn lower_process_priority() {}
