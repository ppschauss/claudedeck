//! Verbindungs-Commands: `connect`/`accept_hostkey_and_connect`/`disconnect`,
//! `get_config`/`set_config`, `save_secret`/`has_secret`.
//!
//! known_hosts ist bewusst KEINE `~/.ssh/known_hosts`, sondern eine app-eigene Datei unter
//! `dirs::config_dir()/claudedeck/known_hosts` (Entscheidung aus dem M2-Final-Review): eine
//! System-known_hosts kann Einträge enthalten, die von woanders (z.B. einem interaktiven
//! `ssh`-Lauf mit `StrictHostKeyChecking=accept-new`) unbeaufsichtigt hineingelangt sind — die
//! App würde ihnen dann blind vertrauen (fail-open). Die eigene Datei kennt nur Keys, die diese
//! App selbst per explizitem Nutzer-OK (`accept_hostkey_and_connect`) akzeptiert hat.
//!
//! ## Task 6, Auflage A — Reentrancy-Guard
//!
//! `do_connect_core` (der gemeinsame Kern hinter `connect`/`accept_hostkey_and_connect` UND
//! den automatischen Versuchen des Reconnect-Supervisors, siehe `reconnect_supervisor.rs`)
//! prüft als ALLERERSTES, ob `AppState.conn` bereits `Some` ist. Wenn ja: die bestehende
//! Verbindung UND ihre Sessions werden vollständig aufgeräumt (`state::cleanup_sessions_fully`,
//! Auflage B — jeder `PtyHandle` explizit `close()`t, `tokio::spawn`, nicht blockierend),
//! BEVOR der neue Verbindungsversuch beginnt. Bewusste Wahl gegenüber einem Fehler: ein
//! zweiter `connect()`-Aufruf, während die App schon verbunden ist, ist kein Sonderfall, den
//! wir ablehnen müssen (z.B. ein künftiger Profilwechsel) — er verhält sich einfach wie ein
//! impliziter `disconnect()` gefolgt von `connect()`. Alle `do_connect_core`-Aufrufe laufen
//! zusätzlich seriell hinter `ReconnectSupervisor::connect_lock` (verhindert zwei echte
//! `SshConnection::connect`-Läufe gleichzeitig gegen denselben Host, z.B. wenn der manuelle
//! "Jetzt neu verbinden"-Button und der Supervisor selbst binnen Millisekunden beide versuchen).
//!
//! Während einer laufenden Reconnect-Recovery (`AppState.conn == None`, Sessions mit
//! `lost == true`) greift dieser Reentrancy-Zweig NICHT — dort ist `conn` ja bereits `None`,
//! genau damit ein nachfolgender `do_connect_core`-Aufruf (egal ob manuell oder vom
//! Supervisor) direkt zum normalen Verbindungsaufbau + Re-Attach (`reattach_lost_sessions`)
//! durchläuft, statt die gerade erst als `lost` markierten Sessions ein zweites Mal
//! wegzuräumen.

use claudedeck_core::config::{self, AuthMethod, Config, Profile};
use claudedeck_core::secrets::{KeyringStore, SecretKind, SecretStore};
use claudedeck_core::ssh::{Auth, ConnectParams, HostkeyPolicy, SshConnection};
use serde::{Deserialize, Serialize};
use std::path::PathBuf;
use std::sync::Arc;
use tauri::{AppHandle, Emitter, Manager, State};

use crate::commands::sessions::reattach_lost_sessions;
use crate::error::ApiError;
use crate::reconnect_supervisor::ReconnectSupervisor;
use crate::state::{cleanup_sessions_fully, AppState};

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

/// Gemeinsamer Kern hinter `connect`/`accept_hostkey_and_connect` UND den automatischen
/// Reconnect-Versuchen des Supervisors (`reconnect_supervisor::run_recovery`) — siehe
/// Moduldoku, Abschnitt "Auflage A", für die Reentrancy-Guard-Begründung. `pub(crate)`, damit
/// `reconnect_supervisor.rs` denselben Pfad nutzt statt Connect-Logik zu duplizieren.
///
/// `opportunistic`: `false` für die beiden Nutzer-Commands (`connect`/
/// `accept_hostkey_and_connect`) — dort GILT die Reentrancy-Wipe-Semantik uneingeschränkt
/// (Auflage A). `true` NUR für die automatischen Versuche des Supervisors: die warten vorher
/// interruptible auf `connect_lock` (über den manuellen Retry-Pfad kann in der Zwischenzeit
/// ein `connect()`-Aufruf bereits erfolgreich durchgelaufen sein, während der Supervisor noch
/// auf den Lock wartete). Ein `opportunistic`-Aufruf, der `connect_lock` bekommt und dann
/// `conn.is_some()` vorfindet, wertet das als "jemand anderes hat es bereits geschafft" und
/// kehrt sofort mit `Ok(())` zurück, STATT die frisch aufgebaute (ggf. bereits re-attachte)
/// Verbindung wieder wegzuwerfen und ein zweites Mal neu zu verbinden — ohne diese
/// Unterscheidung würde jeder erfolgreiche manuelle Reconnect-Klick, der knapp vor einem
/// zeitgleichen Supervisor-Versuch gewinnt, sofort von genau diesem Supervisor-Versuch wieder
/// zunichtegemacht (Connect → sofort erneuter Wipe+Connect-Flicker).
///
/// `opportunistic` steuert außerdem (Fix Important, Review-Fund Task 6), ob dieser Aufruf die
/// `connection-state`-Events `"connecting"`/`"failed"` emittiert: NUR bei `false` (interaktiver
/// Erst-Connect). Bei `true` treibt `run_recovery`s Backoff-Schleife die State-Machine selbst
/// (`"reconnecting"` pro Versuch, `"failed"` nur einmal endgültig nach `AuthFailed`) — würde
/// dieser Kern zusätzlich bei JEDEM einzelnen Supervisor-Versuch `"connecting"`/`"failed"`
/// emittieren, würde das `failed`-Modal im Frontend bei jedem Retry kurz aufblitzen, statt dass
/// der Nutzer einen durchgehenden `reconnecting`-Countdown sieht. `"connected"` bei Erfolg wird
/// dagegen IMMER emittiert (auch `opportunistic`) — das ist das Signal, mit dem der Supervisor
/// seine Schleife als beendet erkennt.
pub(crate) async fn do_connect_core(
    app: &AppHandle,
    state: &AppState,
    password: Option<String>,
    policy: HostkeyPolicy,
    opportunistic: bool,
) -> Result<(), ApiError> {
    // Serialisiert ALLE Connect-Versuche (manuell, Hostkey-Accept, Supervisor) — verhindert
    // zwei echte `SshConnection::connect`-Läufe gleichzeitig gegen denselben Host.
    let sup = app.state::<ReconnectSupervisor>();
    let _connect_guard = sup.connect_lock.lock().await;

    // Auflage A: Reentrancy-Guard. Bereits verbunden? Alte Verbindung + ihre Sessions
    // vollständig aufräumen (Auflage B), bevor der neue Versuch beginnt. Greift NICHT während
    // einer laufenden Reconnect-Recovery — dort ist `conn` bereits `None` (siehe Moduldoku).
    {
        let mut inner = state.lock().await;
        if inner.conn.is_some() {
            if opportunistic {
                // Siehe Doku oben: ein anderer Aufruf (i.d.R. der manuelle Retry-Button) hat
                // bereits erfolgreich verbunden, während dieser Supervisor-Versuch auf den
                // Lock wartete — nichts wegwerfen, einfach als Erfolg werten.
                return Ok(());
            }
            inner.conn = None;
            let old_sessions = std::mem::take(&mut inner.sessions);
            drop(inner);
            cleanup_sessions_fully(old_sessions);
        }
    }

    // Fix Important (Review-Fund Task 6, UX): "connecting"/"failed" nur beim interaktiven
    // Erst-Connect emittieren. Ein `opportunistic`-Aufruf kommt aus `run_recovery`s
    // Backoff-Schleife, die die State-Machine selbst treibt (`reconnecting`/`failed`/
    // `connected`, siehe `reconnect_supervisor.rs`) — ein zusätzliches `connecting`/`failed`
    // aus jedem einzelnen Supervisor-Versuch würde das `failed`-Modal bei jedem Retry kurz
    // aufblitzen lassen, statt dass der Nutzer den durchgehenden `reconnecting`-Countdown sieht.
    if !opportunistic {
        emit_connection_state(app, "connecting");
    }

    let config = config::load_from(&config::config_path());
    let auth = match build_auth(&config.profile, password) {
        Ok(auth) => auth,
        Err(err) => {
            if !opportunistic {
                emit_connection_state(app, "failed");
            }
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
            // Task 6: Re-Attach jeder `lost`-Session auf den neuen `Arc<SshConnection>` — bei
            // einem normalen Erstconnect ist die Sessions-Map leer bzw. keine Session `lost`,
            // dann ist das ein No-Op.
            reattach_lost_sessions(app, state).await;
            // "connected" bleibt UNBEDINGT (auch bei `opportunistic`) — der Supervisor selbst
            // emittiert keinen eigenen Erfolgs-State, sondern verlässt sich genau auf dieses
            // Event, um die Backoff-Schleife als beendet zu markieren (siehe `run_recovery`).
            emit_connection_state(app, "connected");
            Ok(())
        }
        Err(err) => {
            if !opportunistic {
                emit_connection_state(app, "failed");
            }
            Err(ApiError::from(err))
        }
    }
}

/// Verbindet mit dem konfigurierten Profil unter `HostkeyPolicy::Strict`. Bei einem bislang
/// unbekannten Host-Key scheitert das mit `ApiError::HostkeyUnknown{fingerprint}` — das
/// Frontend zeigt dann den Bestätigungsdialog und ruft bei Zustimmung
/// `accept_hostkey_and_connect` auf.
///
/// Ruft bei JEDEM Aufruf `wake_retry()` — falls der Reconnect-Supervisor gerade in seinem
/// Backoff-`sleep` wartet (z.B. weil dies der manuelle "Jetzt neu verbinden"-Button im
/// ReconnectOverlay war), wacht er sofort auf statt den Rest der Wartezeit verstreichen zu
/// lassen. Ist der Supervisor gerade NICHT am Warten, ist der Aufruf ein folgenloses No-Op.
#[tauri::command]
pub async fn connect(
    app: AppHandle,
    state: State<'_, AppState>,
    password: Option<String>,
) -> Result<(), ApiError> {
    app.state::<ReconnectSupervisor>().wake_retry();
    do_connect_core(&app, &state, password, HostkeyPolicy::Strict, false).await
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
    app.state::<ReconnectSupervisor>().wake_retry();
    do_connect_core(&app, &state, password, HostkeyPolicy::AcceptNew, false).await
}

// Tauri-Regel: async Commands mit Referenz-Argumenten (`State<'_, _>`) müssen `Result`
// zurückgeben. `disconnect` kann laut IPC-Contract nicht fehlschlagen — `Result<(), ()>`
// bleibt für das Frontend äquivalent zu `-> ()` (löst immer auf, nie ein `Err`).
///
/// Auflage B: alle `PtyHandle`s der noch offenen Sessions werden explizit `close()`t
/// (`cleanup_sessions_fully` — je ein `tokio::spawn`, nie blockierend), statt sich auf die
/// Drop-Kaskade von `AppInner.sessions` zu verlassen. Bricht zusätzlich eine gerade laufende
/// Reconnect-Recovery-Runde ab (`ReconnectSupervisor::cancel`) — ein Nutzer, der bewusst
/// trennt, während der Supervisor noch im Backoff wartet, soll NICHT Sekunden später
/// automatisch wieder verbunden werden.
#[tauri::command]
pub async fn disconnect(app: AppHandle, state: State<'_, AppState>) -> Result<(), ()> {
    app.state::<ReconnectSupervisor>().cancel();
    let sessions = {
        let mut inner = state.lock().await;
        inner.conn = None;
        std::mem::take(&mut inner.sessions)
    };
    cleanup_sessions_fully(sessions);
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
