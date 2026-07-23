/**
 * Session-Switcher (kein Tab-Bar): drei Gruppen laut Task-5-Auftrag —
 * ● angehängt (im TermPool bereits offen, Klick wechselt nur um), ○ läuft (per `list_sessions`
 * bekannt, aber noch nicht angehängt, Klick ruft `open_session`) und + startbar (Projekte aus
 * den `scan_paths`, Klick ruft `start_project`). Aktualisiert sich bei `sessions-changed` und
 * beim Zurückkommen des Fenster-Fokus.
 */
import { type ReactNode, useCallback, useEffect, useMemo, useState } from "react";
import {
  b64ToBytes,
  listSessions,
  onSessionsChanged,
  openSession,
  resizeSession,
  startProject,
  writeSession,
  type OutputChunk,
  type Project,
} from "../lib/ipc";
import { describeApiError } from "../lib/apiError";
import * as termPool from "../lib/termPool";
import { useSessionStore } from "../stores/sessionStore";

// Fallback-Größe fürs `open_session`/`start_project`-IPC, bevor überhaupt ein Terminal-DOM-
// Element existiert, an dem `fit()` etwas messen könnte (siehe TerminalHost — sobald das
// Terminal sichtbar ist, korrigiert `fit()` cols/rows und meldet die echte Größe per
// `resize_session` nach).
const FALLBACK_COLS = 120;
const FALLBACK_ROWS = 30;

/**
 * Verdrahtet eine frisch geöffnete/gestartete Session komplett: legt den TermPool-Eintrag an
 * und trägt sie in den Store ein. Puffert Output-Chunks, die (laut IPC-Contract nicht
 * ausgeschlossen — Channel-Nachrichten und die `invoke()`-Antwort sind zwei getrennte
 * IPC-Wege) schon eintreffen, bevor die Session-ID vom Backend zurück ist, statt sie stillos zu
 * verlieren.
 */
async function attachAndTrack(
  open: (
    onOutput: (chunk: OutputChunk) => void,
  ) => Promise<{ sessionId: string; sessionName: string }>,
): Promise<string> {
  let sessionId: string | null = null;
  const pending: OutputChunk[] = [];

  const result = await open((chunk) => {
    if (sessionId) {
      termPool.write(sessionId, b64ToBytes(chunk.dataB64));
      useSessionStore.getState().outputReceived(sessionId, Date.now());
    } else {
      pending.push(chunk);
    }
  });

  sessionId = result.sessionId;
  const id = sessionId;
  termPool.ensure(
    id,
    (bytes) => {
      void writeSession(id, bytes);
    },
    (cols, rows) => {
      void resizeSession(id, cols, rows);
    },
  );
  for (const chunk of pending) {
    termPool.write(id, b64ToBytes(chunk.dataB64));
  }
  if (pending.length > 0) {
    useSessionStore.getState().outputReceived(id, Date.now());
  }
  useSessionStore.getState().opened(id, result.sessionName);
  return id;
}

export function Sidebar() {
  const running = useSessionStore((s) => s.running);
  const startable = useSessionStore((s) => s.startable);
  const openSessions = useSessionStore((s) => s.openSessions);
  const activeSessionId = useSessionStore((s) => s.activeSessionId);
  const [busyKey, setBusyKey] = useState<string | null>(null);
  const [error, setError] = useState<string | null>(null);

  const refresh = useCallback(async () => {
    try {
      const list = await listSessions();
      useSessionStore.getState().sessionsLoaded(list);
    } catch (err) {
      setError(describeApiError(err));
    }
  }, []);

  useEffect(() => {
    void refresh();
    const onFocus = () => void refresh();
    window.addEventListener("focus", onFocus);

    let cancelled = false;
    let unlisten: (() => void) | undefined;
    void onSessionsChanged(() => void refresh()).then((fn) => {
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
      window.removeEventListener("focus", onFocus);
      cancelled = true;
      unlisten?.();
    };
  }, [refresh]);

  const openNames = useMemo(() => {
    const names = new Set<string>();
    for (const s of openSessions.values()) names.add(s.name);
    return names;
  }, [openSessions]);

  const attached = useMemo(() => Array.from(openSessions.entries()), [openSessions]);
  const notAttached = useMemo(
    () => running.filter((r) => !openNames.has(r.name)),
    [running, openNames],
  );

  function handleActivate(sessionId: string) {
    useSessionStore.getState().activated(sessionId);
  }

  async function handleAttach(name: string) {
    setBusyKey(name);
    setError(null);
    try {
      await attachAndTrack((onOutput) =>
        openSession(name, FALLBACK_COLS, FALLBACK_ROWS, onOutput).then((sessionId) => ({
          sessionId,
          sessionName: name,
        })),
      );
    } catch (err) {
      setError(describeApiError(err));
    } finally {
      setBusyKey(null);
    }
  }

  async function handleStart(project: Project) {
    setBusyKey(project.path);
    setError(null);
    try {
      await attachAndTrack((onOutput) =>
        startProject(project.path, FALLBACK_COLS, FALLBACK_ROWS, onOutput),
      );
      // start_project emittiert (anders als kill_session) kein `sessions-changed` — ohne
      // manuellen Refresh bliebe das Projekt fälschlich zusätzlich in der "Startbar"-Gruppe
      // stehen, bis der nächste Fensterfokus oder ein Fremd-Event nachlädt.
      void refresh();
    } catch (err) {
      setError(describeApiError(err));
    } finally {
      setBusyKey(null);
    }
  }

  return (
    <aside className="sidebar">
      {error && <p className="error-text sidebar-error">{error}</p>}

      <SidebarGroup title="Angehängt">
        {attached.length === 0 && <p className="sidebar-empty">–</p>}
        <ul>
          {attached.map(([sessionId, s]) => (
            <li key={sessionId}>
              <button
                type="button"
                className={sessionId === activeSessionId ? "session-item active" : "session-item"}
                onClick={() => handleActivate(sessionId)}
              >
                <span className="dot dot-filled" aria-hidden="true">
                  ●
                </span>
                <span className="session-name">{s.name}</span>
                {s.activity.badge > 0 && <span className="badge">{s.activity.badge}</span>}
              </button>
            </li>
          ))}
        </ul>
      </SidebarGroup>

      <SidebarGroup title="Läuft">
        {notAttached.length === 0 && <p className="sidebar-empty">–</p>}
        <ul>
          {notAttached.map((s) => (
            <li key={s.name}>
              <button
                type="button"
                className="session-item"
                disabled={busyKey === s.name}
                onClick={() => void handleAttach(s.name)}
              >
                <span className="dot" aria-hidden="true">
                  ○
                </span>
                <span className="session-name">{s.name}</span>
              </button>
            </li>
          ))}
        </ul>
      </SidebarGroup>

      <SidebarGroup title="Startbar">
        {startable.length === 0 && <p className="sidebar-empty">–</p>}
        <ul>
          {startable.map((p) => (
            <li key={p.path}>
              <button
                type="button"
                className="session-item"
                disabled={busyKey === p.path}
                onClick={() => void handleStart(p)}
              >
                <span className="dot" aria-hidden="true">
                  +
                </span>
                <span className="session-name">{p.name}</span>
              </button>
            </li>
          ))}
        </ul>
      </SidebarGroup>
    </aside>
  );
}

function SidebarGroup({ title, children }: { title: string; children: ReactNode }) {
  return (
    <div className="sidebar-group">
      <h3>{title}</h3>
      {children}
    </div>
  );
}
