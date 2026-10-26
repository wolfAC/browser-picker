use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

/// "Always open this site with ..." — matches the host and its subdomains.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Rule {
    pub host: String,
    pub target: String,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct Config {
    /// Browser or profile ids left out of the picker.
    pub hidden: Vec<String>,
    pub rules: Vec<Rule>,
}

impl Config {
    pub fn load(path: &Path) -> Self {
        std::fs::read_to_string(path)
            .ok()
            .and_then(|text| serde_json::from_str(&text).ok())
            .unwrap_or_default()
    }

    pub fn save(&self, path: &PathBuf) -> Result<(), String> {
        if let Some(dir) = path.parent() {
            std::fs::create_dir_all(dir).map_err(|e| e.to_string())?;
        }
        let text = serde_json::to_string_pretty(self).map_err(|e| e.to_string())?;
        std::fs::write(path, text).map_err(|e| e.to_string())
    }

    pub fn rule_for(&self, host: &str) -> Option<&str> {
        self.rules
            .iter()
            .filter(|r| host == r.host || host.ends_with(&format!(".{}", r.host)))
            // The most specific rule wins: mail.google.com over google.com.
            .max_by_key(|r| r.host.len())
            .map(|r| r.target.as_str())
    }

    pub fn set_rule(&mut self, host: String, target: String) {
        self.rules.retain(|r| r.host != host);
        self.rules.push(Rule { host, target });
        self.rules.sort_by(|a, b| a.host.cmp(&b.host));
    }
}

/// `https://www.github.com/x` -> `github.com`
pub fn host_of(url: &str) -> Option<String> {
    let host = url::Url::parse(url).ok()?.host_str()?.to_lowercase();
    Some(host.strip_prefix("www.").map(str::to_string).unwrap_or(host))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn most_specific_rule_wins() {
        let mut config = Config::default();
        config.set_rule("google.com".into(), "chrome".into());
        config.set_rule("mail.google.com".into(), "chrome/Profile 1".into());
        assert_eq!(config.rule_for("google.com"), Some("chrome"));
        assert_eq!(config.rule_for("docs.google.com"), Some("chrome"));
        assert_eq!(config.rule_for("mail.google.com"), Some("chrome/Profile 1"));
        assert_eq!(config.rule_for("notgoogle.com"), None);
    }

    #[test]
    fn host_drops_www() {
        assert_eq!(host_of("https://www.GitHub.com/a?b").as_deref(), Some("github.com"));
        assert_eq!(host_of("file:///tmp/a.html"), None);
    }
}
