mod browsers;
mod config;

use std::{path::PathBuf, sync::Mutex};

use serde::Serialize;
use tauri::{
    AppHandle, Emitter, LogicalSize, Manager, PhysicalPosition, RunEvent, WebviewUrl, WebviewWindowBuilder,
    WindowEvent,
    menu::{Menu, MenuItem, PredefinedMenuItem},
    tray::TrayIconBuilder,
};

use browsers::Browser;
use config::{Config, host_of};

const PICKER_WIDTH: f64 = 380.0;

struct AppState {
    browsers: Mutex<Vec<Browser>>,
    config: Mutex<Config>,
    config_path: PathBuf,
    /// The link waiting for a choice in the picker.
    pending: Mutex<Option<String>>,
}

impl AppState {
    fn update_config(&self, change: impl FnOnce(&mut Config)) -> Result<(), String> {
        let mut config = self.config.lock().unwrap();
        change(&mut config);
        config.save(&self.config_path)
    }
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct Snapshot {
    url: Option<String>,
    host: Option<String>,
    browsers: Vec<Browser>,
    config: Config,
    is_default: bool,
}

/// First argument that is a web link or an HTML file (Windows passes file paths).
fn url_from_args(args: &[String]) -> Option<String> {
    args.iter().skip(1).find_map(|arg| {
        let lower = arg.to_ascii_lowercase();
        if ["http://", "https://", "file://"].iter().any(|p| lower.starts_with(p)) {
            return Some(arg.clone());
        }
        let path = std::path::Path::new(arg);
        let is_html = path
            .extension()
            .is_some_and(|e| ["html", "htm", "xhtml"].iter().any(|x| e.eq_ignore_ascii_case(x)));
        if is_html && path.exists() {
            let absolute = std::fs::canonicalize(path).ok()?;
            return url::Url::from_file_path(absolute).ok().map(String::from);
        }
        None
    })
}

fn handle_url(app: &AppHandle, url: String) {
    let state = app.state::<AppState>();
    let rule = host_of(&url).and_then(|host| state.config.lock().unwrap().rule_for(&host).map(str::to_string));
    if let Some(target) = rule {
        match browsers::open(&state.browsers.lock().unwrap(), &target, &url) {
            Ok(()) => return,
            Err(e) => log::warn!("Rule for {url} failed, showing picker: {e}"),
        }
    }
    *state.pending.lock().unwrap() = Some(url.clone());
    // The picker sizes itself to its content, then calls `present_picker`.
    let _ = app.emit_to("picker", "open-url", url);
}

fn hide_picker(app: &AppHandle) {
    if let Some(window) = app.get_webview_window("picker") {
        let _ = window.hide();
    }
    *app.state::<AppState>().pending.lock().unwrap() = None;
}

fn show_settings(app: &AppHandle) {
    if let Some(window) = app.get_webview_window("settings") {
        let _ = window.unminimize();
        let _ = window.show();
        let _ = window.set_focus();
        return;
    }
    let built = WebviewWindowBuilder::new(app, "settings", WebviewUrl::App("index.html#settings".into()))
        .title("Browser Picker")
        .inner_size(560.0, 640.0)
        .min_inner_size(440.0, 420.0)
        .center()
        .focused(true)
        .build();
    if let Err(e) = built {
        log::error!("Could not open settings: {e}");
    }
}

#[tauri::command]
fn snapshot(state: tauri::State<AppState>) -> Snapshot {
    let url = state.pending.lock().unwrap().clone();
    Snapshot {
        host: url.as_deref().and_then(host_of),
        url,
        browsers: state.browsers.lock().unwrap().clone(),
        config: state.config.lock().unwrap().clone(),
        is_default: browsers::is_default(),
    }
}

/// Places the picker under the cursor, kept inside the screen's work area.
#[tauri::command]
fn present_picker(app: AppHandle, height: f64) -> Result<(), String> {
    let window = app.get_webview_window("picker").ok_or("Picker window is missing")?;
    window.set_size(LogicalSize::new(PICKER_WIDTH, height)).map_err(|e| e.to_string())?;

    let placed = app.cursor_position().ok().and_then(|cursor| {
        let monitor = app.monitor_from_point(cursor.x, cursor.y).ok().flatten()?;
        let scale = monitor.scale_factor();
        let area = monitor.work_area();
        let (w, h) = ((PICKER_WIDTH * scale) as i32, (height * scale) as i32);
        let (left, top) = (area.position.x, area.position.y);
        let right = (left + area.size.width as i32 - w).max(left);
        let bottom = (top + area.size.height as i32 - h).max(top);
        let x = (cursor.x as i32 - w / 2).clamp(left, right);
        let y = (cursor.y as i32 - (24.0 * scale) as i32).clamp(top, bottom);
        window.set_position(PhysicalPosition::new(x, y)).ok()
    });
    if placed.is_none() {
        let _ = window.center();
    }
    window.show().map_err(|e| e.to_string())?;
    window.set_focus().map_err(|e| e.to_string())
}

#[tauri::command]
fn open_in(app: AppHandle, state: tauri::State<AppState>, target: String, url: String, remember: bool) -> Result<(), String> {
    browsers::open(&state.browsers.lock().unwrap(), &target, &url)?;
    if remember && let Some(host) = host_of(&url) {
        state.update_config(|c| c.set_rule(host, target))?;
    }
    hide_picker(&app);
    Ok(())
}

#[tauri::command]
fn dismiss(app: AppHandle) {
    hide_picker(&app);
}

#[tauri::command]
fn set_hidden(state: tauri::State<AppState>, id: String, hidden: bool) -> Result<(), String> {
    state.update_config(|c| {
        c.hidden.retain(|h| *h != id);
        if hidden {
            c.hidden.push(id);
        }
    })
}

#[tauri::command]
fn remove_rule(state: tauri::State<AppState>, host: String) -> Result<(), String> {
    state.update_config(|c| c.rules.retain(|r| r.host != host))
}

#[tauri::command]
fn refresh_browsers(state: tauri::State<AppState>) {
    *state.browsers.lock().unwrap() = browsers::discover();
}

#[tauri::command]
fn make_default() -> Result<(), String> {
    browsers::make_default()
}

#[tauri::command]
fn open_settings(app: AppHandle) {
    hide_picker(&app);
    show_settings(&app);
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    let app = tauri::Builder::default()
        .plugin(tauri_plugin_single_instance::init(|app, args, _cwd| match url_from_args(&args) {
            Some(url) => handle_url(app, url),
            None => show_settings(app),
        }))
        .plugin(tauri_plugin_log::Builder::new().level(log::LevelFilter::Info).build())
        .invoke_handler(tauri::generate_handler![
            snapshot,
            present_picker,
            open_in,
            dismiss,
            set_hidden,
            remove_rule,
            refresh_browsers,
            make_default,
            open_settings,
        ])
        .setup(|app| {
            #[cfg(target_os = "macos")]
            app.set_activation_policy(tauri::ActivationPolicy::Accessory);

            let config_path = app.path().app_config_dir()?.join("config.json");
            let first_run = !config_path.exists();
            app.manage(AppState {
                browsers: Mutex::new(browsers::discover()),
                config: Mutex::new(Config::load(&config_path)),
                config_path,
                pending: Mutex::new(None),
            });

            let settings = MenuItem::with_id(app, "settings", "Settings…", true, None::<&str>)?;
            let quit = MenuItem::with_id(app, "quit", "Quit Browser Picker", true, None::<&str>)?;
            let menu = Menu::with_items(app, &[&settings, &PredefinedMenuItem::separator(app)?, &quit])?;
            let mut tray = TrayIconBuilder::with_id("tray")
                .tooltip("Browser Picker")
                .menu(&menu)
                .on_menu_event(|app, event| match event.id.as_ref() {
                    "settings" => show_settings(app),
                    "quit" => app.exit(0),
                    _ => {}
                });
            if let Some(icon) = app.default_window_icon() {
                tray = tray.icon(icon.clone());
            }
            tray.build(app)?;

            // Windows and Linux pass the link on the command line; macOS sends
            // it as an `Opened` event once the app is running.
            let args: Vec<String> = std::env::args().collect();
            if let Some(url) = url_from_args(&args) {
                handle_url(app.handle(), url);
            } else if first_run || cfg!(not(target_os = "macos")) {
                show_settings(app.handle());
            }
            Ok(())
        })
        .on_window_event(|window, event| match event {
            // Clicking elsewhere dismisses the picker, like a menu.
            WindowEvent::Focused(false) if window.label() == "picker" => hide_picker(window.app_handle()),
            // Keep running in the tray when settings is closed.
            WindowEvent::CloseRequested { api, .. } if window.label() == "picker" => {
                api.prevent_close();
                hide_picker(window.app_handle());
            }
            _ => {}
        })
        .build(tauri::generate_context!())
        .expect("error while building Browser Picker");

    app.run(|app, event| match event {
        #[cfg(target_os = "macos")]
        RunEvent::Opened { urls } => {
            for url in urls {
                handle_url(app, url.to_string());
            }
        }
        #[cfg(target_os = "macos")]
        RunEvent::Reopen { .. } => show_settings(app),
        // Closing the settings window must not quit the resident app.
        RunEvent::ExitRequested { api, code: None, .. } => api.prevent_exit(),
        _ => {}
    });
}
