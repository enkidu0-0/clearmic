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

/// Windows: the microphone must never wait behind a game. Above-normal priority, and no power throttling: Windows 11
/// otherwise slows (or sleeps the timers of) a process whose window is closed to the tray, which breaks the audio up.
#[cfg(windows)]
pub fn raise_process_priority() {
    use windows_sys::Win32::System::Threading::{
        GetCurrentProcess, ProcessPowerThrottling, SetPriorityClass, SetProcessInformation, ABOVE_NORMAL_PRIORITY_CLASS,
        PROCESS_POWER_THROTTLING_CURRENT_VERSION, PROCESS_POWER_THROTTLING_EXECUTION_SPEED,
        PROCESS_POWER_THROTTLING_IGNORE_TIMER_RESOLUTION, PROCESS_POWER_THROTTLING_STATE,
    };
    unsafe {
        let me = GetCurrentProcess();
        SetPriorityClass(me, ABOVE_NORMAL_PRIORITY_CLASS);
        let state = PROCESS_POWER_THROTTLING_STATE {
            Version: PROCESS_POWER_THROTTLING_CURRENT_VERSION,
            ControlMask: PROCESS_POWER_THROTTLING_EXECUTION_SPEED | PROCESS_POWER_THROTTLING_IGNORE_TIMER_RESOLUTION,
            StateMask: 0, // 0 = throttling off for both
        };
        SetProcessInformation(me, ProcessPowerThrottling, &state as *const _ as *const _, std::mem::size_of_val(&state) as u32);
    }
}

#[cfg(not(windows))]
pub fn boost_audio_thread() {}

#[cfg(not(windows))]
pub fn raise_process_priority() {}
