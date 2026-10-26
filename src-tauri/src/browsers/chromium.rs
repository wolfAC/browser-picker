//! Chromium forks list their profiles in `<user data dir>/Local State`.

use std::path::Path;

use serde_json::Value;

use super::{RawProfile, image_data_url};

const PICTURES: &[&str] = &["Google Profile Picture.png", "Edge Profile Picture.png"];

pub fn profiles(dir: &Path) -> Vec<RawProfile> {
    let Some(state) = std::fs::read_to_string(dir.join("Local State"))
        .ok()
        .and_then(|text| serde_json::from_str::<Value>(&text).ok())
    else {
        return Vec::new();
    };
    let Some(cache) = state["profile"]["info_cache"].as_object() else {
        return Vec::new();
    };

    // `profiles_order` is the order Chrome shows in its own profile menu.
    let mut keys: Vec<&str> = state["profile"]["profiles_order"]
        .as_array()
        .into_iter()
        .flatten()
        .filter_map(Value::as_str)
        .filter(|k| cache.contains_key(*k))
        .collect();
    let mut rest: Vec<&str> = cache.keys().map(String::as_str).filter(|k| !keys.contains(k)).collect();
    rest.sort();
    keys.extend(rest);

    keys.into_iter()
        .map(|key| {
            let info = &cache[key];
            let name = info["name"].as_str().filter(|s| !s.is_empty()).unwrap_or(key).to_string();
            let detail = info["user_name"].as_str().filter(|s| !s.is_empty()).map(str::to_string);
            let avatar = PICTURES.iter().find_map(|f| image_data_url(&dir.join(key).join(f)));
            let color = ["profile_highlight_color", "default_avatar_fill_color"]
                .iter()
                .filter_map(|k| info[*k].as_i64())
                // Chrome stores ARGB as a signed 32-bit int; black means "not themed".
                .map(|argb| argb as u32 & 0x00ff_ffff)
                .find(|rgb| *rgb != 0)
                .map(|rgb| format!("#{rgb:06x}"));
            RawProfile {
                key: key.to_string(),
                name,
                detail,
                avatar,
                color,
                args: vec![format!("--profile-directory={key}")],
            }
        })
        .collect()
}
