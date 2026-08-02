use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use std::path::PathBuf;

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct Config {
    pub sort_column: String,
    pub sort_direction: String,
    pub show_details_panel: bool,
    pub auto_sort_by_size: bool,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            sort_column: "path".to_string(),
            sort_direction: "asc".to_string(),
            show_details_panel: true,
            auto_sort_by_size: true,
        }
    }
}

impl Config {
    pub fn load() -> Self {
        let Some(path) = config_path() else {
            return Self::default();
        };
        std::fs::read_to_string(&path)
            .ok()
            .and_then(|contents| toml::from_str(&contents).ok())
            .unwrap_or_default()
    }

    pub fn save(&self) -> Result<()> {
        let Some(path) = config_path() else {
            return Ok(());
        };
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent).with_context(|| {
                format!("Failed to create config directory {}", parent.display())
            })?;
        }
        std::fs::write(&path, toml::to_string_pretty(self)?)
            .with_context(|| format!("Failed to write config {}", path.display()))?;
        Ok(())
    }
}

pub fn config_path() -> Option<PathBuf> {
    std::env::var_os("HOME").map(|home| PathBuf::from(home).join(".config/dir-kill/config.toml"))
}

/// Shorten an absolute path using `~` for the user's home directory.
pub fn shorten_home_path(path: &str) -> String {
    let Ok(home) = std::env::var("HOME") else {
        return path.to_string();
    };
    if path == home {
        return "~".to_string();
    }
    let prefix = format!("{home}/");
    if path.starts_with(&prefix) {
        format!("~{}", &path[home.len()..])
    } else {
        path.to_string()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_shorten_home_path() {
        if let Ok(home) = std::env::var("HOME") {
            assert_eq!(
                shorten_home_path(&format!("{home}/Developer")),
                "~/Developer"
            );
            assert_eq!(shorten_home_path(&home), "~");
        }
        assert_eq!(shorten_home_path("/other/path"), "/other/path");
    }

    #[test]
    fn test_config_default_roundtrip() {
        let config = Config::default();
        let parsed: Config = toml::from_str(&toml::to_string(&config).unwrap()).unwrap();
        assert!(parsed.show_details_panel);
        assert!(parsed.auto_sort_by_size);
    }
}
