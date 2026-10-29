//! Installed-browser discovery and launching.
//!
//! Each OS module lists the browsers registered for `https` links (`installed`),
//! knows where a known browser keeps its profile data (`data_dir`), and spawns a
//! browser with extra arguments (`launch`). Profile reading is shared: every
//! Chromium fork stores profiles in `Local State`, every Firefox fork in
//! `profiles.ini`.

mod chromium;
mod firefox;

#[cfg(target_os = "linux")]
mod linux;
#[cfg(target_os = "macos")]
mod macos;
#[cfg(target_os = "windows")]
mod windows;

#[cfg(target_os = "linux")]
use linux as platform;
#[cfg(target_os = "macos")]
use macos as platform;
#[cfg(target_os = "windows")]
use windows as platform;

use std::{collections::HashSet, path::Path, process::Command};

use base64::{Engine, engine::general_purpose::STANDARD};
use serde::Serialize;

pub use platform::{is_default, make_default};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Family {
    Chromium,
    Firefox,
}

/// A browser whose profiles we know how to read. Paths are per OS:
/// - `mac_data`: relative to `~/Library/Application Support`
/// - `win_data`: relative to `%LOCALAPPDATA%` (Chromium) or `%APPDATA%` (Firefox)
/// - `linux_data`: relative to `~`
#[allow(dead_code)] // each OS reads only its own fields
pub struct Known {
    pub key: &'static str,
    pub family: Family,
    pub mac_ids: &'static [&'static str],
    pub win_exes: &'static [&'static str],
    pub linux_ids: &'static [&'static str],
    pub mac_data: &'static [&'static str],
    pub win_data: &'static [&'static str],
    pub linux_data: &'static [&'static str],
}

pub const KNOWN: &[Known] = &[
    Known {
        key: "chrome",
        family: Family::Chromium,
        mac_ids: &["com.google.Chrome"],
        win_exes: &["chrome.exe"],
        linux_ids: &["google-chrome.desktop", "com.google.Chrome.desktop"],
        mac_data: &["Google/Chrome"],
        win_data: &["Google/Chrome/User Data"],
        linux_data: &[".config/google-chrome", ".var/app/com.google.Chrome/config/google-chrome"],
    },
    Known {
        key: "chrome-beta",
        family: Family::Chromium,
        mac_ids: &["com.google.Chrome.beta"],
        win_exes: &[],
        linux_ids: &["google-chrome-beta.desktop"],
        mac_data: &["Google/Chrome Beta"],
        win_data: &["Google/Chrome Beta/User Data"],
        linux_data: &[".config/google-chrome-beta"],
    },
    Known {
        key: "chrome-canary",
        family: Family::Chromium,
        mac_ids: &["com.google.Chrome.canary"],
        win_exes: &[],
        linux_ids: &["google-chrome-unstable.desktop"],
        mac_data: &["Google/Chrome Canary"],
        win_data: &["Google/Chrome SxS/User Data"],
        linux_data: &[".config/google-chrome-unstable"],
    },
    Known {
        key: "chromium",
        family: Family::Chromium,
        mac_ids: &["org.chromium.Chromium"],
        win_exes: &[],
        linux_ids: &["chromium.desktop", "chromium-browser.desktop", "org.chromium.Chromium.desktop"],
        mac_data: &["Chromium"],
        win_data: &["Chromium/User Data"],
        linux_data: &[
            ".config/chromium",
            "snap/chromium/common/chromium",
            ".var/app/org.chromium.Chromium/config/chromium",
        ],
    },
    Known {
        key: "brave",
        family: Family::Chromium,
        mac_ids: &["com.brave.Browser"],
        win_exes: &["brave.exe"],
        linux_ids: &["brave-browser.desktop", "com.brave.Browser.desktop"],
        mac_data: &["BraveSoftware/Brave-Browser"],
        win_data: &["BraveSoftware/Brave-Browser/User Data"],
        linux_data: &[
            ".config/BraveSoftware/Brave-Browser",
            ".var/app/com.brave.Browser/config/BraveSoftware/Brave-Browser",
        ],
    },
    Known {
        key: "edge",
        family: Family::Chromium,
        mac_ids: &["com.microsoft.edgemac"],
        win_exes: &["msedge.exe"],
        linux_ids: &["microsoft-edge.desktop", "com.microsoft.Edge.desktop"],
        mac_data: &["Microsoft Edge"],
        win_data: &["Microsoft/Edge/User Data"],
        linux_data: &[".config/microsoft-edge", ".var/app/com.microsoft.Edge/config/microsoft-edge"],
    },
    Known {
        key: "vivaldi",
        family: Family::Chromium,
        mac_ids: &["com.vivaldi.Vivaldi"],
        win_exes: &["vivaldi.exe"],
        linux_ids: &["vivaldi-stable.desktop", "com.vivaldi.Vivaldi.desktop"],
        mac_data: &["Vivaldi"],
        win_data: &["Vivaldi/User Data"],
        linux_data: &[".config/vivaldi"],
    },
    Known {
        key: "helium",
        family: Family::Chromium,
        mac_ids: &["net.imput.helium"],
        win_exes: &["helium.exe"],
        linux_ids: &["helium.desktop"],
        mac_data: &["net.imput.helium"],
        win_data: &["imput/Helium/User Data"],
        linux_data: &[".config/net.imput.helium"],
    },
    Known {
        key: "thorium",
        family: Family::Chromium,
        mac_ids: &["org.chromium.Thorium"],
        win_exes: &["thorium.exe"],
        linux_ids: &["thorium-browser.desktop"],
        mac_data: &["Thorium"],
        win_data: &["Thorium/User Data"],
        linux_data: &[".config/thorium"],
    },
    Known {
        key: "firefox",
        family: Family::Firefox,
        mac_ids: &["org.mozilla.firefox"],
        win_exes: &["firefox.exe"],
        linux_ids: &["firefox.desktop", "firefox_firefox.desktop", "org.mozilla.firefox.desktop"],
        mac_data: &["Firefox"],
        win_data: &["Mozilla/Firefox"],
        linux_data: &[
            ".mozilla/firefox",
            // Firefox 147+ uses the XDG config dir when there is no ~/.mozilla yet.
            ".config/mozilla/firefox",
            "snap/firefox/common/.mozilla/firefox",
            ".var/app/org.mozilla.firefox/.mozilla/firefox",
        ],
    },
    Known {
        key: "firefox-dev",
        family: Family::Firefox,
        mac_ids: &["org.mozilla.firefoxdeveloperedition"],
        win_exes: &[],
        linux_ids: &["firefox-developer-edition.desktop"],
        mac_data: &["Firefox"],
        win_data: &["Mozilla/Firefox"],
        linux_data: &[".mozilla/firefox"],
    },
    Known {
        key: "zen",
        family: Family::Firefox,
        mac_ids: &["app.zen-browser.zen"],
        win_exes: &["zen.exe"],
        linux_ids: &["zen.desktop", "zen-browser.desktop", "app.zen_browser.zen.desktop"],
        mac_data: &["zen"],
        win_data: &["zen"],
        linux_data: &[".zen", ".var/app/app.zen_browser.zen/.zen"],
    },
    Known {
        key: "librewolf",
        family: Family::Firefox,
        mac_ids: &["io.gitlab.librewolf-community", "org.mozilla.librewolf"],
        win_exes: &["librewolf.exe"],
        linux_ids: &["librewolf.desktop", "io.gitlab.librewolf-community.desktop"],
        mac_data: &["librewolf"],
        win_data: &["librewolf"],
        linux_data: &[".librewolf", ".var/app/io.gitlab.librewolf-community/.librewolf"],
    },
    Known {
        key: "floorp",
        family: Family::Firefox,
        mac_ids: &["one.ablaze.floorp"],
        win_exes: &["floorp.exe"],
        linux_ids: &["floorp.desktop", "one.ablaze.floorp.desktop"],
        mac_data: &["Floorp"],
        win_data: &["Floorp"],
        linux_data: &[".floorp", ".var/app/one.ablaze.floorp/.floorp"],
    },
    Known {
        key: "waterfox",
        family: Family::Firefox,
        mac_ids: &["net.waterfox.waterfox"],
        win_exes: &["waterfox.exe"],
        linux_ids: &["waterfox.desktop", "net.waterfox.waterfox.desktop"],
        mac_data: &["Waterfox"],
        win_data: &["Waterfox"],
        linux_data: &[".waterfox"],
    },
];

/// What an OS module reports for one registered browser.
pub struct Installed {
    /// Stable id for browsers we don't know (bundle id, registry key, desktop id).
    pub id: String,
    /// What `platform::matches` compares against a `Known` entry.
    pub match_key: String,
    pub name: String,
    pub icon: Option<String>,
    /// Platform launch data: app bundle path (macOS), exe path (Windows), Exec argv (Linux).
    pub launch: Vec<String>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Profile {
    pub id: String,
    pub name: String,
    pub detail: Option<String>,
    pub avatar: Option<String>,
    pub color: Option<String>,
    #[serde(skip)]
    args: Vec<String>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Browser {
    pub id: String,
    pub name: String,
    pub icon: Option<String>,
    pub profiles: Vec<Profile>,
    #[serde(skip)]
    launch: Vec<String>,
}

/// A profile as read from disk, before it is tied to a browser id.
pub struct RawProfile {
    pub key: String,
    pub name: String,
    pub detail: Option<String>,
    pub avatar: Option<String>,
    pub color: Option<String>,
    pub args: Vec<String>,
}

pub fn discover() -> Vec<Browser> {
    let mut seen = HashSet::new();
    let mut list: Vec<Browser> = platform::installed()
        .into_iter()
        .filter_map(|app| {
            let known = KNOWN.iter().find(|k| platform::matches(k, &app.match_key));
            let id = known.map_or(app.id, |k| k.key.to_string());
            if !seen.insert(id.clone()) {
                return None;
            }
            let profiles = known
                .map(read_profiles)
                .unwrap_or_default()
                .into_iter()
                .map(|p| Profile {
                    id: format!("{id}/{}", p.key),
                    name: p.name,
                    detail: p.detail,
                    avatar: p.avatar,
                    color: p.color,
                    args: p.args,
                })
                .collect();
            Some(Browser { id, name: app.name, icon: app.icon, profiles, launch: app.launch })
        })
        .collect();
    list.sort_by_key(|b| b.name.to_lowercase());
    list
}

/// Reads the first data dir that has profiles; a browser can leave an empty
/// legacy dir behind (Firefox 147+ recreates `~/.mozilla` next to the XDG one).
fn read_profiles(known: &Known) -> Vec<RawProfile> {
    platform::data_dirs(known)
        .iter()
        .map(|dir| match known.family {
            Family::Chromium => chromium::profiles(dir),
            Family::Firefox => firefox::profiles(dir),
        })
        .find(|profiles| !profiles.is_empty())
        .unwrap_or_default()
}

/// Opens `url` with a browser (`"chrome"`) or one of its profiles (`"chrome/Profile 1"`).
pub fn open(browsers: &[Browser], target: &str, url: &str) -> Result<(), String> {
    let (browser_id, profile) = match target.split_once('/') {
        Some((b, _)) => (b, Some(target)),
        None => (target, None),
    };
    let browser = browsers
        .iter()
        .find(|b| b.id == browser_id)
        .ok_or_else(|| format!("Browser {browser_id} is no longer installed"))?;
    let args = match profile {
        Some(id) => {
            &browser
                .profiles
                .iter()
                .find(|p| p.id == id)
                .ok_or_else(|| format!("Profile {id} no longer exists"))?
                .args
        }
        None => &Vec::new(),
    };
    platform::launch(&browser.launch, args, url).map_err(|e| format!("Could not start {}: {e}", browser.name))
}

/// Spawns a command without blocking, reaping it on a background thread.
fn spawn(mut command: Command) -> std::io::Result<()> {
    let mut child = command.spawn()?;
    std::thread::spawn(move || {
        let _ = child.wait();
    });
    Ok(())
}

pub fn image_data_url(path: &Path) -> Option<String> {
    let mime = match path.extension()?.to_str()?.to_ascii_lowercase().as_str() {
        "png" => "image/png",
        "svg" => "image/svg+xml",
        "jpg" | "jpeg" => "image/jpeg",
        "ico" => "image/x-icon",
        _ => return None,
    };
    let bytes = std::fs::read(path).ok()?;
    Some(format!("data:{mime};base64,{}", STANDARD.encode(bytes)))
}

#[cfg(test)]
mod tests {
    #[test]
    #[ignore = "prints this machine's browsers; run with --ignored --nocapture"]
    fn list_installed() {
        for b in super::discover() {
            println!("{} [{}] icon={}", b.name, b.id, b.icon.is_some());
            for p in b.profiles {
                println!("    {} [{}] {:?} avatar={} color={:?} args={:?}", p.name, p.id, p.detail, p.avatar.is_some(), p.color, p.args);
            }
        }
    }
}
