//! Session-Streaming-Commands (Kern von M4): `list_sessions`, `open_session`,
//! `start_project`, `write_session`, `resize_session`, `close_session`, `kill_session`.
//! Task 6 ergänzt: `reattach_lost_sessions` (Reconnect-Re-Attach, siehe
//! `reconnect_supervisor.rs`) und einen Verlust-Trigger (`note_ssh_failure`) an jeder Stelle,
//! die einen SSH-Fehlpfad in `ApiError` übersetzt.
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
//! `start_project`/`reattach_lost_sessions`): liest `PtyEvent`s aus `take_output()`, batcht
//! Bytes über die reine Entscheidungsfunktion `claudedeck_core::util::should_flush` (>=32 KiB
//! ODER >=10ms seit dem ersten ungeflushten Byte) und sendet den Batch base64-kodiert über den
//! `Channel<OutputChunk>` ans Frontend. Bei `PtyEvent::Exit` wird der Restpuffer geflusht;
//! danach entscheidet `closing`/`lost` (Fix Important + Task-6-Auflage B/C), was passiert:
//! - `closing == false`: ein `exit_code` von `None` (kein `ChannelMsg::ExitStatus` je
//!   empfangen) ist ein starkes Indiz für einen toten Transport statt eines echten
//!   Prozessendes (ein regulärer `tmux kill-session`/Prozess-Exit liefert normalerweise einen
//!   Exit-Code, bevor der Kanal schließt) — statt hier zu raten, wird nur `trigger_loss()`
//!   aufgerufen; die eigentliche Klassifizierung (Sessions verlieren, `pty-exit
//!   reason:"connectionLost"` emittieren, PTYs schließen) läuft zentral und einmalig in
//!   `reconnect_supervisor::mark_all_lost`. Ein `exit_code` von `Some(_)` ist ein echtes
//!   Prozessende → `pty-exit{reason:"exited"}` + Map-Eintrag entfernen.
//! - `closing == true` (Detach via `close_session` ODER Verlust via `mark_all_lost` — beide
//!   setzen `closing` VOR dem `pty.close()`-Spawn): kein `pty-exit` hier. Der Map-Eintrag wird
//!   nur entfernt, wenn NICHT `lost` (Detach-Fall) — eine `lost`-Session bleibt für den
//!   Re-Attach in der Map stehen.

use std::collections::HashSet;
use std::path::Path;
use std::sync::atomic::{AtomicBool, AtomicU32, Ordering};
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
use claudedeck_core::tmux::parser::{
    merge, parse_panes, parse_projects, parse_sessions, SessionInfo, SessionKind,
};
use claudedeck_core::util::should_flush;

use crate::error::ApiError;
use crate::reconnect_supervisor::ReconnectSupervisor;
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

/// `pub(crate)` (statt privat wie vor Task 6) — `reconnect_supervisor::mark_all_lost` baut
/// dieselbe Struktur direkt, um `pty-exit{reason:"connectionLost"}` zu emittieren, ohne einen
/// eigenen, zweiten Event-Payload-Typ zu duplizieren.
#[derive(Serialize, Clone)]
#[serde(rename_all = "camelCase")]
pub(crate) struct PtyExitEvent {
    pub(crate) session_id: String,
    pub(crate) reason: &'static str,
}

/// Task-6-Ergänzung: Payload des neuen `session-reattached`-Events (Auflage C). Erweitert den
/// im Plan dokumentierten IPC-Contract um dieses eine Event — siehe Task-6-Report, Abschnitt
/// "Re-Attach-Variante", für die Begründung (serverseitiges Re-Attach auf denselben
/// `Channel<OutputChunk>` statt eines Frontend-getriebenen erneuten `open_session`).
#[derive(Serialize, Clone)]
#[serde(rename_all = "camelCase")]
struct SessionReattachedEvent {
    session_id: String,
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
    /// Unix-Sekunden der neuesten Änderung — Grundlage der Sortierung „Zuletzt aktiv", die für
    /// Projekte vorher mangels Zeitstempel wirkungslos war.
    pub modified: i64,
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
pub(crate) fn require_conn(inner: &AppInner) -> Result<Arc<SshConnection>, ApiError> {
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

/// Task-6-Ergänzung: eigene, sprechendere Meldung für eine `session_id`, die es zwar noch gibt,
/// deren PTY aber wegen eines Verbindungsverlusts gerade `None` ist (wartet auf Re-Attach) —
/// unterscheidbar von `session_not_found` (Session existiert wirklich nicht mehr), damit das
/// Frontend bei Bedarf differenzieren kann (aktuell zeigt es ohnehin schon das dauerhafte
/// `connection-lost-banner`, siehe `TerminalHost.tsx`).
fn session_lost(session_id: &str) -> ApiError {
    ApiError::Io {
        message: format!("Session {session_id} wartet auf Reconnect"),
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

/// Task-6-Ergänzung: wie `ssh_to_api`, meldet den Fehlpfad aber zusätzlich dem
/// Reconnect-Supervisor (`trigger_loss` — idempotent, siehe `reconnect_supervisor.rs`), BEVOR
/// er in eine `ApiError` übersetzt wird. Das ist der zweite der beiden im Auftrag genannten
/// Verlust-Trigger ("Fehlpfade von write/exec"), neben dem periodischen Keepalive im
/// Supervisor selbst. Wird an JEDER Stelle genutzt, die einen echten SSH-Transport-Fehler
/// (nicht: `TmuxMissing`, nicht: "Session-ID unbekannt") in `ApiError` übersetzt.
pub(crate) fn note_ssh_failure<E: std::fmt::Display>(app: &AppHandle, err: E) -> ApiError {
    app.state::<ReconnectSupervisor>().trigger_loss();
    ssh_to_api(err)
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
async fn running_sessions(
    app: &AppHandle,
    conn: &SshConnection,
) -> Result<Vec<SessionInfo>, ApiError> {
    let sessions_out = conn
        .exec_capture(&tmux_cmd::cmd_list_sessions())
        .await
        .map_err(|e| note_ssh_failure(app, e))?;
    check_tmux_exit(&sessions_out)?;
    let panes_out = conn
        .exec_capture(&tmux_cmd::cmd_list_panes())
        .await
        .map_err(|e| note_ssh_failure(app, e))?;
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
/// `closing`/`lost`: siehe Moduldoku oben und `state.rs::SessionEntry`.
fn spawn_forwarder(
    app: AppHandle,
    session_id: String,
    mut rx: mpsc::Receiver<PtyEvent>,
    channel: Channel<OutputChunk>,
    closing: Arc<AtomicBool>,
    lost: Arc<AtomicBool>,
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
                        Some(PtyEvent::Exit(exit_code)) => {
                            flush(&mut buf, &channel);
                            let is_closing = closing.load(Ordering::SeqCst);
                            let is_lost = lost.load(Ordering::SeqCst);

                            if is_closing {
                                // Detach (`close_session`) ODER bereits durch
                                // `mark_all_lost` als Verlust markiert (beide setzen
                                // `closing` VOR dem `pty.close()`-Spawn) — kein `pty-exit`
                                // hier. Bei `lost==true` bleibt der Map-Eintrag für den
                                // Re-Attach stehen; sonst (Detach) ist `remove` idempotent
                                // (der Command hat den Eintrag meist schon selbst entfernt).
                                if !is_lost {
                                    app.state::<AppState>().lock().await.sessions.remove(&session_id);
                                }
                                return;
                            }

                            if exit_code.is_none() {
                                // Kein selbst ausgelöstes Schließen UND kein Exit-Code vom
                                // Server — starkes Indiz für einen toten Transport statt
                                // eines echten Prozessendes (siehe Moduldoku). Überlässt die
                                // Klassifizierung zentral `mark_all_lost`, statt hier über
                                // eine einzelne Session zu entscheiden.
                                app.state::<ReconnectSupervisor>().trigger_loss();
                                return;
                            }

                            // Echtes Prozessende mit bekanntem Exit-Code.
                            let _ = app.emit(
                                "pty-exit",
                                PtyExitEvent { session_id: session_id.clone(), reason: "exited" },
                            );
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
pub async fn list_sessions(
    app: AppHandle,
    state: State<'_, AppState>,
) -> Result<SessionList, ApiError> {
    let conn = {
        let inner = state.lock().await;
        require_conn(&inner)?
    };
    let running = running_sessions(&app, &conn).await?;

    let cfg = config::load_from(&config::config_path());
    let scan_out = conn
        .exec_capture(&tmux_cmd::cmd_scan_projects(
            &cfg.scan_paths,
            &cfg.project_markers,
        ))
        .await
        .map_err(|e| note_ssh_failure(&app, e))?;

    let running_names: HashSet<String> = running.iter().map(|s| s.name.clone()).collect();

    // Zerlegen macht `parse_projects` (pure, mit Fixtures getestet) — hier bleibt nur die
    // Fachregel: was schon als Session läuft, gehört nicht mehr unter „Startbar".
    let startable: Vec<Project> = parse_projects(&scan_out.stdout)
        .into_iter()
        .filter_map(|entry| {
            let session_name = format!("cc-{}", names::sanitize(&entry.name));
            if running_names.contains(&session_name) {
                None
            } else {
                Some(Project {
                    path: entry.path,
                    name: entry.name,
                    modified: entry.modified,
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
        .map_err(|e| note_ssh_failure(&app, e))?;
    let output_rx = pty.take_output();
    let closing = Arc::new(AtomicBool::new(false));
    let lost = Arc::new(AtomicBool::new(false));
    let pty_arc = Arc::new(tokio::sync::Mutex::new(Some(pty)));

    let session_id = {
        let mut inner = state.lock().await;
        let session_id = inner.alloc_session_id();
        inner.sessions.insert(
            session_id.clone(),
            SessionEntry {
                pty: pty_arc,
                closing: closing.clone(),
                lost: lost.clone(),
                channel: on_output.clone(),
                cols: Arc::new(AtomicU32::new(cols as u32)),
                rows: Arc::new(AtomicU32::new(rows as u32)),
                name,
            },
        );
        session_id
    };

    spawn_forwarder(app, session_id.clone(), output_rx, on_output, closing, lost);
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

    let running = running_sessions(&app, &conn).await?;
    let existing: HashSet<String> = running.iter().map(|s| s.name.clone()).collect();

    let basename = Path::new(&path)
        .file_name()
        .and_then(|n| n.to_str())
        .unwrap_or(path.as_str())
        .to_string();
    let session_name =
        names::resolve_collision(&format!("cc-{}", names::sanitize(&basename)), &existing);

    let cfg = config::load_from(&config::config_path());

    let new_out = conn
        .exec_capture(&tmux_cmd::cmd_new_detached(
            &session_name,
            &path,
            // Model/Effort aus der Config — der Regler im Befehls-Panel schreibt sie dorthin.
            &tmux_cmd::claude_invocation(
                cfg.defaults.model.as_deref(),
                cfg.defaults.effort.as_deref(),
            ),
        ))
        .await
        .map_err(|e| note_ssh_failure(&app, e))?;
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
        .map_err(|e| note_ssh_failure(&app, e))?;
    let output_rx = pty.take_output();
    let closing = Arc::new(AtomicBool::new(false));
    let lost = Arc::new(AtomicBool::new(false));
    let pty_arc = Arc::new(tokio::sync::Mutex::new(Some(pty)));

    let session_id = {
        let mut inner = state.lock().await;
        let session_id = inner.alloc_session_id();
        inner.sessions.insert(
            session_id.clone(),
            SessionEntry {
                pty: pty_arc,
                closing: closing.clone(),
                lost: lost.clone(),
                channel: on_output.clone(),
                cols: Arc::new(AtomicU32::new(cols as u32)),
                rows: Arc::new(AtomicU32::new(rows as u32)),
                name: session_name.clone(),
            },
        );
        session_id
    };

    spawn_forwarder(app, session_id.clone(), output_rx, on_output, closing, lost);
    Ok(StartResult {
        session_id,
        session_name,
    })
}

#[tauri::command]
pub async fn write_session(
    app: AppHandle,
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
    let (pty, lost) = {
        let inner = state.lock().await;
        let entry = inner
            .sessions
            .get(&session_id)
            .ok_or_else(|| session_not_found(&session_id))?;
        (entry.pty.clone(), entry.lost.clone())
    };
    if lost.load(Ordering::SeqCst) {
        return Err(session_lost(&session_id));
    }

    let mut guard = pty.lock().await;
    let handle = guard
        .as_mut()
        .ok_or_else(|| session_not_found(&session_id))?;
    match tokio::time::timeout(SESSION_IO_TIMEOUT, handle.write(&bytes)).await {
        Ok(res) => res.map_err(|e| note_ssh_failure(&app, e)),
        Err(_) => {
            app.state::<ReconnectSupervisor>().trigger_loss();
            Err(io_timeout("Schreiben in Session"))
        }
    }
}

#[tauri::command]
pub async fn resize_session(
    app: AppHandle,
    state: State<'_, AppState>,
    session_id: String,
    cols: u16,
    rows: u16,
) -> Result<(), ApiError> {
    let (pty, lost, cols_atomic, rows_atomic) = {
        let inner = state.lock().await;
        let entry = inner
            .sessions
            .get(&session_id)
            .ok_or_else(|| session_not_found(&session_id))?;
        (
            entry.pty.clone(),
            entry.lost.clone(),
            entry.cols.clone(),
            entry.rows.clone(),
        )
    };
    if lost.load(Ordering::SeqCst) {
        return Err(session_lost(&session_id));
    }

    let guard = pty.lock().await;
    let handle = guard
        .as_ref()
        .ok_or_else(|| session_not_found(&session_id))?;
    match tokio::time::timeout(SESSION_IO_TIMEOUT, handle.resize(cols as u32, rows as u32)).await {
        Ok(Ok(())) => {
            // Task-6-Ergänzung: letzte bekannte Größe merken, damit ein Re-Attach nach
            // Verbindungsverlust `open_pty` mit der aktuellen (nicht der ursprünglichen
            // `open_session`-Fallback-)Größe aufruft.
            cols_atomic.store(cols as u32, Ordering::Relaxed);
            rows_atomic.store(rows as u32, Ordering::Relaxed);
            Ok(())
        }
        Ok(Err(e)) => Err(note_ssh_failure(&app, e)),
        Err(_) => {
            app.state::<ReconnectSupervisor>().trigger_loss();
            Err(io_timeout("Resize der Session"))
        }
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
        .map_err(|e| note_ssh_failure(&app, e))?;

    check_tmux_exit(&out)?;
    if !out.success() {
        return Err(ApiError::Ssh {
            message: format!("tmux kill-session fehlgeschlagen: {}", out.stderr),
        });
    }

    let _ = app.emit("sessions-changed", ());
    Ok(())
}

/// Task-6-Kern (Auflage C, Re-Attach): nach einem erfolgreichen (Re-)Connect aufgerufen (aus
/// `commands::connection::do_connect_core`, sowohl bei einem normalen `connect()`-Aufruf — dort
/// i.d.R. No-Op, weil keine Session `lost` ist — als auch nach jedem erfolgreichen
/// Supervisor-Reconnect-Versuch). Für jede Session mit `lost == true`:
/// - `conn.open_pty(cmd_attach(name), …)` mit der zuletzt bekannten Größe (`SessionEntry::cols`/
///   `rows`, von `resize_session` nachgeführt) statt der ursprünglichen `open_session`-
///   Fallback-Größe.
/// - Erfolg: neuer `PtyHandle` in den (leeren) `pty`-Slot, neuer Forwarder auf DEMSELBEN
///   `Channel<OutputChunk>` (`SessionEntry::channel`, `Clone` — siehe `state.rs`) — das Frontend
///   merkt vom Re-Attach nur das `session-reattached`-Event (`sessionStore.reattached()`,
///   Auflage C), der Channel-Callback selbst läuft unverändert weiter.
/// - Fehlschlag (z.B. die tmux-Session existiert serverseitig nicht mehr, weil sie während des
///   Ausfalls extern gekillt wurde): Eintrag endgültig entfernen, `pty-exit{reason:"exited"}`
///   statt eines Re-Attachs — robuster als eine für immer `lost` bleibende Zombie-Session.
///
/// Fix Important (Review-Fund Task 6, Race): `closing`/`lost` werden NICHT wiederverwendet —
/// die bestehenden `Arc`s aus dem alten `SessionEntry` gehören noch dem ALTEN Forwarder-Task
/// (der ist erst beendet, wenn er sein `PtyEvent::Exit` verarbeitet hat, was bei einem schnellen
/// manuellen Reconnect durchaus erst NACH diesem Re-Attach passiert). Würde man dieselben Arcs
/// auf `false` zurücksetzen und an den neuen Forwarder weitergeben, sähe der alte Forwarder
/// `closing==false` beim eigenen `Exit` und würde das fälschlich als toten Transport werten
/// (`trigger_loss()`) statt als sein eigenes, gewolltes Verstummen. Stattdessen werden für jede
/// Session FRISCHE `Arc::new(AtomicBool::new(false))` alloziert, in den `SessionEntry` in der
/// Map geschrieben (damit `write_session`/`resize_session`/`close_session` ab sofort diese
/// neuen Arcs sehen) UND an den neuen Forwarder gegeben. Der alte Forwarder behält seine alten
/// Arcs, die durch `mark_all_lost` bereits auf `true` stehen und dort bleiben — er ist damit
/// inert, ganz gleich, wann sein `Exit` eintrifft.
pub(crate) async fn reattach_lost_sessions(app: &AppHandle, state: &AppState) {
    let conn = {
        let inner = state.lock().await;
        match inner.conn.clone() {
            Some(c) => c,
            None => return,
        }
    };

    #[allow(clippy::type_complexity)]
    let lost_sessions: Vec<(
        String,
        String,
        Arc<tokio::sync::Mutex<Option<claudedeck_core::ssh::PtyHandle>>>,
        Channel<OutputChunk>,
        u32,
        u32,
    )> = {
        let inner = state.lock().await;
        inner
            .sessions
            .iter()
            .filter(|(_, e)| e.lost.load(Ordering::SeqCst))
            .map(|(id, e)| {
                (
                    id.clone(),
                    e.name.clone(),
                    e.pty.clone(),
                    e.channel.clone(),
                    e.cols.load(Ordering::Relaxed),
                    e.rows.load(Ordering::Relaxed),
                )
            })
            .collect()
    };

    for (id, name, pty_arc, channel, cols, rows) in lost_sessions {
        match conn
            .open_pty(&tmux_cmd::cmd_attach(&name), cols, rows)
            .await
        {
            Ok(mut new_pty) => {
                let output_rx = new_pty.take_output();
                *pty_arc.lock().await = Some(new_pty);

                // Frische Arcs statt der alten, noch vom vorherigen Forwarder gehaltenen —
                // siehe Fix-Doku oben. Sofort in den Map-Eintrag zurückschreiben, damit
                // gleichzeitige write_session/resize_session/close_session-Aufrufe ab jetzt
                // konsistent die neuen Arcs sehen.
                let closing = Arc::new(AtomicBool::new(false));
                let lost_flag = Arc::new(AtomicBool::new(false));
                {
                    let mut inner = state.lock().await;
                    if let Some(entry) = inner.sessions.get_mut(&id) {
                        entry.closing = closing.clone();
                        entry.lost = lost_flag.clone();
                    }
                }

                spawn_forwarder(
                    app.clone(),
                    id.clone(),
                    output_rx,
                    channel,
                    closing,
                    lost_flag,
                );
                let _ = app.emit(
                    "session-reattached",
                    SessionReattachedEvent { session_id: id },
                );
            }
            Err(_) => {
                state.lock().await.sessions.remove(&id);
                let _ = app.emit(
                    "pty-exit",
                    PtyExitEvent {
                        session_id: id,
                        reason: "exited",
                    },
                );
            }
        }
    }
}
