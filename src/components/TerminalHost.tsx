/**
 * Hauptfläche: zeigt das xterm.js-Terminal der aktiven Session (TermPool, Task 4). Kein eigenes
 * Terminal-Objekt hier — nur Host-`<div>` + drei Verantwortungen:
 *
 * 1. Sessionwechsel: vorheriges Terminal explizit ausblenden (Review-Fund M4-Task-4:
 *    `termPool.show()` blendet ein vorher sichtbares Terminal NICHT selbst aus), dann das neue
 *    zeigen.
 * 2. Host-Größenänderung → aktives Terminal per `fit()` neu einpassen (ändert sich dadurch die
 *    Größe wirklich, meldet xterms eigenes `onResize`, in `termPool.ensure()` verdrahtet, sie
 *    von selbst per `resize_session` ans Backend).
 * 3. `pty-exit`: zwei grundverschiedene reasons (Review-Fund M4-Task-5, Fix 1+2):
 *    - "exited": Session ist wirklich beendet → TermPool-Eintrag endgültig entsorgen,
 *      Store-Eintrag entfernen, ggf. zur nächsten offenen Session wechseln (Review-Fund
 *      M4-Task-4: `sessionStore.closed()` wechselt selbst nicht um) — ABER NUR, wenn die
 *      beendete Session auch die gerade aktive war (Fix 1: eine Hintergrund-Session, die
 *      endet, darf den Nutzer nicht aus der aktiven Session reißen).
 *    - "connectionLost": Task 6 re-attacht später mit derselben sessionId ins selbe Terminal
 *      → KEIN dispose, KEIN Entfernen aus dem Store, nur `markLost()` + Banner-Overlay, falls
 *      die betroffene Session gerade aktiv ist.
 *    In beiden Fällen zusätzlich ein dismissbarer Hinweis oben im Terminalbereich (nur für
 *    "exited" — der connectionLost-Fall hat sein eigenes, dauerhaftes Overlay statt eines
 *    wegklickbaren Einzeil-Hinweises).
 */
import { useEffect, useRef, useState } from "react";
import { onPtyExit } from "../lib/ipc";
import { nextActiveSessionId } from "../lib/sessionSwitch";
import * as termPool from "../lib/termPool";
import { useSessionStore } from "../stores/sessionStore";
import { SearchBar } from "./SearchBar";

// Nur noch für reason "exited" — "connectionLost" bekommt das dauerhafte Banner-Overlay unten
// statt eines wegklickbaren Einzeil-Hinweises (Fix 2).
interface ExitNotice {
  id: string;
  name: string;
}

export function TerminalHost() {
  const hostRef = useRef<HTMLDivElement>(null);
  const activeSessionId = useSessionStore((s) => s.activeSessionId);
  const hasOpenSessions = useSessionStore((s) => s.openSessions.size > 0);
  const activeIsLost = useSessionStore(
    (s) => (s.activeSessionId ? (s.openSessions.get(s.activeSessionId)?.lost ?? false) : false),
  );
  const prevActiveRef = useRef<string | null>(null);
  const [notices, setNotices] = useState<ExitNotice[]>([]);
  const [searchOpen, setSearchOpen] = useState(false);

  useEffect(() => {
    const prev = prevActiveRef.current;
    if (prev && prev !== activeSessionId) {
      termPool.hide(prev);
    }
    if (activeSessionId && hostRef.current) {
      termPool.show(activeSessionId, hostRef.current);
    }
    prevActiveRef.current = activeSessionId;
    // Beim Sessionwechsel die Suche schließen — ein offenes Suchfeld für "die andere Session"
    // wäre irreführend (Treffer würden für ein anderes Terminal gesucht als sichtbar ist).
    setSearchOpen(false);
  }, [activeSessionId]);

  // Strg+F öffnet die Scrollback-Suche für die aktive Session (Task 6). `preventDefault`
  // verhindert die Browser-eigene Seitensuche im WebView.
  useEffect(() => {
    function handleKeyDown(e: KeyboardEvent) {
      if ((e.ctrlKey || e.metaKey) && e.key.toLowerCase() === "f") {
        if (!useSessionStore.getState().activeSessionId) return;
        e.preventDefault();
        setSearchOpen(true);
      }
    }
    window.addEventListener("keydown", handleKeyDown);
    return () => window.removeEventListener("keydown", handleKeyDown);
  }, []);

  useEffect(() => {
    const host = hostRef.current;
    if (!host) return;
    const ro = new ResizeObserver(() => {
      const id = useSessionStore.getState().activeSessionId;
      if (id) termPool.fit(id);
    });
    ro.observe(host);
    return () => ro.disconnect();
  }, []);

  useEffect(() => {
    let cancelled = false;
    let unlisten: (() => void) | undefined;
    void onPtyExit(({ sessionId, reason }) => {
      const state = useSessionStore.getState();
      const name = state.openSessions.get(sessionId)?.name ?? sessionId;

      if (reason === "connectionLost") {
        // Fix 2: Terminal bleibt im TermPool, Session bleibt im Store — Task 6 re-attacht mit
        // derselben sessionId ins selbe Terminal. Kein Notice hier, das dauerhafte
        // Banner-Overlay unten übernimmt den Hinweis, solange die Session aktiv/lost ist.
        state.markLost(sessionId);
        return;
      }

      // reason === "exited": Session ist wirklich beendet.
      const wasActive = state.activeSessionId === sessionId;
      const orderedIds = Array.from(state.openSessions.keys());
      const next = nextActiveSessionId(orderedIds, sessionId);

      termPool.dispose(sessionId);
      state.closed(sessionId);
      // Fix 1: nur umschalten, wenn die beendete Session auch die aktive war — sonst reißt das
      // Enden einer Hintergrund-Session den Nutzer aus der gerade aktiven Session.
      if (wasActive && next) useSessionStore.getState().activated(next);

      setNotices((prev) => [...prev, { id: `${sessionId}-${Date.now()}`, name }]);
    }).then((fn) => {
      // Siehe App.tsx: Cleanup kann (StrictMode-Doppel-Mount) schon vor der `listen()`-Antwort
      // gelaufen sein — dann sofort wieder abmelden statt einen zweiten Listener leaken zu
      // lassen.
      if (cancelled) {
        fn();
      } else {
        unlisten = fn;
      }
    });
    return () => {
      cancelled = true;
      unlisten?.();
    };
  }, []);

  function dismissNotice(id: string) {
    setNotices((prev) => prev.filter((n) => n.id !== id));
  }

  return (
    <div className="terminal-area">
      {notices.length > 0 && (
        <div className="exit-notices">
          {notices.map((n) => (
            <div key={n.id} className="exit-notice">
              <span>„{n.name}“ wurde beendet.</span>
              <button type="button" onClick={() => dismissNotice(n.id)} aria-label="Hinweis schließen">
                ×
              </button>
            </div>
          ))}
        </div>
      )}
      {searchOpen && activeSessionId && (
        <SearchBar sessionId={activeSessionId} onClose={() => setSearchOpen(false)} />
      )}
      <div className="terminal-host" ref={hostRef}>
        {!hasOpenSessions && (
          <div className="terminal-empty-hint">
            Keine Session offen — links eine laufende oder startbare Session wählen.
          </div>
        )}
        {activeIsLost && (
          <div className="connection-lost-banner">Verbindung verloren – Reconnect folgt</div>
        )}
      </div>
    </div>
  );
}
