use serde::{Deserialize, Serialize};
use std::fs;
use std::path::{Path, PathBuf};

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
#[serde(default)]
pub struct Config {
    pub profile: Profile,
    pub scan_paths: Vec<String>,
    pub favorites: Vec<String>,
    pub notifications: NotifySettings,
    /// Vorgaben, mit denen `start_project` neue Sessions startet.
    pub defaults: SessionDefaults,
    /// Auswahl des Model-Reglers. Bewusst konfigurierbar statt im Code fest verdrahtet: ein neues
    /// Modell soll ohne Rebuild wählbar sein. Aliase halten die Liste über Releases hinweg gültig.
    pub available_models: Vec<String>,
}

/// Model und Arbeitsstärke für neu gestartete Sessions. `None` heißt „Flag weglassen" — dann
/// gelten Claude Codes eigene Vorgaben.
#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Default)]
#[serde(default)]
pub struct SessionDefaults {
    pub model: Option<String>,
    pub effort: Option<String>,
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
#[serde(default)]
pub struct Profile {
    pub host: String,
    pub port: u16,
    pub user: String,
    pub auth: AuthMethod,
    #[serde(default)]
    pub key_path: Option<String>,
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Default)]
pub enum AuthMethod {
    Key,
    #[default]
    Password,
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
#[serde(default)]
pub struct NotifySettings {
    pub enabled: bool,
    pub silence_ms: u64,
}

impl Default for Config {
    fn default() -> Self {
        Config {
            profile: Profile::default(),
            scan_paths: vec!["/mnt/cache/appdata".to_string()],
            favorites: vec![],
            notifications: NotifySettings::default(),
            defaults: SessionDefaults::default(),
            available_models: vec![
                "opus".to_string(),
                "sonnet".to_string(),
                "haiku".to_string(),
                "fable".to_string(),
            ],
        }
    }
}

impl Default for Profile {
    fn default() -> Self {
        Profile {
            host: "isekai.local".to_string(),
            port: 22,
            user: "root".to_string(),
            auth: AuthMethod::default(),
            key_path: None,
        }
    }
}

impl Default for NotifySettings {
    fn default() -> Self {
        NotifySettings {
            enabled: true,
            silence_ms: 2000,
        }
    }
}

pub fn config_path() -> PathBuf {
    dirs::config_dir()
        .map(|d| d.join("claudedeck/config.json"))
        .unwrap_or_else(|| PathBuf::from("~/.config/claudedeck/config.json"))
}

pub fn load_from(path: &Path) -> Config {
    fs::read_to_string(path)
        .ok()
        .and_then(|s| serde_json::from_str(&s).ok())
        .unwrap_or_default()
}

pub fn save_to(path: &Path, cfg: &Config) -> std::io::Result<()> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }
    let json = serde_json::to_string_pretty(cfg)?;
    fs::write(path, json)?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    #[test]
    fn load_from_nonexistent_path_returns_default() {
        let nonexistent = PathBuf::from("/nonexistent/path/that/does/not/exist/config.json");
        let cfg = load_from(&nonexistent);

        // Verify all default values
        assert_eq!(cfg.profile.host, "isekai.local");
        assert_eq!(cfg.profile.port, 22);
        assert_eq!(cfg.profile.user, "root");
        assert_eq!(cfg.profile.auth, AuthMethod::Password);
        assert_eq!(cfg.profile.key_path, None);
        assert_eq!(cfg.scan_paths, vec!["/mnt/cache/appdata".to_string()]);
        assert_eq!(cfg.favorites, vec![] as Vec<String>);
        assert!(cfg.notifications.enabled);
        assert_eq!(cfg.notifications.silence_ms, 2000);
        assert_eq!(cfg.defaults.model, None);
        assert_eq!(cfg.defaults.effort, None);
    }

    /// Aliase statt fester Model-IDs: so braucht ein neues Modell keine Code-Änderung.
    #[test]
    fn available_models_sind_per_default_die_aliase() {
        let cfg = Config::default();
        assert_eq!(cfg.available_models, vec!["opus", "sonnet", "haiku", "fable"]);
    }

    /// Eine vor M7 geschriebene config.json darf nicht auf Defaults zurückfallen.
    #[test]
    fn alte_config_ohne_defaults_bleibt_lesbar() {
        let tmpdir = TempDir::new().unwrap();
        let config_file = tmpdir.path().join("config.json");
        fs::write(
            &config_file,
            r#"{"profile":{"host":"isekai.local"},"scan_paths":["/mnt/x"]}"#,
        )
        .unwrap();

        let cfg = load_from(&config_file);

        assert_eq!(cfg.scan_paths, vec!["/mnt/x".to_string()]);
        assert_eq!(cfg.defaults.model, None);
        assert_eq!(cfg.available_models, vec!["opus", "sonnet", "haiku", "fable"]);
    }

    #[test]
    fn roundtrip_save_and_load_equals_original() {
        let tmpdir = TempDir::new().unwrap();
        let config_file = tmpdir.path().join("config.json");

        let original = Config {
            profile: Profile {
                host: "custom.host".to_string(),
                port: 2222,
                user: "alice".to_string(),
                auth: AuthMethod::Key,
                key_path: Some("/home/alice/.ssh/id_rsa".to_string()),
            },
            scan_paths: vec!["/mnt/one".to_string(), "/mnt/two".to_string()],
            favorites: vec!["fav1".to_string()],
            notifications: NotifySettings {
                enabled: false,
                silence_ms: 5000,
            },
            defaults: SessionDefaults {
                model: Some("opus".to_string()),
                effort: Some("xhigh".to_string()),
            },
            available_models: vec!["opus".to_string(), "fable".to_string()],
        };

        save_to(&config_file, &original).unwrap();
        let loaded = load_from(&config_file);

        assert_eq!(loaded, original);
    }

    #[test]
    fn partial_json_with_defaults() {
        let tmpdir = TempDir::new().unwrap();
        let config_file = tmpdir.path().join("config.json");

        // Write partial JSON with only profile.host set
        let partial_json = r#"{"profile":{"host":"other"}}"#;
        fs::write(&config_file, partial_json).unwrap();

        let cfg = load_from(&config_file);

        // profile.host should be overridden
        assert_eq!(cfg.profile.host, "other");

        // All other profile fields should be default
        assert_eq!(cfg.profile.port, 22);
        assert_eq!(cfg.profile.user, "root");
        assert_eq!(cfg.profile.auth, AuthMethod::Password);
        assert_eq!(cfg.profile.key_path, None);

        // All other top-level fields should be default
        assert_eq!(cfg.scan_paths, vec!["/mnt/cache/appdata".to_string()]);
        assert_eq!(cfg.favorites, vec![] as Vec<String>);
        assert!(cfg.notifications.enabled);
        assert_eq!(cfg.notifications.silence_ms, 2000);
    }

    #[test]
    fn corrupted_json_returns_default() {
        let tmpdir = TempDir::new().unwrap();
        let config_file = tmpdir.path().join("config.json");

        // Write invalid JSON
        let invalid_json = r#"{"profile":{"host":"other", invalid json syntax"#;
        fs::write(&config_file, invalid_json).unwrap();

        let cfg = load_from(&config_file);

        // Should return complete default, not panic
        assert_eq!(cfg, Config::default());
    }
}
