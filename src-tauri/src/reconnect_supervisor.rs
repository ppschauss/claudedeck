//! Reconnect-Supervisor (M4/M5-Task 6): erkennt Verbindungsverlust, markiert alle offenen
//! Sessions als `lost`, versucht mit `claudedeck_core::reconnect::attempt_delay`-Backoff
//! (3/6/12/dann dauerhaft 30s) automatisch neu zu verbinden und re-attacht bei Erfolg jede
//! verlorene Session serverseitig (`commands::sessions::reattach_lost_sessions`).
//!
//! ## Warum kein `SshConnection::disconnected()`-Watch
//!
//! `crates/claudedeck-core/src/ssh/connection.rs` (M2/M3) bietet kein solches Signal — geprüft
//! vor der Implementierung (siehe Task-6-Report). Verlust wird deshalb aus ZWEI Quellen erkannt
//! (wie im Auftrag vorgesehen):
//! 1. **Periodischer Keepalive** (`spawn_keepalive`): alle 30s `exec_capture("true")`, solange
//!    `AppState.conn` gesetzt ist. Schlägt der Aufruf fehl, wird `trigger_loss()` ausgelöst.
//! 2. **Fehlpfade von write/exec**: jede Stelle in `commands/sessions.rs`, die einen SSH-Fehler
//!    in `ApiError` übersetzt (`note_ssh_failure`) oder einen `SESSION_IO_TIMEOUT` erreicht,
//!    ruft ebenfalls `trigger_loss()`. Zusätzlich meldet der PTY-Forwarder selbst ein
//!    `PtyEvent::Exit(None)` (kein Exit-Code vom Server — starkes Indiz für einen toten
//!    Transport statt eines echten Prozessendes) als Trigger, statt es als "Session beendet"
//!    misszuinterpretieren (siehe `spawn_forwarder`-Doku).
//!
//! `trigger_loss` ist idempotent (`in_recovery`-Test-and-Set) — beliebig viele gleichzeitige
//! Trigger (Keepalive UND mehrere parallel scheiternde Session-Commands binnen Millisekunden
//! am selben Netzausfall) lösen nur EINE Recovery-Runde aus.
//!
//! ## Ablauf einer Recovery-Runde (`run_recovery`)
//!
//! 1. `mark_all_lost`: `conn = None`, für jede offene Session `closing=true`, `lost=true`,
//!    `pty.close()` (`tokio::spawn`, Auflage B — nie blockierend), `pty-exit
//!    {reason:"connectionLost"}` emittiert.
//! 2. Backoff-Schleife: pro Versuch `connection-state {state:"reconnecting", attempt,
//!    nextRetryInS}` emittieren, dann interruptible warten (`tokio::select!` zwischen
//!    `sleep(delay)` und dem manuellen `retry_notify`, geweckt von `connect()`/
//!    `accept_hostkey_and_connect()` — "Jetzt neu verbinden"-Button im ReconnectOverlay).
//!    Danach `do_connect_core` (dieselbe Funktion wie ein manueller Connect) — Erfolg beendet
//!    die Schleife (inkl. Re-Attach + `connection-state:"connected"`, das übernimmt
//!    `do_connect_core` selbst); `ApiError::AuthFailed` beendet die Schleife MIT
//!    `connection-state:"failed"` und OHNE weiteren Versuch (Global Constraint: kein Auto-Retry
//!    nach AuthFailed); jeder andere Fehler führt zum nächsten Versuch.
//! 3. Ein expliziter `disconnect()` während einer laufenden Runde setzt `cancelled` (siehe
//!    `cancel()`) — die Schleife bricht beim nächsten Aufwachen sauber ab, ohne "failed" zu
//!    emittieren (der Nutzer hat ja bewusst getrennt).

use std::sync::atomic::{AtomicBool, Ordering};

use tauri::{AppHandle, Emitter, Manager};
use tokio::sync::Notify;

use claudedeck_core::reconnect;
use claudedeck_core::ssh::HostkeyPolicy;

use crate::commands::connection::do_connect_core;
use crate::commands::sessions::PtyExitEvent;
use crate::error::ApiError;
use crate::state::AppState;

const KEEPALIVE_INTERVAL: std::time::Duration = std::time::Duration::from_secs(30);

/// Von `lib.rs` per `.manage()` gehaltener Supervisor-Zustand — getrennt von `AppState`, weil
/// er reine Steuerungs-/Signalisierungs-Primitive hält, keine Verbindungsdaten.
pub struct ReconnectSupervisor {
    /// Weckt den Backoff-`sleep` vorzeitig — der manuelle "Jetzt neu verbinden"-Button ruft
    /// `connect()`, das bei JEDEM Aufruf `wake_retry()` auslöst (unabhängig davon, ob der
    /// Supervisor gerade tatsächlich wartet — `Notify::notify_one()` ohne Empfänger ist sicher
    /// und wird nicht "nachgeholt").
    retry_notify: Notify,
    /// Weckt den äußeren "warte auf Verbindungsverlust"-Zustand.
    loss_notify: Notify,
    /// Test-and-Set-Guard: verhindert, dass zwei gleichzeitige Verlust-Trigger zwei parallele
    /// Recovery-Runden starten. Wird am Ende von `run_recovery` zurückgesetzt.
    in_recovery: AtomicBool,
    /// Von `disconnect()` gesetzt: eine laufende Recovery-Runde soll beim nächsten Aufwachen
    /// abbrechen, OHNE `connection-state:"failed"` zu emittieren (der Nutzer hat bewusst
    /// getrennt, das ist kein Fehlschlag). Wird zu Beginn jeder neuen Runde zurückgesetzt, damit
    /// ein alter, nie konsumierter Cancel keine künftige Runde sofort abwürgt.
    cancelled: AtomicBool,
    /// Serialisiert ALLE `do_connect_core`-Aufrufe (manueller Connect, Hostkey-Accept,
    /// Supervisor-Versuche) — verhindert zwei echte `SshConnection::connect`-Läufe gleichzeitig
    /// gegen denselben Host.
    pub(crate) connect_lock: tokio::sync::Mutex<()>,
}

impl ReconnectSupervisor {
    pub fn new() -> Self {
        Self {
            retry_notify: Notify::new(),
            loss_notify: Notify::new(),
            in_recovery: AtomicBool::new(false),
            cancelled: AtomicBool::new(false),
            connect_lock: tokio::sync::Mutex::new(()),
        }
    }

    /// Idempotenter Verlust-Trigger — siehe Moduldoku. Aufgerufen vom Keepalive, von
    /// `commands/sessions.rs`s `note_ssh_failure`/Timeout-Pfaden und vom Forwarder bei einem
    /// exit-code-losen `PtyEvent::Exit`.
    pub fn trigger_loss(&self) {
        if self
            .in_recovery
            .compare_exchange(false, true, Ordering::SeqCst, Ordering::SeqCst)
            .is_ok()
        {
            self.loss_notify.notify_one();
        }
    }

    /// Weckt einen laufenden Backoff-`sleep` vorzeitig (manueller Retry). Sicher aufzurufen,
    /// auch wenn der Supervisor gerade nicht in Recovery ist (No-Op, kein Effekt auf eine
    /// spätere Runde).
    pub fn wake_retry(&self) {
        self.retry_notify.notify_one();
    }

    /// Bricht eine laufende Recovery-Runde ab (aufgerufen von `disconnect()`). Sicher, auch
    /// wenn gerade keine Runde läuft.
    pub fn cancel(&self) {
        self.cancelled.store(true, Ordering::SeqCst);
        self.retry_notify.notify_one();
    }
}

impl Default for ReconnectSupervisor {
    fn default() -> Self {
        Self::new()
    }
}

#[derive(serde::Serialize, Clone)]
#[serde(rename_all = "camelCase")]
struct ConnectionStateEvent {
    state: &'static str,
    #[serde(skip_serializing_if = "Option::is_none")]
    attempt: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    next_retry_in_s: Option<u64>,
}

fn emit_state(
    app: &AppHandle,
    state: &'static str,
    attempt: Option<u32>,
    next_retry_in_s: Option<u64>,
) {
    let _ = app.emit(
        "connection-state",
        ConnectionStateEvent {
            state,
            attempt,
            next_retry_in_s,
        },
    );
}

/// Startet den Supervisor-Loop (wartet auf Verlust-Trigger) PLUS den periodischen Keepalive —
/// je ein `tokio::spawn`, beide laufen für die App-Lebensdauer. Aufgerufen einmal aus `lib.rs`s
/// `.setup()`.
pub fn spawn(app: AppHandle) {
    spawn_keepalive(app.clone());
    tokio::spawn(async move {
        loop {
            app.state::<ReconnectSupervisor>()
                .loss_notify
                .notified()
                .await;
            run_recovery(&app).await;
        }
    });
}

fn spawn_keepalive(app: AppHandle) {
    tokio::spawn(async move {
        let mut ticker = tokio::time::interval(KEEPALIVE_INTERVAL);
        ticker.tick().await; // erster Tick feuert sofort — überspringen, sonst Keepalive direkt beim Start
        loop {
            ticker.tick().await;
            let conn = { app.state::<AppState>().lock().await.conn.clone() };
            if let Some(conn) = conn {
                if conn.exec_capture("true").await.is_err() {
                    app.state::<ReconnectSupervisor>().trigger_loss();
                }
            }
        }
    });
}

/// Eine komplette Recovery-Runde: Sessions verlieren (`mark_all_lost`), dann Backoff-Schleife
/// bis Erfolg, `AuthFailed` oder expliziter Abbruch (`cancel()`).
async fn run_recovery(app: &AppHandle) {
    {
        let sup = app.state::<ReconnectSupervisor>();
        // Frischer Zyklus: ein alter, nie konsumierter Cancel-Wunsch (aus einer VORHERIGEN
        // Runde, die schon beendet war, als `disconnect()` lief) darf diese neue Runde nicht
        // sofort abwürgen.
        sup.cancelled.store(false, Ordering::SeqCst);
    }

    let state = app.state::<AppState>();
    mark_all_lost(app, &state).await;

    let mut attempt: u32 = 0;
    loop {
        attempt += 1;
        let delay = reconnect::attempt_delay(attempt);
        emit_state(app, "reconnecting", Some(attempt), Some(delay.as_secs()));

        {
            let sup = app.state::<ReconnectSupervisor>();
            tokio::select! {
                _ = tokio::time::sleep(delay) => {}
                _ = sup.retry_notify.notified() => {}
            }
        }

        if app
            .state::<ReconnectSupervisor>()
            .cancelled
            .swap(false, Ordering::SeqCst)
        {
            break; // expliziter disconnect() — kein "failed", kein weiterer Versuch
        }
        if state.lock().await.conn.is_some() {
            // Ein manueller `connect()`-Aufruf während des Wartens hat bereits erfolgreich neu
            // verbunden (inkl. Re-Attach, siehe `do_connect_core`) — kein zweiter Versuch nötig.
            break;
        }

        match do_connect_core(app, &state, None, HostkeyPolicy::Strict).await {
            Ok(()) => break,
            Err(ApiError::AuthFailed { .. }) => {
                // Global Constraint: nach AuthFailed nie automatisch weiterversuchen.
                emit_state(app, "failed", None, None);
                break;
            }
            Err(_) => continue,
        }
    }

    app.state::<ReconnectSupervisor>()
        .in_recovery
        .store(false, Ordering::SeqCst);
}

/// Setzt `conn = None`, markiert jede offene Session als verloren (`closing`+`lost`), schließt
/// ihre `PtyHandle`s explizit (`tokio::spawn`, Auflage B) und emittiert pro Session
/// `pty-exit{reason:"connectionLost"}`. Läuft VOR der ersten `"reconnecting"`-Emission (siehe
/// `run_recovery`).
async fn mark_all_lost(app: &AppHandle, state: &tauri::State<'_, AppState>) {
    #[allow(clippy::type_complexity)]
    let to_process: Vec<(
        String,
        std::sync::Arc<tokio::sync::Mutex<Option<claudedeck_core::ssh::PtyHandle>>>,
        std::sync::Arc<AtomicBool>,
        std::sync::Arc<AtomicBool>,
    )> = {
        let mut inner = state.lock().await;
        inner.conn = None;
        inner
            .sessions
            .iter()
            .map(|(id, e)| (id.clone(), e.pty.clone(), e.closing.clone(), e.lost.clone()))
            .collect()
    };

    for (id, pty, closing, lost) in to_process {
        closing.store(true, Ordering::SeqCst);
        lost.store(true, Ordering::SeqCst);
        tokio::spawn(async move {
            let handle = pty.lock().await.take();
            if let Some(handle) = handle {
                let _ = handle.close().await;
            }
        });
        let _ = app.emit(
            "pty-exit",
            PtyExitEvent {
                session_id: id,
                reason: "connectionLost",
            },
        );
    }
}
