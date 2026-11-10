// Hide the console window on Windows release builds.
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod autostart;
mod state;

use serde::Serialize;
use serde_json::Value;
use state::Shared;
use tauri::menu::{CheckMenuItem, Menu, MenuItem, PredefinedMenuItem};
use tauri::tray::{MouseButton, MouseButtonState, TrayIconBuilder, TrayIconEvent};
use tauri::{AppHandle, Manager, RunEvent, State, WebviewUrl, WebviewWindowBuilder};

#[derive(Serialize)]
struct Status {
    /// "waiting" | "connected" | "error"
    state: &'static str,
    error: Option<String>,
    /// Not a failure: for example, sound is going to the speakers because no virtual cable is installed.
    notice: Option<String>,
    /// Whether "hear my voice" is playing, and on which device.
    monitor_info: String,
    delay_ms: u32,
    nr_db: i32,
    loss_pct: f32,
    addr: String,
    /// Share of one 10 ms frame the processing takes.
    load_pct: u32,
    /// True while no phone is sending and the receiver is asleep.
    idle: bool,
    mode: String,
    /// True if the PC was too busy and a lighter noise remover took over.
    stepped_down: bool,
}

#[derive(Serialize)]
struct Levels {
    #[serde(rename = "in")]
    input: Vec<f32>,
    out: Vec<f32>,
}

#[derive(Serialize)]
struct Pairing {
    uri: String,
    addr: String,
    key: String,
    qr_svg: String,
}

#[derive(Serialize)]
struct Params {
    strength: f32,
    preset: String,
    performance: String,
    autostart: bool,
    autostart_supported: bool,
    hpf: bool,
    gate: bool,
    eq: bool,
    comp: bool,
    deess: bool,
    monitor: bool,
    output: String,
    outputs: Vec<String>,
}

#[tauri::command]
fn get_status(st: State<Shared>) -> Status {
    st.status()
}

#[tauri::command]
fn get_levels(st: State<Shared>) -> Levels {
    st.levels()
}

#[tauri::command]
fn get_pairing(st: State<Shared>) -> Result<Pairing, String> {
    st.pairing()
}

#[tauri::command]
fn get_params(st: State<Shared>) -> Params {
    st.params()
}

#[tauri::command]
fn set_param(st: State<Shared>, name: String, value: Value) -> Result<(), String> {
    st.set_param(&name, value)
}

#[tauri::command]
fn set_bypass(st: State<Shared>, on: bool) {
    st.set_bypass(on);
}

/// Start listening again (after an error or a changed setting).
#[tauri::command]
fn reconnect(st: State<Shared>) -> Result<(), String> {
    st.restart()
}

/// New key, new QR code. Phones paired before have to scan again.
#[tauri::command]
fn pair(st: State<Shared>) -> Result<(), String> {
    st.new_pairing()
}

/// Close the window and keep the receiver running in the tray. The web view is destroyed, which
/// is what gives its memory back.
#[tauri::command]
fn hide_to_tray(app: AppHandle) {
    if let Some(w) = app.get_webview_window("main") {
        let _ = w.destroy();
    }
}

/// Open the window, or bring it forward if it is already open.
fn show_window(app: &AppHandle) {
    if let Some(w) = app.get_webview_window("main") {
        let _ = w.unminimize();
        let _ = w.show();
        let _ = w.set_focus();
        return;
    }
    let _ = WebviewWindowBuilder::new(app, "main", WebviewUrl::App("index.html".into()))
        .title("ClearMic")
        .inner_size(1180.0, 780.0)
        .min_inner_size(900.0, 640.0)
        .build();
}

fn main() {
    clearmic_engine::os::raise_process_priority();

    let app = tauri::Builder::default()
        // a second launch just brings the first one forward
        .plugin(tauri_plugin_single_instance::init(|app, _args, _cwd| show_window(app)))
        .setup(|app| {
            let dir = app.path().app_config_dir().map_err(|e| e.to_string())?;
            let models = app.path().resource_dir().ok().map(|d| d.join("models"));
            let shared = Shared::load(dir, models);
            if let Err(e) = shared.restart() {
                eprintln!("engine did not start: {e}");
            }
            app.manage(shared);

            // tray: the app lives here when the window is closed
            let open = MenuItem::with_id(app, "open", "Open ClearMic", true, None::<&str>)?;
            let boot = CheckMenuItem::with_id(app, "boot", "Start with Windows", autostart::SUPPORTED, autostart::is_enabled(), None::<&str>)?;
            let quit = MenuItem::with_id(app, "quit", "Quit", true, None::<&str>)?;
            let menu = Menu::with_items(app, &[&open, &boot, &PredefinedMenuItem::separator(app)?, &quit])?;
            let boot_item = boot.clone();
            let mut tray = TrayIconBuilder::with_id("tray")
                .tooltip("ClearMic")
                .menu(&menu)
                .show_menu_on_left_click(false)
                .on_menu_event(move |app, event| match event.id().as_ref() {
                    "open" => show_window(app),
                    "boot" => {
                        let want = boot_item.is_checked().unwrap_or(false);
                        if autostart::set(want).is_err() {
                            let _ = boot_item.set_checked(autostart::is_enabled());
                        }
                    }
                    "quit" => app.exit(0),
                    _ => {}
                })
                .on_tray_icon_event(|tray, event| {
                    if let TrayIconEvent::Click { button: MouseButton::Left, button_state: MouseButtonState::Up, .. } = event {
                        show_window(tray.app_handle());
                    }
                });
            // the Mac menu bar wants a one-colour template image that the system tints; elsewhere the app icon is fine
            #[cfg(target_os = "macos")]
            {
                tray = tray.icon(tauri::image::Image::from_bytes(include_bytes!("../icons/tray-template.png"))?).icon_as_template(true);
            }
            #[cfg(not(target_os = "macos"))]
            if let Some(icon) = app.default_window_icon() {
                tray = tray.icon(icon.clone());
            }
            tray.build(app)?;

            // the tooltip says what the receiver is doing, refreshed lazily
            let handle = app.handle().clone();
            std::thread::spawn(move || {
                let mut last = String::new();
                loop {
                    std::thread::sleep(std::time::Duration::from_secs(3));
                    let st = handle.state::<Shared>().status();
                    let text = match st.state {
                        "connected" => "ClearMic: phone connected",
                        "waiting" => "ClearMic: waiting for a phone",
                        _ => "ClearMic: not connected",
                    };
                    if text != last {
                        if let Some(t) = handle.tray_by_id("tray") {
                            let _ = t.set_tooltip(Some(text));
                        }
                        last = text.to_string();
                    }
                }
            });

            // started by Windows with --minimized: stay in the tray, no window until asked
            if !std::env::args().any(|a| a == "--minimized") {
                show_window(app.handle());
            }
            // test hook: CLEARMIC_AUTOCLOSE_MS=<n> closes the window after n ms, as the close button does
            if let Some(ms) = std::env::var("CLEARMIC_AUTOCLOSE_MS").ok().and_then(|v| v.parse::<u64>().ok()) {
                let h = app.handle().clone();
                std::thread::spawn(move || {
                    std::thread::sleep(std::time::Duration::from_millis(ms));
                    hide_to_tray(h);
                });
            }
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            get_status, get_levels, get_pairing, get_params, set_param, set_bypass, reconnect, pair, hide_to_tray
        ])
        .build(tauri::generate_context!())
        .expect("error while building ClearMic");

    app.run(|app, event| match event {
        // closing the last window must not end the app; only Quit in the tray does
        RunEvent::ExitRequested { api, code, .. } if code.is_none() => api.prevent_exit(),
        RunEvent::Exit => app.state::<Shared>().shutdown(),
        _ => {}
    });
}
