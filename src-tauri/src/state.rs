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
use std::sync::atomic::AtomicBool;
use std::sync::Arc;

use claudedeck_core::ssh::{PtyHandle, SshConnection};
use tokio::sync::Mutex;

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
/// geschlossenen Kanal zu schreiben.
///
/// `closing`: von `close_session` und dem Forwarder-Task geteiltes Flag (Review-Fund,
/// Important). `close_session` (Detach) setzt es VOR dem Spawn von `pty.close()` — der
/// Forwarder prüft es beim `PtyEvent::Exit`, um einen selbst ausgelösten Detach von einem
/// echten, fremdverursachten Prozessende zu unterscheiden (nur Letzteres emittiert `pty-exit`
/// ans Frontend). Siehe `commands/sessions.rs`.
pub struct SessionEntry {
    pub pty: Arc<Mutex<Option<PtyHandle>>>,
    pub closing: Arc<AtomicBool>,
    // Vom Task-3-Brief vorgesehen (u.a. für spätere Diagnose/Reattach-Anzeige), aber von
    // keinem aktuellen Command gelesen — `dead_code`-Warnung daher bewusst unterdrückt statt
    // das Feld zu entfernen und den Contract-Spielraum zu verlieren.
    #[allow(dead_code)]
    pub name: String,
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
    /// `sessions` und als `Channel`-Zielschlüssel im Frontend. Bislang nur über den Unit-Test
    /// unten aufgerufen; die Test-Erreichbarkeit zählt für `-D warnings` im Nicht-Test-Build
    /// nicht als Nutzung, daher `#[allow(dead_code)]`.
    #[allow(dead_code)]
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
