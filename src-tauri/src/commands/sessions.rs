//! Session-Streaming-Commands (Kern von M4): `list_sessions`, `open_session`,
//! `start_project`, `write_session`, `resize_session`, `close_session`, `kill_session`.
//!
//! Lock-Disziplin (Fix Critical, siehe `state.rs`): JEDER Command lockt `AppState` nur kurz,
//! um ein `Arc<SshConnection>` bzw. `Arc<Mutex<Option<PtyHandle>>>` zu klonen oder einen
//! Map-Eintrag zu holen/einzufügen — der Guard wird VOR jedem SSH-Await gedroppt (entweder
//! durch einen eigenen `{ }`-Block oder weil der Lock-Ausdruck als Statement endet). SSH-Calls
//! (`exec_capture`, `open_pty`, `pty.write`/`resize`) laufen ausschließlich außerhalb des
//! `AppState`-Locks. `write_session`/`resize_session` locken zusätzlich NUR den
//! Session-eigenen `tokio::Mutex` (`SessionEntry.pty`) — paralleles Schreiben in verschiedene
//! Sessions blockiert sich damit nicht mehr gegenseitig. `pty.write`/`pty.resize` laufen unter
//! einem 10s-`tokio::time::timeout`; ein Timeout wird als `ApiError::Io{"Timeout ..."}`
//! gemeldet, statt den aufrufenden Task unbegrenzt hängen zu lassen.
//!
//! `close_session` nimmt den Eintrag aus der Map, setzt `closing` (siehe Forwarder unten) und
//! `take()`t den `PtyHandle` aus dem Session-Mutex in einem separaten `tokio::spawn` — `close()`
//! kann laut `pty.rs`-Doku bis zu 2s dauern und darf den Command-Aufruf nie blockieren
//! (Global Constraint).
//!
//! Forwarder-Task (ein `tokio::spawn` pro offener Session, gestartet in `open_session`/
//! `start_project`): liest `PtyEvent`s aus `take_output()`, batcht Bytes über die reine
//! Entscheidungsfunktion `claudedeck_core::util::should_flush` (>=32 KiB ODER >=10ms seit
//! dem ersten ungeflushten Byte) und sendet den Batch base64-kodiert über den
//! `Channel<OutputChunk>` ans Frontend. Bei `PtyEvent::Exit` wird der Restpuffer geflusht; ob
//! `pty-exit` emittiert wird, hängt vom `closing`-Flag ab (Fix Important — siehe
//! `spawn_forwarder`): bei einem selbst ausgelösten Detach (`close_session` hat `closing`
//! bereits gesetzt) wird NICHT emittiert, weil das Frontend sonst fälschlich "Prozess beendet"
//! anzeigen würde, obwohl der Nutzer nur detached hat. Der Map-Eintrag wird in jedem Fall
//! (idempotent) entfernt.

use std::collections::HashSet;
use std::path::Path;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::Duration;

use data_encoding::BASE64;
use serde::Serialize;
use tauri::ipc::Channel;
use tauri::{AppHandle, Emitter, Manager, State};
use tokio::sync::mpsc;
use tokio::time::{sleep_until, Instant};

use claudedeck_core::config;
use claudedeck_core::ssh::{ExecOutput, PtyEvent, SshConnection};
use claudedeck_core::tmux::commands as tmux_cmd;
use claudedeck_core::tmux::names;
use claudedeck_core::tmux::parser::{merge, parse_panes, parse_sessions, SessionInfo, SessionKind};
use claudedeck_core::util::should_flush;

use crate::error::ApiError;
use crate::state::{AppInner, AppState, SessionEntry};

/// Timeout für `pty.write`/`pty.resize` (Fix Critical, Timeout-Härtung im Hot-Path) — ein
/// hängender SSH-Await auf einer einzelnen Session soll dem Aufrufer nach spätestens dieser
/// Zeit einen sichtbaren Fehler geben statt den Tauri-Command-Aufruf unbegrenzt offen zu lassen.
const SESSION_IO_TIMEOUT: Duration = Duration::from_secs(10);

/// Ein Output-Batch für `Channel<OutputChunk>` — `data_b64` wird über serdes
/// `rename_all = "camelCase"` zu `dataB64`, wie im IPC-Contract festgelegt.
#[derive(Serialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct OutputChunk {
    data_b64: String,
}

#[derive(Serialize, Clone)]
#[serde(rename_all = "camelCase")]
struct PtyExitEvent {
    session_id: String,
    reason: &'static str,
}

/// App-eigenes Abbild von `claudedeck_core::tmux::parser::SessionInfo` fürs Frontend
/// (`SessionKind` selbst ist nicht `Serialize` — core kennt keine IPC-Belange).
#[derive(Serialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct SessionInfoDto {
    pub name: String,
    pub kind: String,
    pub cwd: String,
    pub attached: bool,
    pub created: i64,
    pub managed: bool,
}

impl From<SessionInfo> for SessionInfoDto {
    fn from(s: SessionInfo) -> Self {
        SessionInfoDto {
            name: s.name,
            kind: match s.kind {
                SessionKind::Claude => "claude".to_string(),
                SessionKind::Shell => "shell".to_string(),
            },
            cwd: s.cwd,
            attached: s.attached,
            created: s.created,
            managed: s.managed,
        }
    }
}

/// Ein per `scan_paths` gefundenes, noch nicht angehängtes Projektverzeichnis.
#[derive(Serialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct Project {
    pub path: String,
    pub name: String,
}

#[derive(Serialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct SessionList {
    pub running: Vec<SessionInfoDto>,
    pub startable: Vec<Project>,
}

#[derive(Serialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct StartResult {
    pub session_id: String,
    pub session_name: String,
}

fn not_connected() -> ApiError {
    ApiError::NotConnected {
        message: "nicht verbunden".to_string(),
    }
}

/// Klont das `Arc<SshConnection>` aus dem State — der Aufrufer hält den `AppInner`-Guard nur für
/// diesen einen Klon und droppt ihn danach, bevor er mit dem `Arc` einen SSH-Await macht (Fix
/// Critical: der globale Lock wird nie über einen SSH-Await gehalten).
fn require_conn(inner: &AppInner) -> Result<Arc<SshConnection>, ApiError> {
    inner.conn.clone().ok_or_else(not_connected)
}

/// Fix Minor (Fix 3): für eine unbekannte/bereits geschlossene `session_id` bewusst
/// `ApiError::Io` statt `ApiError::NotConnected` — Letzteres ist im IPC-Contract für "keine
/// SSH-Verbindung" reserviert (siehe `require_conn`/`not_connected`); eine unbekannte Session
/// ist ein anderer Fehlerfall und würde das Frontend fälschlich zum Reconnect-Dialog schicken.
/// Eine eigene Contract-Variante extra dafür einzuführen wäre für diesen einen Fehlerfall
/// Overkill — `ApiError::Io` mit stabiler, sprechender Message ist Contract-kompatibel; das
/// Frontend kann bei Bedarf über `message` unterscheiden (kein `kind` nötig).
fn session_not_found(session_id: &str) -> ApiError {
    ApiError::Io {
        message: format!("Session {session_id} nicht gefunden"),
    }
}

/// Fix Critical (Timeout-Härtung): Message für einen abgelaufenen `pty.write`/`pty.resize`.
fn io_timeout(op: &str) -> ApiError {
    ApiError::Io {
        message: format!("Timeout beim {op}"),
    }
}

/// Übersetzt jeden Fehler mit `Display` (in der Praxis: `russh::Error`) in eine generische
/// `ApiError::Ssh` — ohne dass diese Datei den konkreten Fehlertyp benennen (und damit
/// `russh` als eigene Abhängigkeit von `app` einführen) müsste.
fn ssh_to_api<E: std::fmt::Display>(err: E) -> ApiError {
    ApiError::Ssh {
        message: err.to_string(),
    }
}

/// `2>/dev/null || true` in `cmd_list_sessions`/`cmd_list_panes` schluckt zwar den
/// Exit-Code des eigentlichen tmux-Aufrufs — dieser Check bleibt trotzdem die einzige
/// Stelle, die "exit 127 → tmux fehlt" kennt, und wird auf jedes tmux-Kommando angewendet
/// (auch die beiden, deren Code praktisch immer 0 ist), damit ein zukünftiges Entfernen
/// des `|| true` die Erkennung nicht zusätzlich verdrahten muss.
fn check_tmux_exit(out: &ExecOutput) -> Result<(), ApiError> {
    if out.exit_code == Some(127) {
        Err(ApiError::TmuxMissing {
            message: "tmux nicht gefunden (exit 127)".to_string(),
        })
    } else {
        Ok(())
    }
}

/// `list-sessions` + `list-panes` ausführen und mergen — von `list_sessions` und
/// `start_project` (Kollisionsprüfung gegen laufende Namen) gemeinsam genutzt.
async fn running_sessions(conn: &SshConnection) -> Result<Vec<SessionInfo>, ApiError> {
    let sessions_out = conn
        .exec_capture(&tmux_cmd::cmd_list_sessions())
        .await
        .map_err(ssh_to_api)?;
    check_tmux_exit(&sessions_out)?;
    let panes_out = conn
        .exec_capture(&tmux_cmd::cmd_list_panes())
        .await
        .map_err(ssh_to_api)?;
    check_tmux_exit(&panes_out)?;
    Ok(merge(
        parse_sessions(&sessions_out.stdout),
        parse_panes(&panes_out.stdout),
    ))
}

/// Batcht `PtyEvent`s aus `rx` und sendet sie base64-kodiert über `channel`. Läuft als
/// eigener `tokio::spawn`-Task, solange die Session offen ist; endet, sobald `PtyEvent::Exit`
/// eintrifft (oder der Sender — der PTY-Reader-Task — ohne vorheriges `Exit` wegfällt).
///
/// `closing` (Fix Important): dasselbe `Arc<AtomicBool>`, das auch in der `SessionEntry` liegt
/// und von `close_session` VOR dem Schließen gesetzt wird. Ist es beim `PtyEvent::Exit` bereits
/// `true`, war der Exit die erwartete Folge eines selbst ausgelösten Detach (nicht ein
/// fremdverursachtes Prozessende) — dann wird `pty-exit` NICHT emittiert, aber trotzdem
/// geflusht und der (meist schon fehlende) Map-Eintrag idempotent entfernt.
fn spawn_forwarder(
    app: AppHandle,
    session_id: String,
    mut rx: mpsc::Receiver<PtyEvent>,
    channel: Channel<OutputChunk>,
    closing: Arc<AtomicBool>,
) {
    fn flush(buf: &mut Vec<u8>, channel: &Channel<OutputChunk>) {
        if buf.is_empty() {
            return;
        }
        let data_b64 = BASE64.encode(buf);
        // Ein fehlgeschlagener Send (Frontend/Fenster weg) darf den Forwarder nicht crashen —
        // er räumt bei PtyEvent::Exit trotzdem korrekt auf.
        let _ = channel.send(OutputChunk { data_b64 });
        buf.clear();
    }

    tokio::spawn(async move {
        let mut buf: Vec<u8> = Vec::new();
        let mut first_byte_at: Option<Instant> = None;

        loop {
            let sleep_branch = async {
                match first_byte_at {
                    Some(t) => sleep_until(t + Duration::from_millis(10)).await,
                    // Kein ungeflushtes Byte: dieser Zweig darf nie feuern — `pending()`
                    // hält ihn dauerhaft unerreicht, `select!` pollt ihn trotzdem (kein
                    // `if`-Guard nötig, aber äquivalent dazu).
                    None => std::future::pending().await,
                }
            };

            tokio::select! {
                event = rx.recv() => {
                    match event {
                        Some(PtyEvent::Data(data)) => {
                            if first_byte_at.is_none() {
                                first_byte_at = Some(Instant::now());
                            }
                            buf.extend_from_slice(&data);
                            let elapsed_ms = first_byte_at.map(|t| t.elapsed().as_millis() as u64);
                            if should_flush(buf.len(), elapsed_ms) {
                                flush(&mut buf, &channel);
                                first_byte_at = None;
                            }
                        }
                        Some(PtyEvent::Exit(_)) => {
                            flush(&mut buf, &channel);
                            // Fix Important: kein `pty-exit` bei selbst ausgelöstem Detach —
                            // siehe Doku oben und an `SessionEntry::closing`.
                            if !closing.load(Ordering::SeqCst) {
                                let _ = app.emit(
                                    "pty-exit",
                                    PtyExitEvent { session_id: session_id.clone(), reason: "exited" },
                                );
                            }
                            // `remove` ist ein No-op, falls `close_session` den Eintrag schon
                            // entfernt hat (Detach-Fall) — sonst räumt es hier den durch ein
                            // echtes Prozessende verwaisten Eintrag auf.
                            app.state::<AppState>().lock().await.sessions.remove(&session_id);
                            return;
                        }
                        None => {
                            // Reader-Task ist weg, ohne vorher `Exit` gesendet zu haben (laut
                            // `pty.rs` sendet er ihn immer zuletzt) — defensiv trotzdem
                            // flushen und den Forwarder beenden; die Session bleibt in der
                            // Map (kein bekannter Exit-Grund), spätere Writes schlagen dann
                            // beim nächsten SSH-Fehler sichtbar fehl.
                            flush(&mut buf, &channel);
                            return;
                        }
                    }
                }
                _ = sleep_branch => {
                    flush(&mut buf, &channel);
                    first_byte_at = None;
                }
            }
        }
    });
}

#[tauri::command]
pub async fn list_sessions(state: State<'_, AppState>) -> Result<SessionList, ApiError> {
    let conn = {
        let inner = state.lock().await;
        require_conn(&inner)?
    };
    let running = running_sessions(&conn).await?;

    let cfg = config::load_from(&config::config_path());
    let scan_out = conn
        .exec_capture(&tmux_cmd::cmd_scan_projects(&cfg.scan_paths))
        .await
        .map_err(ssh_to_api)?;

    let running_names: HashSet<String> = running.iter().map(|s| s.name.clone()).collect();

    let startable: Vec<Project> = scan_out
        .stdout
        .lines()
        .filter(|l| !l.is_empty())
        .filter_map(|path| {
            let display_name = Path::new(path).file_name()?.to_str()?.to_string();
            let session_name = format!("cc-{}", names::sanitize(&display_name));
            if running_names.contains(&session_name) {
                None
            } else {
                Some(Project {
                    path: path.to_string(),
                    name: display_name,
                })
            }
        })
        .collect();

    Ok(SessionList {
        running: running.into_iter().map(SessionInfoDto::from).collect(),
        startable,
    })
}

#[tauri::command]
pub async fn open_session(
    app: AppHandle,
    state: State<'_, AppState>,
    name: String,
    cols: u16,
    rows: u16,
    on_output: Channel<OutputChunk>,
) -> Result<String, ApiError> {
    let conn = {
        let inner = state.lock().await;
        require_conn(&inner)?
    };
    let mut pty = conn
        .open_pty(&tmux_cmd::cmd_attach(&name), cols as u32, rows as u32)
        .await
        .map_err(ssh_to_api)?;
    let output_rx = pty.take_output();
    let closing = Arc::new(AtomicBool::new(false));
    let pty_arc = Arc::new(tokio::sync::Mutex::new(Some(pty)));

    let session_id = {
        let mut inner = state.lock().await;
        let session_id = inner.alloc_session_id();
        inner.sessions.insert(
            session_id.clone(),
            SessionEntry {
                pty: pty_arc,
                closing: closing.clone(),
                name,
            },
        );
        session_id
    };

    spawn_forwarder(app, session_id.clone(), output_rx, on_output, closing);
    Ok(session_id)
}

#[tauri::command]
pub async fn start_project(
    app: AppHandle,
    state: State<'_, AppState>,
    path: String,
    cols: u16,
    rows: u16,
    on_output: Channel<OutputChunk>,
) -> Result<StartResult, ApiError> {
    let conn = {
        let inner = state.lock().await;
        require_conn(&inner)?
    };

    let running = running_sessions(&conn).await?;
    let existing: HashSet<String> = running.iter().map(|s| s.name.clone()).collect();

    let basename = Path::new(&path)
        .file_name()
        .and_then(|n| n.to_str())
        .unwrap_or(path.as_str())
        .to_string();
    let session_name =
        names::resolve_collision(&format!("cc-{}", names::sanitize(&basename)), &existing);

    let new_out = conn
        .exec_capture(&tmux_cmd::cmd_new_detached(&session_name, &path, "claude"))
        .await
        .map_err(ssh_to_api)?;
    check_tmux_exit(&new_out)?;
    if !new_out.success() {
        return Err(ApiError::Ssh {
            message: format!("tmux new-session fehlgeschlagen: {}", new_out.stderr),
        });
    }

    let mut pty = conn
        .open_pty(
            &tmux_cmd::cmd_attach(&session_name),
            cols as u32,
            rows as u32,
        )
        .await
        .map_err(ssh_to_api)?;
    let output_rx = pty.take_output();
    let closing = Arc::new(AtomicBool::new(false));
    let pty_arc = Arc::new(tokio::sync::Mutex::new(Some(pty)));

    let session_id = {
        let mut inner = state.lock().await;
        let session_id = inner.alloc_session_id();
        inner.sessions.insert(
            session_id.clone(),
            SessionEntry {
                pty: pty_arc,
                closing: closing.clone(),
                name: session_name.clone(),
            },
        );
        session_id
    };

    spawn_forwarder(app, session_id.clone(), output_rx, on_output, closing);
    Ok(StartResult {
        session_id,
        session_name,
    })
}

#[tauri::command]
pub async fn write_session(
    state: State<'_, AppState>,
    session_id: String,
    data_b64: String,
) -> Result<(), ApiError> {
    let bytes = BASE64
        .decode(data_b64.as_bytes())
        .map_err(|e| ApiError::Io {
            message: format!("ungültiges Base64 in write_session: {e}"),
        })?;

    // Fix Critical: nur den `Arc` klonen und den State-Lock sofort wieder freigeben — der
    // eigentliche Schreib-Await läuft danach ausschließlich gegen den Session-eigenen Mutex.
    let pty = {
        let inner = state.lock().await;
        inner
            .sessions
            .get(&session_id)
            .map(|e| e.pty.clone())
            .ok_or_else(|| session_not_found(&session_id))?
    };

    let mut guard = pty.lock().await;
    let handle = guard
        .as_mut()
        .ok_or_else(|| session_not_found(&session_id))?;
    match tokio::time::timeout(SESSION_IO_TIMEOUT, handle.write(&bytes)).await {
        Ok(res) => res.map_err(ApiError::from),
        Err(_) => Err(io_timeout("Schreiben in Session")),
    }
}

#[tauri::command]
pub async fn resize_session(
    state: State<'_, AppState>,
    session_id: String,
    cols: u16,
    rows: u16,
) -> Result<(), ApiError> {
    let pty = {
        let inner = state.lock().await;
        inner
            .sessions
            .get(&session_id)
            .map(|e| e.pty.clone())
            .ok_or_else(|| session_not_found(&session_id))?
    };

    let guard = pty.lock().await;
    let handle = guard
        .as_ref()
        .ok_or_else(|| session_not_found(&session_id))?;
    match tokio::time::timeout(SESSION_IO_TIMEOUT, handle.resize(cols as u32, rows as u32)).await {
        Ok(res) => res.map_err(ssh_to_api),
        Err(_) => Err(io_timeout("Resize der Session")),
    }
}

/// Detach: `pty.close()` läuft in einem eigenen `tokio::spawn` (kann laut `pty.rs` bis zu 2s
/// dauern — der Command-Aufruf selbst kehrt sofort zurück, Global Constraint). Die
/// tmux-Session lebt serverseitig weiter, nur der Reader-Task/Forwarder dieser App-Instanz
/// endet. Wie `disconnect` (Task 2) `Result<(), ()>` statt `()`: async Commands mit
/// `State<'_, _>`-Argument müssen laut Tauri `Result` liefern, auch wenn der IPC-Contract für
/// das Frontend nur `-> ()` vorsieht (löst immer auf, nie ein `Err`).
///
/// Fix Important: `closing` wird VOR dem Spawn gesetzt, nicht erst darin — der Forwarder-Task
/// kann das `PtyEvent::Exit`, das `pty.close()` auslöst, jederzeit danach empfangen und muss
/// das Flag dann bereits auf `true` sehen (siehe `spawn_forwarder`). Der Eintrag wird sofort
/// aus der Map entfernt (der Forwarder greift beim eigentlichen `close()`-Exit also ins Leere —
/// `remove` ist dort ein dokumentiertes No-op), der `PtyHandle` wird erst im Spawn aus dem
/// Session-Mutex `take()`n, damit ein parallel laufender `write_session`/`resize_session`
/// seinen bereits gehaltenen Lock zuerst normal beenden kann statt auf `Arc::try_unwrap` zu
/// warten (das könnte je nach Timing scheitern).
#[tauri::command]
pub async fn close_session(state: State<'_, AppState>, session_id: String) -> Result<(), ()> {
    let entry = {
        let mut inner = state.lock().await;
        inner.sessions.remove(&session_id)
    };
    if let Some(entry) = entry {
        entry.closing.store(true, Ordering::SeqCst);
        tokio::spawn(async move {
            let handle = entry.pty.lock().await.take();
            if let Some(handle) = handle {
                let _ = handle.close().await;
            }
        });
    }
    Ok(())
}

#[tauri::command]
pub async fn kill_session(
    app: AppHandle,
    state: State<'_, AppState>,
    name: String,
) -> Result<(), ApiError> {
    let conn = {
        let inner = state.lock().await;
        require_conn(&inner)?
    };
    let out = conn
        .exec_capture(&tmux_cmd::cmd_kill(&name))
        .await
        .map_err(ssh_to_api)?;

    check_tmux_exit(&out)?;
    if !out.success() {
        return Err(ApiError::Ssh {
            message: format!("tmux kill-session fehlgeschlagen: {}", out.stderr),
        });
    }

    let _ = app.emit("sessions-changed", ());
    Ok(())
}
