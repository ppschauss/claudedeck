//! App-globaler Zustand: eine `SshConnection` + die Map laufender PTY-Sessions. Ein einziger
//! `tokio::sync::Mutex` (nicht `std::sync::Mutex`) schützt beides zusammen, weil `SshConnection`
//! und `PtyHandle` selbst async-Methoden haben, die über einen Lock-Guard hinweg awaitet werden
//! müssen (z.B. `conn.open_pty(..).await` in Task 3).

use std::collections::HashMap;

use claudedeck_core::ssh::{PtyHandle, SshConnection};
use tokio::sync::Mutex;

/// Eine laufende, an diese App-Instanz angehängte PTY-Session. `pty` ist das volle
/// `PtyHandle` (nicht nur die Write-Hälfte) — Task 3s Forwarder-Task braucht `take_output()`,
/// das nur auf dem ganzen Handle existiert; `write`/`resize`/`close` laufen dann exklusiv über
/// den `AppState`-Mutex statt über einen zweiten, session-eigenen Lock.
// `pty`/`name` werden erst von Task 3s Session-Commands befüllt und gelesen (open_session,
// write_session, list_sessions) — Struktur ist hier bereits vollständig angelegt, siehe
// Aufteilung im Plan-Brief.
#[allow(dead_code)]
pub struct SessionEntry {
    pub pty: PtyHandle,
    pub name: String,
}

/// Innerer, durch `AppState`s Mutex geschützter Zustand.
#[derive(Default)]
pub struct AppInner {
    pub conn: Option<SshConnection>,
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
        Self { inner: Mutex::new(AppInner::default()) }
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
