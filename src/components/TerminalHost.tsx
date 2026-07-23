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
 * 3. `pty-exit`: Session ist wirklich beendet (nicht nur detached — das Backend unterdrückt das
 *    Event bei einem selbst ausgelösten Detach) → TermPool-Eintrag endgültig entsorgen,
 *    Store-Eintrag entfernen, ggf. zur nächsten offenen Session wechseln (Review-Fund
 *    M4-Task-4: `sessionStore.closed()` wechselt selbst nicht um) und einen Hinweis anzeigen.
 */
import { useEffect, useRef, useState } from "react";
import { onPtyExit, type PtyExitEvent } from "../lib/ipc";
import { nextActiveSessionId } from "../lib/sessionSwitch";
import * as termPool from "../lib/termPool";
import { useSessionStore } from "../stores/sessionStore";

interface ExitNotice {
  id: string;
  name: string;
  reason: PtyExitEvent["reason"];
}

export function TerminalHost() {
  const hostRef = useRef<HTMLDivElement>(null);
  const activeSessionId = useSessionStore((s) => s.activeSessionId);
  const hasOpenSessions = useSessionStore((s) => s.openSessions.size > 0);
  const prevActiveRef = useRef<string | null>(null);
  const [notices, setNotices] = useState<ExitNotice[]>([]);

  useEffect(() => {
    const prev = prevActiveRef.current;
    if (prev && prev !== activeSessionId) {
      termPool.hide(prev);
    }
    if (activeSessionId && hostRef.current) {
      termPool.show(activeSessionId, hostRef.current);
    }
    prevActiveRef.current = activeSessionId;
  }, [activeSessionId]);

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
      const orderedIds = Array.from(state.openSessions.keys());
      const next = nextActiveSessionId(orderedIds, sessionId);

      termPool.dispose(sessionId);
      state.closed(sessionId);
      if (next) useSessionStore.getState().activated(next);

      setNotices((prev) => [...prev, { id: `${sessionId}-${Date.now()}`, name, reason }]);
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
              <span>
                „{n.name}“{" "}
                {n.reason === "exited" ? "wurde beendet." : "hat die Verbindung verloren."}
              </span>
              <button type="button" onClick={() => dismissNotice(n.id)} aria-label="Hinweis schließen">
                ×
              </button>
            </div>
          ))}
        </div>
      )}
      <div className="terminal-host" ref={hostRef}>
        {!hasOpenSessions && (
          <div className="terminal-empty-hint">
            Keine Session offen — links eine laufende oder startbare Session wählen.
          </div>
        )}
      </div>
    </div>
  );
}
