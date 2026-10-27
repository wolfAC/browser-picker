//! Background auto-update from GitHub releases (`latest.json`, see scripts/release.sh).
//!
//! Checks shortly after launch and then every few hours. A found update is
//! downloaded and installed straight away; the app restarts into it once no
//! window is open, so it never interrupts a pick.

use std::{
    sync::{
        Mutex,
        atomic::{AtomicBool, Ordering},
    },
    thread,
    time::Duration,
};

use serde::Serialize;
use tauri::{AppHandle, Emitter, Manager};
use tauri_plugin_updater::UpdaterExt;

const FIRST_CHECK: Duration = Duration::from_secs(30);
const CHECK_EVERY: Duration = Duration::from_secs(6 * 60 * 60);
const IDLE_POLL: Duration = Duration::from_secs(30);

#[derive(Debug, Clone, Default, Serialize)]
#[serde(tag = "state", rename_all = "camelCase")]
pub enum Status {
    #[default]
    Idle,
    Checking,
    UpToDate,
    Downloading { version: String },
    Ready { version: String },
    Failed { message: String },
}

#[derive(Default)]
pub struct Updates {
    status: Mutex<Status>,
    busy: AtomicBool,
}

#[derive(Clone, Serialize)]
pub struct Info {
    version: String,
    status: Status,
}

pub fn info(app: &AppHandle) -> Info {
    Info {
        version: app.package_info().version.to_string(),
        status: app.state::<Updates>().status.lock().unwrap().clone(),
    }
}

fn set(app: &AppHandle, status: Status) {
    *app.state::<Updates>().status.lock().unwrap() = status;
    let _ = app.emit("update-status", info(app));
}

/// Starts the periodic check. Development builds never update themselves.
pub fn start(app: &AppHandle) {
    if cfg!(debug_assertions) {
        return;
    }
    let app = app.clone();
    thread::spawn(move || {
        thread::sleep(FIRST_CHECK);
        loop {
            check_now(&app);
            thread::sleep(CHECK_EVERY);
        }
    });
}

/// Checks, downloads and installs on the calling thread; a no-op if a check is already running.
pub fn check_now(app: &AppHandle) {
    if cfg!(debug_assertions) {
        set(app, Status::Failed { message: "Updates are off in development builds.".into() });
        return;
    }
    let updates = app.state::<Updates>();
    if matches!(*updates.status.lock().unwrap(), Status::Ready { .. }) || updates.busy.swap(true, Ordering::SeqCst) {
        return;
    }
    let result = tauri::async_runtime::block_on(install(app));
    updates.busy.store(false, Ordering::SeqCst);
    match result {
        Ok(Some(version)) => {
            log::info!("Installed update {version}; restarting when idle");
            set(app, Status::Ready { version });
            restart_when_idle(app);
        }
        Ok(None) => set(app, Status::UpToDate),
        Err(message) => {
            log::warn!("Update check failed: {message}");
            set(app, Status::Failed { message });
        }
    }
}

async fn install(app: &AppHandle) -> Result<Option<String>, String> {
    set(app, Status::Checking);
    let update = app.updater().map_err(|e| e.to_string())?.check().await.map_err(|e| e.to_string())?;
    let Some(update) = update else { return Ok(None) };
    set(app, Status::Downloading { version: update.version.clone() });
    // On Windows this hands over to the installer, which closes the app itself.
    update.download_and_install(|_, _| {}, || {}).await.map_err(|e| e.to_string())?;
    Ok(Some(update.version))
}

fn restart_when_idle(app: &AppHandle) {
    let app = app.clone();
    thread::spawn(move || {
        loop {
            let busy = app.webview_windows().values().any(|w| w.is_visible().unwrap_or(false));
            if !busy {
                app.restart();
            }
            thread::sleep(IDLE_POLL);
        }
    });
}
