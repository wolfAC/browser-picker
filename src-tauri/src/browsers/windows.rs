//! Windows: browsers register under `Clients\StartMenuInternet`.

use std::{
    os::windows::process::CommandExt,
    path::{Path, PathBuf},
    process::Command,
};

use winreg::{
    RegKey,
    enums::{HKEY_CURRENT_USER, HKEY_LOCAL_MACHINE, KEY_READ},
};

use super::{Family, Installed, Known, spawn};

const SELF_KEY: &str = "BrowserPicker";
const PROG_ID: &str = "BrowserPickerURL";
const CLIENTS: &str = r"Software\Clients\StartMenuInternet";

pub fn installed() -> Vec<Installed> {
    let roots = [
        (HKEY_CURRENT_USER, CLIENTS),
        (HKEY_LOCAL_MACHINE, CLIENTS),
        (HKEY_LOCAL_MACHINE, r"Software\WOW6432Node\Clients\StartMenuInternet"),
    ];
    let mut found: Vec<Installed> = Vec::new();
    for (hive, path) in roots {
        let Ok(clients) = RegKey::predef(hive).open_subkey_with_flags(path, KEY_READ) else { continue };
        for key_name in clients.enum_keys().flatten() {
            if key_name.eq_ignore_ascii_case(SELF_KEY) {
                continue;
            }
            let Ok(client) = clients.open_subkey(&key_name) else { continue };
            let Some(exe) = client
                .open_subkey(r"shell\open\command")
                .and_then(|k| k.get_value::<String, _>(""))
                .ok()
                .and_then(|cmd| exe_from_command(&cmd))
            else {
                continue;
            };
            if found.iter().any(|f| Path::new(&f.launch[0]) == exe) || !exe.exists() {
                continue;
            }
            let name = client.get_value::<String, _>("").unwrap_or_else(|_| key_name.clone());
            let file = exe.file_name().map(|f| f.to_string_lossy().to_lowercase()).unwrap_or_default();
            found.push(Installed {
                id: key_name,
                match_key: file,
                name,
                icon: None,
                launch: vec![exe.to_string_lossy().into_owned()],
            });
        }
    }
    found
}

/// `"C:\Program Files\...\chrome.exe" --flag` -> `C:\Program Files\...\chrome.exe`
fn exe_from_command(command: &str) -> Option<PathBuf> {
    let command = command.trim();
    if let Some(rest) = command.strip_prefix('"') {
        return rest.split('"').next().map(PathBuf::from);
    }
    let end = command.to_lowercase().find(".exe")? + 4;
    Some(PathBuf::from(&command[..end]))
}

pub fn matches(known: &Known, exe_name: &str) -> bool {
    known.win_exes.iter().any(|e| e.eq_ignore_ascii_case(exe_name))
}

pub fn data_dirs(known: &Known) -> Vec<PathBuf> {
    let base = match known.family {
        Family::Chromium => dirs::data_local_dir(),
        Family::Firefox => dirs::data_dir(),
    };
    let Some(base) = base else { return Vec::new() };
    known.win_data.iter().map(|p| base.join(p)).collect()
}

pub fn launch(launch: &[String], args: &[String], url: &str) -> std::io::Result<()> {
    const DETACHED_PROCESS: u32 = 0x0000_0008;
    let mut command = Command::new(&launch[0]);
    command.args(args).arg(url).creation_flags(DETACHED_PROCESS);
    spawn(command)
}

pub fn is_default() -> bool {
    RegKey::predef(HKEY_CURRENT_USER)
        .open_subkey(r"Software\Microsoft\Windows\Shell\Associations\UrlAssociations\https\UserChoice")
        .and_then(|k| k.get_value::<String, _>("ProgId"))
        .is_ok_and(|id| id == PROG_ID)
}

/// Registers us as a browser for the current user, then opens the Default Apps
/// page: Windows only lets the user pick the default browser there.
pub fn make_default() -> Result<(), String> {
    register().map_err(|e| format!("Could not register Browser Picker: {e}"))?;
    let page = format!("ms-settings:defaultapps?registeredAppUser={SELF_KEY}");
    spawn({
        let mut c = Command::new("explorer.exe");
        c.arg(page);
        c
    })
    .map_err(|e| e.to_string())
}

fn register() -> std::io::Result<()> {
    let exe = std::env::current_exe()?.to_string_lossy().into_owned();
    let icon = format!("{exe},0");
    let open = format!("\"{exe}\" \"%1\"");
    let hkcu = RegKey::predef(HKEY_CURRENT_USER);

    let (class, _) = hkcu.create_subkey(format!(r"Software\Classes\{PROG_ID}"))?;
    class.set_value("", &"Browser Picker URL")?;
    class.set_value("URL Protocol", &"")?;
    class.create_subkey("DefaultIcon")?.0.set_value("", &icon)?;
    class.create_subkey(r"shell\open\command")?.0.set_value("", &open)?;

    let client_path = format!(r"{CLIENTS}\{SELF_KEY}");
    let (client, _) = hkcu.create_subkey(&client_path)?;
    client.set_value("", &"Browser Picker")?;
    client.create_subkey("DefaultIcon")?.0.set_value("", &icon)?;
    client.create_subkey(r"shell\open\command")?.0.set_value("", &format!("\"{exe}\""))?;

    let (caps, _) = client.create_subkey("Capabilities")?;
    caps.set_value("ApplicationName", &"Browser Picker")?;
    caps.set_value("ApplicationDescription", &"Pick a browser and profile for every link")?;
    caps.set_value("ApplicationIcon", &icon)?;
    let (urls, _) = caps.create_subkey("URLAssociations")?;
    for scheme in ["http", "https"] {
        urls.set_value(scheme, &PROG_ID)?;
    }
    let (files, _) = caps.create_subkey("FileAssociations")?;
    for ext in [".htm", ".html", ".xhtml"] {
        files.set_value(ext, &PROG_ID)?;
    }

    let (registered, _) = hkcu.create_subkey(r"Software\RegisteredApplications")?;
    registered.set_value(SELF_KEY, &format!(r"{client_path}\Capabilities"))?;
    Ok(())
}
