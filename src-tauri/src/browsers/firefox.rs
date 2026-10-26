//! Firefox forks list their profiles in `<data dir>/profiles.ini`.

use std::path::Path;

use super::RawProfile;

pub fn profiles(dir: &Path) -> Vec<RawProfile> {
    let Ok(text) = std::fs::read_to_string(dir.join("profiles.ini")) else {
        return Vec::new();
    };

    let mut profiles = Vec::new();
    let mut in_profile = false;
    let mut name: Option<String> = None;
    let flush = |name: &mut Option<String>, profiles: &mut Vec<RawProfile>| {
        if let Some(name) = name.take() {
            profiles.push(RawProfile {
                key: name.clone(),
                args: vec!["-P".into(), name.clone()],
                name,
                detail: None,
                avatar: None,
                color: None,
            });
        }
    };

    for line in text.lines().map(str::trim) {
        if let Some(section) = line.strip_prefix('[').and_then(|l| l.strip_suffix(']')) {
            flush(&mut name, &mut profiles);
            in_profile = section.starts_with("Profile");
        } else if in_profile && let Some(value) = line.strip_prefix("Name=") {
            name = Some(value.to_string());
        }
    }
    flush(&mut name, &mut profiles);
    profiles
}
