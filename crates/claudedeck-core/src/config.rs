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
    /// Aussehen des Terminals.
    pub terminal: TerminalSettings,
    /// Alle hinterlegten Verbindungsziele. Nach [`migrate_profiles`] nie leer.
    pub profiles: Vec<NamedProfile>,
    /// ID des gewählten Profils; zeigt sie ins Leere, greift [`Config::active`] auf das erste zu.
    pub active_profile: Option<String>,
    /// Beim Start automatisch verbinden, sofern ein Passwort hinterlegt ist.
    pub auto_connect: bool,
}

/// Ein benanntes Verbindungsziel.
///
/// `id` ist **nicht** kosmetisch: sie ist der Schlüssel, unter dem Passwort und Key-Passphrase
/// im Keyring liegen ([`crate::secrets::SecretStore`] nimmt ihn seit jeher als ersten
/// Parameter entgegen). Sie darf sich deshalb nie ändern — der sichtbare `name` schon.
#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
#[serde(default)]
pub struct NamedProfile {
    pub id: String,
    pub name: String,
    pub host: String,
    pub port: u16,
    pub user: String,
    pub auth: AuthMethod,
    pub key_path: Option<String>,
}

impl Default for NamedProfile {
    fn default() -> Self {
        let p = Profile::default();
        NamedProfile {
            id: LEGACY_PROFILE_ID.to_string(),
            name: format!("{}@{}", p.user, p.host),
            host: p.host,
            port: p.port,
            user: p.user,
            auth: p.auth,
            key_path: p.key_path,
        }
    }
}

/// Die ID, unter der vor der Profil-Unterstützung sämtliche Secrets abgelegt wurden. Das
/// migrierte Altprofil MUSS sie behalten, sonst findet die App ein bereits gespeichertes
/// Passwort nicht mehr wieder.
pub const LEGACY_PROFILE_ID: &str = "default";

impl Config {
    /// Das gewählte Profil — oder das erste, falls `active_profile` ins Leere zeigt (gelöschtes
    /// oder vertipptes Profil darf die App nicht ohne Verbindungsziel zurücklassen).
    ///
    /// Setzt voraus, dass `profiles` nicht leer ist; dafür sorgt [`migrate_profiles`], das
    /// [`load_from`] immer anwendet.
    pub fn active(&self) -> &NamedProfile {
        self.active_profile
            .as_deref()
            .and_then(|id| self.profiles.iter().find(|p| p.id == id))
            .or_else(|| self.profiles.first())
            .expect("profiles ist nach migrate_profiles nie leer")
    }
}

/// Sorgt dafür, dass mindestens ein Profil existiert.
///
/// Ist `profiles` leer (config.json von vor M8 oder frisch angelegt), entsteht eines aus dem
/// alten `profile`-Feld — mit [`LEGACY_PROFILE_ID`] als ID, damit vorhandene Keyring-Einträge
/// weiter passen. Vorhandene Profile bleiben unangetastet.
pub fn migrate_profiles(mut config: Config) -> Config {
    if config.profiles.is_empty() {
        let p = &config.profile;
        config.profiles = vec![NamedProfile {
            id: LEGACY_PROFILE_ID.to_string(),
            name: format!("{}@{}", p.user, p.host),
            host: p.host.clone(),
            port: p.port,
            user: p.user.clone(),
            auth: p.auth.clone(),
            key_path: p.key_path.clone(),
        }];
    }

    if config.active_profile.is_none() {
        config.active_profile = Some(config.profiles[0].id.clone());
    }

    config
}

/// Model und Arbeitsstärke für neu gestartete Sessions. `None` heißt „Flag weglassen" — dann
/// gelten Claude Codes eigene Vorgaben.
#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Default)]
#[serde(default)]
pub struct SessionDefaults {
    pub model: Option<String>,
    pub effort: Option<String>,
}

/// Aussehen des Terminals.
///
/// Einzige Struktur in dieser Datei mit `rename_all = "camelCase"`: die Felder gehen 1:1 an
/// `TerminalDisplay` in `src/lib/terminalTheme.ts` und werden dort direkt an xterm gereicht.
/// Gleiche Schreibweise auf beiden Seiten spart eine Übersetzungsschicht, die nur eine weitere
/// Stelle zum Auseinanderlaufen wäre.
///
/// Die Werte werden hier **nicht** validiert — das erledigt das Frontend beim Anwenden
/// (`themeById` fängt eine unbekannte Schema-ID ab, `clampFontSize` eine unsinnige Größe).
/// So bleibt eine von Hand editierte `config.json` immer ladbar.
#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
#[serde(default, rename_all = "camelCase")]
pub struct TerminalSettings {
    pub theme_id: String,
    pub font_family: String,
    pub font_size: u16,
    pub line_height: f32,
    pub letter_spacing: f32,
    pub cursor_style: String,
    pub cursor_blink: bool,
    pub scrollback: u32,
}

impl Default for TerminalSettings {
    fn default() -> Self {
        // Spiegelt DEFAULT_DISPLAY in src/lib/terminalTheme.ts.
        TerminalSettings {
            theme_id: "claudedeck-dark".to_string(),
            font_family: "\"JetBrains Mono\", Consolas, monospace".to_string(),
            font_size: 14,
            line_height: 1.2,
            letter_spacing: 0.0,
            cursor_style: "bar".to_string(),
            cursor_blink: true,
            scrollback: 10000,
        }
    }
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
            terminal: TerminalSettings::default(),
            // Bleibt leer — `migrate_profiles` füllt es aus `profile`, damit es genau einen Weg
            // gibt, wie ein Profil entsteht.
            profiles: vec![],
            active_profile: None,
            auto_connect: true,
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

/// Lädt die Config und migriert sie in einem Zug — so kann kein Aufrufer eine Config ohne
/// Profil in die Hand bekommen.
pub fn load_from(path: &Path) -> Config {
    migrate_profiles(
        fs::read_to_string(path)
            .ok()
            .and_then(|s| serde_json::from_str(&s).ok())
            .unwrap_or_default(),
    )
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
        assert_eq!(
            cfg.available_models,
            vec!["opus", "sonnet", "haiku", "fable"]
        );
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
        assert_eq!(
            cfg.available_models,
            vec!["opus", "sonnet", "haiku", "fable"]
        );
        // Auch die Terminal-Einstellungen müssen ohne Eintrag sinnvoll dastehen.
        assert_eq!(cfg.terminal, TerminalSettings::default());
        assert_eq!(cfg.terminal.theme_id, "claudedeck-dark");
    }

    /// Die Terminal-Felder gehen 1:1 ans Frontend — die Schreibweise muss camelCase sein, sonst
    /// findet `TerminalDisplay` sie nicht und alles fällt still auf Vorgaben zurück.
    #[test]
    fn terminal_settings_serialisieren_camel_case() {
        let json = serde_json::to_string(&TerminalSettings::default()).unwrap();
        assert!(json.contains("\"themeId\""), "{json}");
        assert!(json.contains("\"fontFamily\""), "{json}");
        assert!(json.contains("\"fontSize\""), "{json}");
        assert!(json.contains("\"cursorBlink\""), "{json}");
        assert!(!json.contains("theme_id"), "{json}");
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
            terminal: TerminalSettings {
                theme_id: "nord".to_string(),
                font_family: "Consolas, monospace".to_string(),
                font_size: 16,
                line_height: 1.4,
                letter_spacing: 0.5,
                cursor_style: "block".to_string(),
                cursor_blink: false,
                scrollback: 5000,
            },
            profiles: vec![NamedProfile::default()],
            active_profile: Some("default".to_string()),
            auto_connect: false,
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

    // --- Verbindungsprofile -----------------------------------------------------------------

    /// Eine config.json aus der Zeit vor den Profilen muss weiterlaufen — und zwar mit der ID
    /// `default`, weil genau darunter das Passwort bereits im Keyring liegt. Eine andere ID
    /// würde ein gespeichertes Passwort unauffindbar machen.
    #[test]
    fn migration_macht_aus_dem_altprofil_das_profil_default() {
        let cfg = migrate_profiles(Config {
            profile: Profile {
                host: "isekai.local".to_string(),
                port: 2222,
                user: "root".to_string(),
                auth: AuthMethod::Password,
                key_path: None,
            },
            ..Config::default()
        });

        assert_eq!(cfg.profiles.len(), 1);
        assert_eq!(cfg.profiles[0].id, "default");
        assert_eq!(cfg.profiles[0].host, "isekai.local");
        assert_eq!(cfg.profiles[0].port, 2222);
        assert_eq!(cfg.profiles[0].user, "root");
        assert_eq!(cfg.active_profile.as_deref(), Some("default"));
    }

    /// Der sichtbare Name soll ohne Zutun brauchbar sein, nicht „default" heißen.
    #[test]
    fn migration_benennt_das_altprofil_nach_benutzer_und_host() {
        let cfg = migrate_profiles(Config::default());
        assert_eq!(cfg.profiles[0].name, "root@isekai.local");
    }

    #[test]
    fn migration_laesst_vorhandene_profile_unangetastet() {
        let existing = NamedProfile {
            id: "vps".to_string(),
            name: "VPS".to_string(),
            host: "vps.example".to_string(),
            port: 22,
            user: "deploy".to_string(),
            auth: AuthMethod::Key,
            key_path: Some("/root/.ssh/id_ed25519".to_string()),
        };
        let cfg = migrate_profiles(Config {
            profiles: vec![existing.clone()],
            active_profile: Some("vps".to_string()),
            ..Config::default()
        });

        assert_eq!(cfg.profiles, vec![existing]);
        assert_eq!(cfg.active_profile.as_deref(), Some("vps"));
    }

    /// Zeigt `active_profile` ins Leere (Profil gelöscht, ID vertippt), darf die App nicht ohne
    /// Verbindungsziel dastehen.
    #[test]
    fn active_faellt_bei_unbekannter_id_auf_das_erste_profil_zurueck() {
        let cfg = migrate_profiles(Config {
            active_profile: Some("gibts-nicht".to_string()),
            ..Config::default()
        });
        assert_eq!(cfg.active().id, "default");
    }

    #[test]
    fn active_findet_das_gewaehlte_profil() {
        let mut cfg = migrate_profiles(Config::default());
        cfg.profiles.push(NamedProfile {
            id: "zweit".to_string(),
            name: "Zweiter".to_string(),
            host: "b.example".to_string(),
            port: 22,
            user: "u".to_string(),
            auth: AuthMethod::Password,
            key_path: None,
        });
        cfg.active_profile = Some("zweit".to_string());

        assert_eq!(cfg.active().host, "b.example");
    }

    /// `load_from` migriert selbst — sonst müsste jeder Aufrufer daran denken.
    #[test]
    fn load_from_liefert_immer_mindestens_ein_profil() {
        let tmpdir = TempDir::new().unwrap();
        let config_file = tmpdir.path().join("config.json");
        fs::write(&config_file, r#"{"profile":{"host":"alt.example"}}"#).unwrap();

        let cfg = load_from(&config_file);

        assert_eq!(cfg.profiles.len(), 1);
        assert_eq!(cfg.profiles[0].host, "alt.example");
        assert_eq!(cfg.active().host, "alt.example");
    }

    #[test]
    fn load_from_nonexistent_liefert_ebenfalls_ein_profil() {
        let cfg = load_from(&PathBuf::from("/nicht/vorhanden/config.json"));
        assert_eq!(cfg.profiles.len(), 1);
        assert_eq!(cfg.active().id, "default");
    }

    #[test]
    fn auto_connect_ist_per_default_an() {
        assert!(Config::default().auto_connect);
    }

    #[test]
    fn corrupted_json_returns_default() {
        let tmpdir = TempDir::new().unwrap();
        let config_file = tmpdir.path().join("config.json");

        // Write invalid JSON
        let invalid_json = r#"{"profile":{"host":"other", invalid json syntax"#;
        fs::write(&config_file, invalid_json).unwrap();

        let cfg = load_from(&config_file);

        // Vollständige Defaults statt Panik — inklusive Migration, weil `load_from` sie immer
        // anwendet und niemand eine Config ohne Profil in die Hand bekommen soll.
        assert_eq!(cfg, migrate_profiles(Config::default()));
    }
}
