//! Linux: browsers are `.desktop` entries that handle `x-scheme-handler/https`.

use std::{
    collections::{HashMap, HashSet},
    path::{Path, PathBuf},
    process::Command,
};

use super::{Installed, Known, image_data_url, spawn};

const SELF_ID: &str = "browser-picker.desktop";
/// Ours from `make_default`, and the one the deb/rpm packages install (named after the product).
const SELF_IDS: &[&str] = &[SELF_ID, "Browser Picker.desktop"];

fn data_roots() -> Vec<PathBuf> {
    let home = dirs::home_dir().unwrap_or_default();
    let mut roots = vec![dirs::data_dir().unwrap_or_else(|| home.join(".local/share"))];
    let system = std::env::var("XDG_DATA_DIRS").unwrap_or_else(|_| "/usr/local/share:/usr/share".into());
    roots.extend(system.split(':').filter(|s| !s.is_empty()).map(PathBuf::from));
    roots.push(home.join(".local/share/flatpak/exports/share"));
    roots.push("/var/lib/flatpak/exports/share".into());
    roots.push("/var/lib/snapd/desktop".into());
    let mut seen = HashSet::new();
    roots.retain(|r| seen.insert(r.clone()));
    roots
}

pub fn installed() -> Vec<Installed> {
    let roots = data_roots();
    let mut seen = HashSet::new();
    let mut found = Vec::new();
    for root in &roots {
        let Ok(entries) = std::fs::read_dir(root.join("applications")) else { continue };
        for path in entries.flatten().map(|e| e.path()) {
            let Some(id) = path.file_name().map(|f| f.to_string_lossy().into_owned()) else { continue };
            // Earlier roots (the user's own) shadow later ones with the same id.
            if !id.ends_with(".desktop") || SELF_IDS.contains(&id.as_str()) || !seen.insert(id.clone()) {
                continue;
            }
            if let Some(app) = inspect(&path, id, &roots) {
                found.push(app);
            }
        }
    }
    found
}

fn desktop_entry(path: &Path) -> Option<HashMap<String, String>> {
    let text = std::fs::read_to_string(path).ok()?;
    let mut in_entry = false;
    let mut fields = HashMap::new();
    for line in text.lines().map(str::trim) {
        if line.starts_with('[') {
            in_entry = line == "[Desktop Entry]";
        } else if in_entry && let Some((k, v)) = line.split_once('=') {
            fields.entry(k.trim().to_string()).or_insert_with(|| v.trim().to_string());
        }
    }
    Some(fields)
}

fn inspect(path: &Path, id: String, roots: &[PathBuf]) -> Option<Installed> {
    let entry = desktop_entry(path)?;
    let handles = entry.get("MimeType").is_some_and(|m| m.split(';').any(|t| t == "x-scheme-handler/https"));
    if !handles || entry.get("Hidden").is_some_and(|h| h == "true") {
        return None;
    }
    let exec = exec_argv(entry.get("Exec")?)?;
    if is_self(&exec[0]) {
        return None;
    }
    Some(Installed {
        match_key: id.clone(),
        name: entry.get("Name").cloned().unwrap_or_else(|| id.trim_end_matches(".desktop").to_string()),
        icon: entry.get("Icon").and_then(|i| icon(i, roots)),
        id,
        launch: exec,
    })
}

/// Whether an `Exec=` program is this app, e.g. a renamed copy of our desktop entry.
/// Packaged entries name the binary bare (`browser-picker`), so it is looked up on `PATH`.
fn is_self(program: &str) -> bool {
    let program = Path::new(program);
    let resolved = if program.components().count() > 1 {
        Some(program.to_path_buf())
    } else {
        std::env::var_os("PATH")
            .and_then(|path| std::env::split_paths(&path).map(|d| d.join(program)).find(|p| p.is_file()))
    };
    let Some(resolved) = resolved.and_then(|p| p.canonicalize().ok()) else { return false };
    // AppImages run from a temporary mount; the stable path is in $APPIMAGE.
    let me = [std::env::current_exe().ok(), std::env::var_os("APPIMAGE").map(PathBuf::from)];
    me.into_iter().flatten().filter_map(|p| p.canonicalize().ok()).any(|p| p == resolved)
}

/// Splits an `Exec=` line and drops field codes (`%u`, `%U`, ...) and flatpak's
/// file-forwarding markers; the URL is appended at launch.
fn exec_argv(exec: &str) -> Option<Vec<String>> {
    let argv: Vec<String> = shlex::split(exec)?
        .into_iter()
        .filter(|a| !(a.len() == 2 && a.starts_with('%')))
        .filter(|a| !matches!(a.as_str(), "@@" | "@@u" | "--file-forwarding"))
        .map(|a| a.replace("%%", "%"))
        .collect();
    (!argv.is_empty()).then_some(argv)
}

fn icon(name: &str, roots: &[PathBuf]) -> Option<String> {
    let path = Path::new(name);
    if path.is_absolute() {
        return image_data_url(path);
    }
    const SIZES: &[&str] = &["128x128", "256x256", "96x96", "64x64", "48x48", "scalable"];
    for root in roots {
        for size in SIZES {
            for ext in ["png", "svg"] {
                let candidate = root.join(format!("icons/hicolor/{size}/apps/{name}.{ext}"));
                if let Some(url) = image_data_url(&candidate) {
                    return Some(url);
                }
            }
        }
        for ext in ["png", "svg"] {
            if let Some(url) = image_data_url(&root.join(format!("pixmaps/{name}.{ext}"))) {
                return Some(url);
            }
        }
    }
    None
}

pub fn matches(known: &Known, desktop_id: &str) -> bool {
    known.linux_ids.iter().any(|id| id.eq_ignore_ascii_case(desktop_id))
}

pub fn data_dirs(known: &Known) -> Vec<PathBuf> {
    let Some(home) = dirs::home_dir() else { return Vec::new() };
    known.linux_data.iter().map(|p| home.join(p)).collect()
}

pub fn launch(launch: &[String], args: &[String], url: &str) -> std::io::Result<()> {
    let mut command = Command::new(&launch[0]);
    command.args(&launch[1..]).args(args).arg(url);
    spawn(command)
}

pub fn is_default() -> bool {
    Command::new("xdg-settings")
        .args(["get", "default-web-browser"])
        .output()
        .is_ok_and(|o| SELF_IDS.contains(&String::from_utf8_lossy(&o.stdout).trim()))
}

/// Writes a user-level desktop entry pointing at this binary and makes it the
/// handler for web links and HTML files.
pub fn make_default() -> Result<(), String> {
    let exe = std::env::current_exe().map_err(|e| e.to_string())?;
    // AppImages run from a temporary mount; the stable path is in $APPIMAGE.
    let exe = std::env::var("APPIMAGE").map(PathBuf::from).unwrap_or(exe);
    let dir = dirs::data_dir().ok_or("No data directory")?.join("applications");
    std::fs::create_dir_all(&dir).map_err(|e| e.to_string())?;
    let entry = format!(
        "[Desktop Entry]\nType=Application\nName=Browser Picker\nComment=Pick a browser and profile for every link\n\
         Exec=\"{}\" %u\nIcon=browser-picker\nCategories=Network;WebBrowser;\nTerminal=false\nStartupNotify=false\n\
         MimeType=x-scheme-handler/http;x-scheme-handler/https;text/html;application/xhtml+xml;\n",
        exe.display()
    );
    std::fs::write(dir.join(SELF_ID), entry).map_err(|e| e.to_string())?;
    let _ = Command::new("update-desktop-database").arg(&dir).status();

    let ok = Command::new("xdg-settings")
        .args(["set", "default-web-browser", SELF_ID])
        .status()
        .is_ok_and(|s| s.success());
    let mime_ok = Command::new("xdg-mime")
        .args(["default", SELF_ID, "x-scheme-handler/http", "x-scheme-handler/https", "text/html"])
        .status()
        .is_ok_and(|s| s.success());
    if ok || mime_ok { Ok(()) } else { Err("xdg-settings and xdg-mime both failed".into()) }
}
