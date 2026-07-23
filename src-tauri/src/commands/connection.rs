//! Verbindungs-Commands: `connect`/`accept_hostkey_and_connect`/`disconnect`,
//! `get_config`/`set_config`, `save_secret`/`has_secret`.
//!
//! known_hosts ist bewusst KEINE `~/.ssh/known_hosts`, sondern eine app-eigene Datei unter
//! `dirs::config_dir()/claudedeck/known_hosts` (Entscheidung aus dem M2-Final-Review): eine
//! System-known_hosts kann Einträge enthalten, die von woanders (z.B. einem interaktiven
//! `ssh`-Lauf mit `StrictHostKeyChecking=accept-new`) unbeaufsichtigt hineingelangt sind — die
//! App würde ihnen dann blind vertrauen (fail-open). Die eigene Datei kennt nur Keys, die diese
//! App selbst per explizitem Nutzer-OK (`accept_hostkey_and_connect`) akzeptiert hat.

use claudedeck_core::config::{self, AuthMethod, Config, Profile};
use claudedeck_core::secrets::{KeyringStore, SecretKind, SecretStore};
use claudedeck_core::ssh::{Auth, ConnectParams, HostkeyPolicy, SshConnection};
use serde::{Deserialize, Serialize};
use std::path::PathBuf;
use std::sync::Arc;
use tauri::{AppHandle, Emitter, State};

use crate::error::ApiError;
use crate::state::AppState;

/// Bis das Config-Schema mehrere benannte Profile unterstützt (aktuell genau ein
/// `Config::profile`, siehe `claudedeck_core::config::Config`), ist die Keyring-Ablage an
/// diesen festen Bezeichner gebunden statt an einen Profilnamen.
const PROFILE_ID: &str = "default";

/// `kind`-Argument von `save_secret`/`has_secret` — spiegelt `SecretKind` aus core, aber als
/// eigener, `Deserialize`-fähiger Typ mit den camelCase-Literalen aus dem IPC-Contract
/// (`"password"|"keyPassphrase"`).
#[derive(Deserialize, Clone, Copy)]
#[serde(rename_all = "camelCase")]
pub enum SecretArgKind {
    Password,
    KeyPassphrase,
}

impl From<SecretArgKind> for SecretKind {
    fn from(kind: SecretArgKind) -> Self {
        match kind {
            SecretArgKind::Password => SecretKind::Password,
            SecretArgKind::KeyPassphrase => SecretKind::KeyPassphrase,
        }
    }
}

#[derive(Serialize, Clone)]
#[serde(rename_all = "camelCase")]
struct ConnectionStateEvent {
    state: &'static str,
}

fn emit_connection_state(app: &AppHandle, state: &'static str) {
    // Ein fehlgeschlagener Emit (z.B. kein Fenster mehr offen) darf den Connect-Flow nicht
    // abbrechen — das Ergebnis des Commands selbst (Ok/Err) ist die verbindliche Antwort.
    let _ = app.emit("connection-state", ConnectionStateEvent { state });
}

fn known_hosts_path() -> PathBuf {
    dirs::config_dir()
        .map(|d| d.join("claudedeck").join("known_hosts"))
        .unwrap_or_else(|| PathBuf::from("~/.config/claudedeck/known_hosts"))
}

/// Baut `Auth` aus dem Profil: Password-Auth nimmt den Parameter-Override, sonst das Keyring;
/// Key-Auth braucht `key_path` aus der Config, die Passphrase (falls gesetzt) kommt aus dem
/// Keyring. Liefert `ApiError::AuthFailed`, wenn die nötigen Angaben fehlen (kein Connect-
/// Versuch ohne verwertbare Auth).
fn build_auth(profile: &Profile, password_override: Option<String>) -> Result<Auth, ApiError> {
    match profile.auth {
        AuthMethod::Password => {
            let password = password_override
                .or_else(|| KeyringStore.get(PROFILE_ID, SecretKind::Password))
                .ok_or_else(|| ApiError::AuthFailed {
                    message: "Kein Passwort hinterlegt".to_string(),
                })?;
            Ok(Auth::Password(password))
        }
        AuthMethod::Key => {
            let key_path = profile
                .key_path
                .clone()
                .ok_or_else(|| ApiError::AuthFailed {
                    message: "Kein Key-Pfad konfiguriert".to_string(),
                })?;
            let passphrase = KeyringStore.get(PROFILE_ID, SecretKind::KeyPassphrase);
            Ok(Auth::Key {
                path: PathBuf::from(key_path),
                passphrase,
            })
        }
    }
}

async fn do_connect(
    app: &AppHandle,
    state: &AppState,
    password: Option<String>,
    policy: HostkeyPolicy,
) -> Result<(), ApiError> {
    emit_connection_state(app, "connecting");

    let config = config::load_from(&config::config_path());
    let auth = match build_auth(&config.profile, password) {
        Ok(auth) => auth,
        Err(err) => {
            emit_connection_state(app, "failed");
            return Err(err);
        }
    };

    let params = ConnectParams {
        host: config.profile.host.clone(),
        port: config.profile.port,
        user: config.profile.user.clone(),
        auth,
        known_hosts: known_hosts_path(),
        policy,
    };

    match SshConnection::connect(params).await {
        Ok(conn) => {
            // `Arc`, nicht die nackte `SshConnection` — Fix Critical (siehe `state.rs`):
            // Commands klonen nur dieses `Arc` unterm State-Lock und awaiten SSH-Operationen
            // danach außerhalb des Locks.
            state.lock().await.conn = Some(Arc::new(conn));
            emit_connection_state(app, "connected");
            Ok(())
        }
        Err(err) => {
            emit_connection_state(app, "failed");
            Err(ApiError::from(err))
        }
    }
}

/// Verbindet mit dem konfigurierten Profil unter `HostkeyPolicy::Strict`. Bei einem bislang
/// unbekannten Host-Key scheitert das mit `ApiError::HostkeyUnknown{fingerprint}` — das
/// Frontend zeigt dann den Bestätigungsdialog und ruft bei Zustimmung
/// `accept_hostkey_and_connect` auf.
#[tauri::command]
pub async fn connect(
    app: AppHandle,
    state: State<'_, AppState>,
    password: Option<String>,
) -> Result<(), ApiError> {
    do_connect(&app, &state, password, HostkeyPolicy::Strict).await
}

/// Wie `connect`, aber mit `HostkeyPolicy::AcceptNew`: genau EIN Connect-Versuch, der den bis
/// dahin unbekannten Host-Key in die app-eigene known_hosts appendet und akzeptiert. Ab diesem
/// Zeitpunkt ist der Key "Known" — nachfolgende `connect`-Aufrufe laufen wieder unter `Strict`.
#[tauri::command]
pub async fn accept_hostkey_and_connect(
    app: AppHandle,
    state: State<'_, AppState>,
    password: Option<String>,
) -> Result<(), ApiError> {
    do_connect(&app, &state, password, HostkeyPolicy::AcceptNew).await
}

// Tauri-Regel: async Commands mit Referenz-Argumenten (`State<'_, _>`) müssen `Result`
// zurückgeben. `disconnect` kann laut IPC-Contract nicht fehlschlagen — `Result<(), ()>`
// bleibt für das Frontend äquivalent zu `-> ()` (löst immer auf, nie ein `Err`).
#[tauri::command]
pub async fn disconnect(app: AppHandle, state: State<'_, AppState>) -> Result<(), ()> {
    let mut inner = state.lock().await;
    inner.conn = None;
    inner.sessions.clear();
    drop(inner);
    emit_connection_state(&app, "disconnected");
    Ok(())
}

#[tauri::command]
pub fn get_config() -> Config {
    config::load_from(&config::config_path())
}

#[tauri::command]
pub fn set_config(config: Config) -> Result<(), ApiError> {
    Ok(config::save_to(&config::config_path(), &config)?)
}

/// Legt `value` (Passwort oder Key-Passphrase) im OS-Keyring ab. `value` wird nie geloggt —
/// `SecretArgKind` trägt bewusst kein `Debug`, das `value` mitausgeben könnte.
#[tauri::command]
pub fn save_secret(kind: SecretArgKind, value: String) -> Result<(), ApiError> {
    KeyringStore
        .set(PROFILE_ID, kind.into(), &value)
        .map_err(|message| ApiError::Io { message })
}

#[tauri::command]
pub fn has_secret(kind: SecretArgKind) -> bool {
    KeyringStore.get(PROFILE_ID, kind.into()).is_some()
}
