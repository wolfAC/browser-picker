//! Updates from GitHub releases (`latest.json`, see scripts/release.sh).
//!
//! Checks once when the app launches, and again whenever the user asks. A found update is
//! only reported; it is downloaded and installed when the user clicks Update in Settings.

use std::{
    sync::{
        Mutex,
        atomic::{AtomicBool, Ordering},
    },
    thread,
};

use serde::Serialize;
use tauri::{AppHandle, Emitter, Manager};
use tauri_plugin_updater::UpdaterExt;

#[derive(Debug, Clone, Default, Serialize)]
#[serde(tag = "state", rename_all = "camelCase")]
pub enum Status {
    #[default]
    Idle,
    Checking,
    UpToDate,
    Available { version: String },
    Downloading { version: String },
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

/// Runs `work` unless another check or install is in progress.
fn exclusive(app: &AppHandle, work: impl FnOnce()) {
    let updates = app.state::<Updates>();
    if updates.busy.swap(true, Ordering::SeqCst) {
        return;
    }
    work();
    updates.busy.store(false, Ordering::SeqCst);
}

/// The launch check. Development builds never check.
pub fn start(app: &AppHandle) {
    if cfg!(debug_assertions) {
        return;
    }
    let app = app.clone();
    thread::spawn(move || check_now(&app));
}

/// Looks for a newer version on the calling thread and reports it, without installing.
pub fn check_now(app: &AppHandle) {
    if cfg!(debug_assertions) {
        set(app, Status::Failed { message: "Updates are off in development builds.".into() });
        return;
    }
    exclusive(app, || {
        set(app, Status::Checking);
        let result = tauri::async_runtime::block_on(async {
            app.updater().map_err(|e| e.to_string())?.check().await.map_err(|e| e.to_string())
        });
        match result {
            Ok(Some(update)) => {
                log::info!("Update {} available", update.version);
                set(app, Status::Available { version: update.version });
            }
            Ok(None) => set(app, Status::UpToDate),
            Err(message) => {
                log::warn!("Update check failed: {message}");
                set(app, Status::Failed { message });
            }
        }
    });
}

/// Downloads and installs the latest version, then restarts into it.
pub fn install_now(app: &AppHandle) {
    exclusive(app, || {
        let result = tauri::async_runtime::block_on(async {
            let update = app.updater().map_err(|e| e.to_string())?.check().await.map_err(|e| e.to_string())?;
            let Some(update) = update else { return Ok(false) };
            set(app, Status::Downloading { version: update.version.clone() });
            // On Windows this hands over to the installer, which closes the app itself.
            update.download_and_install(|_, _| {}, || {}).await.map_err(|e| e.to_string())?;
            Ok::<_, String>(true)
        });
        match result {
            Ok(true) => app.restart(),
            Ok(false) => set(app, Status::UpToDate),
            Err(message) => {
                log::warn!("Update failed: {message}");
                set(app, Status::Failed { message: format!("Update failed: {message}") });
            }
        }
    });
}
