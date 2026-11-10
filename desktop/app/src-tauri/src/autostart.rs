//! "Start with Windows": a value under HKCU\...\Run that launches the app minimized to the tray.

#[cfg(windows)]
mod imp {
    use winreg::enums::{HKEY_CURRENT_USER, KEY_READ, KEY_WRITE};
    use winreg::RegKey;

    const KEY: &str = r"Software\Microsoft\Windows\CurrentVersion\Run";
    const NAME: &str = "Sonivane";

    pub const SUPPORTED: bool = true;

    pub fn is_enabled() -> bool {
        RegKey::predef(HKEY_CURRENT_USER)
            .open_subkey_with_flags(KEY, KEY_READ)
            .and_then(|k| k.get_value::<String, _>(NAME))
            .is_ok()
    }

    pub fn set(on: bool) -> Result<(), String> {
        let run = RegKey::predef(HKEY_CURRENT_USER)
            .open_subkey_with_flags(KEY, KEY_WRITE)
            .map_err(|e| e.to_string())?;
        if on {
            let exe = std::env::current_exe().map_err(|e| e.to_string())?;
            run.set_value(NAME, &format!("\"{}\" --minimized", exe.display())).map_err(|e| e.to_string())
        } else {
            match run.delete_value(NAME) {
                Ok(()) => Ok(()),
                Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(()),
                Err(e) => Err(e.to_string()),
            }
        }
    }
}

#[cfg(not(windows))]
mod imp {
    pub const SUPPORTED: bool = false;
    pub fn is_enabled() -> bool {
        false
    }
    pub fn set(_on: bool) -> Result<(), String> {
        Err("Starting with the computer is only set up for Windows".into())
    }
}

pub use imp::{is_enabled, set, SUPPORTED};
