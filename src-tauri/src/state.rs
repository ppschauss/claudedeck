//! App-globaler Zustand: eine `SshConnection` (in `Arc`) + die Map laufender PTY-Sessions.
//!
//! Review-Fund M4-Task-3 (Critical): eine frühere Version dieses Kommentars begründete einen
//! einzigen `tokio::sync::Mutex`, der bewusst über SSH-Awaits hinweg gehalten wurde (z.B.
//! `conn.open_pty(..).await` UNTER dem `AppState`-Lock). Das bedeutet: ein hängender SSH-Aufruf
//! (Netz weg, Server hängt) blockiert den GESAMTEN State — jede andere Session, jedes `write`,
//! sogar `disconnect` — bis der eine Await zurückkehrt oder timeoutet. Jetzt gilt stattdessen:
//! der `AppState`-Mutex wird nur so lange gehalten, wie es braucht, um ein `Arc` zu klonen bzw.
//! einen Map-Eintrag zu holen/einzufügen — der Guard wird VOR jedem SSH-Await gedroppt.
//! `SshConnection::exec_capture`/`open_pty` nehmen bereits `&self` (geprüft, keine Änderung an
//! `claudedeck-core` nötig); `Arc<SshConnection>` reicht daher aus, um sie außerhalb des Locks
//! aufzurufen, ohne `SshConnection` selbst klonbar machen zu müssen (kleinstmöglicher Eingriff).

use std::collections::HashMap;
use std::sync::atomic::{AtomicBool, AtomicU32, Ordering};
use std::sync::Arc;

use claudedeck_core::ssh::{PtyHandle, SshConnection};
use tokio::sync::Mutex;

use crate::commands::sessions::OutputChunk;

/// Eine laufende, an diese App-Instanz angehängte PTY-Session.
///
/// `pty`: `Arc<tokio::Mutex<Option<PtyHandle>>>` statt eines nackten `PtyHandle` unter dem
/// globalen `AppState`-Mutex (Review-Fund, Critical) — `write_session`/`resize_session` klonen
/// nur das `Arc` unterm kurzen State-Lock und locken danach ausschließlich diesen
/// Session-eigenen Mutex; paralleles Schreiben in verschiedene Sessions blockiert sich damit
/// nicht mehr gegenseitig, und ein hängender SSH-Await in Session A blockiert nicht mehr
/// Session B oder `conn`. Der innere `Option` (statt direkt `PtyHandle`) ist nötig, weil
/// `PtyHandle::close(self)` den Wert konsumiert (`self`, nicht `&self`/`&mut self`) —
/// `close_session` `take()`t den `PtyHandle` aus dem Mutex, sobald der Lock frei wird, statt
/// per `Arc::try_unwrap` zu raten, ob gerade sonst niemand mehr eine Referenz hält (Race mit
/// einem parallel laufenden `write_session`/`resize_session`). Nach dem `take()` sehen spätere
/// Zugriffe `None` und scheitern sauber mit einer `ApiError::Io`, statt auf einen bereits
/// geschlossenen Kanal zu schreiben. Während einer Reconnect-Pause (`lost == true`) ist `pty`
/// ebenfalls `None` — nicht weil die Session geschlossen wurde, sondern weil ihr PTY-Kanal zur
/// toten Verbindung gehörte und explizit geschlossen wurde (Auflage B); der Slot wird beim
/// erfolgreichen Re-Attach (`commands::sessions::reattach_lost_sessions`) neu befüllt.
///
/// `closing`: von `close_session`/dem Reconnect-Supervisor und dem Forwarder-Task geteiltes
/// Flag (Review-Fund M4-Task-3, Important). Wird VOR dem Spawn von `pty.close()` gesetzt — der
/// Forwarder prüft es beim `PtyEvent::Exit`, um ein selbst ausgelöstes Schließen (Detach ODER
/// Verbindungsverlust) von einem echten, fremdverursachten Prozessende zu unterscheiden (nur
/// Letzteres emittiert `pty-exit{reason:"exited"}` ans Frontend). Siehe `commands/sessions.rs`.
///
/// `lost`: Task-6-Ergänzung (Auflage C/Reconnect). `true`, während diese Session auf ein
/// Re-Attach nach Verbindungsverlust wartet — steuert zusammen mit `closing`, ob der Forwarder
/// beim `PtyEvent::Exit` den Map-Eintrag entfernen darf (nein, solange `lost`) und ob
/// `reattach_lost_sessions` diese Session überhaupt anfasst.
///
/// `channel`/`cols`/`rows`: Task-6-Ergänzung fürs Re-Attach. `tauri::ipc::Channel` ist `Clone`
/// (geprüft laut Auftrag) — derselbe Channel, den das Frontend beim ursprünglichen
/// `open_session`/`start_project` übergeben hat, wird für den neuen Forwarder nach einem
/// erfolgreichen Reconnect wiederverwendet, statt dass das Frontend selbst erneut
/// `open_session` aufrufen und die `sessionId` im Store/TermPool austauschen müsste (siehe
/// Task-6-Report, Abschnitt "Re-Attach-Variante"). `cols`/`rows` sind `AtomicU32`, damit
/// `resize_session` die zuletzt gemeldete Terminalgröße lock-frei nachführen kann (gebraucht,
/// um beim Re-Attach mit der richtigen Größe zu `open_pty` statt der ursprünglichen
/// `open_session`-Fallback-Größe).
pub struct SessionEntry {
    pub pty: Arc<Mutex<Option<PtyHandle>>>,
    pub closing: Arc<AtomicBool>,
    pub lost: Arc<AtomicBool>,
    pub channel: tauri::ipc::Channel<OutputChunk>,
    pub cols: Arc<AtomicU32>,
    pub rows: Arc<AtomicU32>,
    pub name: String,
}

/// Schließt JEDEN `PtyHandle` einer bereits aus der Map entfernten Session-Sammlung explizit
/// (Auflage B aus dem Task-6-Ledger: "alle PtyHandles explizit close()n … kein Verlassen auf
/// Drop-Kaskade") — ein `tokio::spawn` pro Session, nie blockierend (Global Constraint: PTY-
/// `close()` kann laut `pty.rs` bis zu 2s dauern). Setzt `closing=true` vorher, damit ein noch
/// laufender Forwarder das dadurch ausgelöste `PtyEvent::Exit` nicht fälschlich als "echtes"
/// Prozessende meldet — dieselbe Konvention wie `close_session`/Detach (Fix Important,
/// Task-3-Report). Geteilt von `disconnect()` (Auflage B) und dem Reentrancy-Cleanup-Pfad in
/// `commands::connection::do_connect_core` (Auflage A).
pub fn cleanup_sessions_fully(sessions: HashMap<String, SessionEntry>) {
    for (_, entry) in sessions {
        entry.closing.store(true, Ordering::SeqCst);
        tokio::spawn(async move {
            let handle = entry.pty.lock().await.take();
            if let Some(handle) = handle {
                let _ = handle.close().await;
            }
        });
    }
}

/// Innerer, durch `AppState`s Mutex geschützter Zustand.
#[derive(Default)]
pub struct AppInner {
    pub conn: Option<Arc<SshConnection>>,
    pub sessions: HashMap<String, SessionEntry>,
    next_id: u64,
}

impl AppInner {
    /// Vergibt eine neue, innerhalb dieser App-Instanz eindeutige Session-ID. Format ist
    /// bewusst opak (kein Bezug zu tmux-Namen) — Task 3 nutzt sie als Schlüssel für
    /// `sessions` und als `Channel`-Zielschlüssel im Frontend. Produktiv genutzt in
    /// `commands/sessions.rs` (Session öffnen/starten).
    pub fn alloc_session_id(&mut self) -> String {
        self.next_id += 1;
        format!("s{}", self.next_id)
    }
}

/// Von Tauri via `.manage(AppState::new())` gehaltener App-Zustand.
pub struct AppState {
    inner: Mutex<AppInner>,
}

impl AppState {
    pub fn new() -> Self {
        Self {
            inner: Mutex::new(AppInner::default()),
        }
    }

    /// Sperrt den inneren Zustand. Heißt bewusst nicht `inner()` — `tauri::State<'_, T>` hat
    /// selbst ein inhärentes `inner(&self) -> &T`, das beim Aufruf über `state.inner()` Vorrang
    /// vor Deref-Coercion zu `AppState::inner()` hätte (Methodenauflösung bevorzugt den
    /// Empfängertyp vor `Deref`-Zielen) und so lautlos `&AppState` statt `&Mutex<AppInner>`
    /// zurückgäbe.
    pub async fn lock(&self) -> tokio::sync::MutexGuard<'_, AppInner> {
        self.inner.lock().await
    }
}

impl Default for AppState {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn alloc_session_id_ist_monoton_und_eindeutig() {
        let mut inner = AppInner::default();
        assert_eq!(inner.alloc_session_id(), "s1");
        assert_eq!(inner.alloc_session_id(), "s2");
        assert_eq!(inner.alloc_session_id(), "s3");
    }
}
