//! macOS: browsers are app bundles that declare the `https` URL scheme.

use std::{
    fs::File,
    io::BufReader,
    path::{Path, PathBuf},
    process::Command,
};

use core_foundation::{
    base::TCFType,
    string::{CFString, CFStringRef},
};
use base64::{Engine, engine::general_purpose::STANDARD};
use plist::{Dictionary, Value};

use super::{Installed, Known, spawn};

const SELF_ID: &str = "dev.browserpicker.desktop";

#[link(name = "CoreServices", kind = "framework")]
unsafe extern "C" {
    fn LSSetDefaultHandlerForURLScheme(scheme: CFStringRef, bundle_id: CFStringRef) -> i32;
    fn LSCopyDefaultHandlerForURLScheme(scheme: CFStringRef) -> CFStringRef;
}

pub fn installed() -> Vec<Installed> {
    let mut roots = vec![PathBuf::from("/Applications")];
    if let Some(home) = dirs::home_dir() {
        roots.push(home.join("Applications"));
    }

    let mut apps = Vec::new();
    for root in roots {
        let Ok(entries) = std::fs::read_dir(&root) else { continue };
        for path in entries.flatten().map(|e| e.path()) {
            if is_app(&path) {
                apps.push(path);
            } else if path.is_dir() {
                // One level of folders, e.g. /Applications/Setapp.
                let Ok(inner) = std::fs::read_dir(&path) else { continue };
                apps.extend(inner.flatten().map(|e| e.path()).filter(|p| is_app(p)));
            }
        }
    }

    apps.into_iter().filter_map(|path| inspect(&path)).collect()
}

fn is_app(path: &Path) -> bool {
    path.extension().is_some_and(|e| e == "app")
}

fn inspect(app: &Path) -> Option<Installed> {
    let info = Value::from_file(app.join("Contents/Info.plist")).ok()?.into_dictionary()?;
    let id = info.get("CFBundleIdentifier")?.as_string()?.to_string();
    if id.eq_ignore_ascii_case(SELF_ID) || !handles_https(&info) {
        return None;
    }
    let name = app.file_stem()?.to_string_lossy().into_owned();
    Some(Installed {
        match_key: id.clone(),
        id,
        name,
        icon: icon(app, &info),
        launch: vec![app.to_string_lossy().into_owned()],
    })
}

fn handles_https(info: &Dictionary) -> bool {
    info.get("CFBundleURLTypes")
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
        .filter_map(|t| t.as_dictionary()?.get("CFBundleURLSchemes")?.as_array())
        .flatten()
        .filter_map(Value::as_string)
        .any(|s| s.eq_ignore_ascii_case("https"))
}

fn icon(app: &Path, info: &Dictionary) -> Option<String> {
    let resources = app.join("Contents/Resources");
    let file = info.get("CFBundleIconFile").and_then(Value::as_string).unwrap_or("AppIcon");
    let path = if file.ends_with(".icns") { resources.join(file) } else { resources.join(format!("{file}.icns")) };
    let family = icns::IconFamily::read(BufReader::new(File::open(path).ok()?)).ok()?;

    // Smallest icon that is still sharp at 2x in a 32pt row.
    let mut types = family.available_icons();
    types.sort_by_key(|t| t.pixel_width());
    let kind = types.iter().find(|t| t.pixel_width() >= 64).or(types.last())?;
    let image = family.get_icon_with_type(*kind).ok()?;
    let mut png = Vec::new();
    image.write_png(&mut png).ok()?;
    Some(format!("data:image/png;base64,{}", STANDARD.encode(png)))
}

pub fn matches(known: &Known, bundle_id: &str) -> bool {
    known.mac_ids.iter().any(|id| id.eq_ignore_ascii_case(bundle_id))
}

pub fn data_dirs(known: &Known) -> Vec<PathBuf> {
    let Some(base) = dirs::data_dir() else { return Vec::new() };
    known.mac_data.iter().map(|p| base.join(p)).collect()
}

pub fn launch(launch: &[String], args: &[String], url: &str) -> std::io::Result<()> {
    let mut command = Command::new("open");
    if args.is_empty() {
        command.arg("-a").args(launch).arg(url);
    } else {
        // `-n` starts a fresh process so `--args` reaches it; Chromium and Firefox
        // hand the request to their already-running instance and exit.
        command.arg("-n").arg("-a").args(launch).arg("--args").args(args).arg(url);
    }
    spawn(command)
}

pub fn is_default() -> bool {
    let scheme = CFString::new("https");
    let handler = unsafe { LSCopyDefaultHandlerForURLScheme(scheme.as_concrete_TypeRef()) };
    if handler.is_null() {
        return false;
    }
    let handler = unsafe { CFString::wrap_under_create_rule(handler) };
    handler.to_string().eq_ignore_ascii_case(SELF_ID)
}

/// Asks macOS to make us the default browser; the system shows a confirmation dialog.
pub fn make_default() -> Result<(), String> {
    let bundle = CFString::new(SELF_ID);
    for scheme in ["http", "https"] {
        let scheme = CFString::new(scheme);
        let status =
            unsafe { LSSetDefaultHandlerForURLScheme(scheme.as_concrete_TypeRef(), bundle.as_concrete_TypeRef()) };
        if status != 0 {
            return Err(format!(
                "macOS refused (error {status}). Run the installed Browser Picker.app, not the dev build."
            ));
        }
    }
    Ok(())
}
