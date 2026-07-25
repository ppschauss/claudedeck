/**
 * Session-Switcher (kein Tab-Bar): drei Gruppen laut Task-5-Auftrag —
 * ● angehängt (im TermPool bereits offen, Klick wechselt nur um), ○ läuft (per `list_sessions`
 * bekannt, aber noch nicht angehängt, Klick ruft `open_session`) und + startbar (Projekte aus
 * den `scan_paths`, Klick ruft `start_project`). Aktualisiert sich bei `sessions-changed` und
 * beim Zurückkommen des Fenster-Fokus.
 *
 * Task 6 ergänzt ein Kontextmenü ("⋮") pro angehängter Session: Benachrichtigungen aus/ein
 * (`notifyToggled`), Detach (`closeSession` + lokale Aufräumung, spiegelbildlich zum
 * "exited"-Zweig in `TerminalHost.tsx`s `pty-exit`-Handler) und Kill (`kill_session` nach
 * `window.confirm`). `start_project`-Fehler laufen als Toast statt (nur) als Inline-Text.
 */
import { type ReactNode, useCallback, useEffect, useMemo, useState } from "react";
import {
  b64ToBytes,
  closeSession,
  killSession,
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
import { findOpenByName } from "../lib/attachGuard";
import { activityState, type ActivityState } from "../lib/badges";
import { debounceTrailing } from "../lib/debounce";
import { useNow } from "../lib/useNow";
import { matchesQuery, sortByKey, type SortKey } from "../lib/sessionFilter";
import { nextActiveSessionId } from "../lib/sessionSwitch";
import * as termPool from "../lib/termPool";
import { useSessionStore } from "../stores/sessionStore";
import { useToastStore } from "../stores/toastStore";

// Fix 3 (Review-Fund M4-Task-5): der `fit()`-Aufruf in `TerminalHost`s `ResizeObserver` bleibt
// unverändert sofort/ungedrosselt (reine Layout-Anpassung, kein IPC) — aber das dadurch
// ausgelöste `onResize` von xterm (hier verdrahtet) triggert bei jedem Fenster-Resize-Frame
// erneut `resize_session`. Bei einem Drag-Resize wären das viele IPC-Calls pro Sekunde; 100ms
// Trailing-Debounce genügt (keine spürbare Verzögerung, aber max. 1 Call pro 100ms-Fenster mit
// den zuletzt gemessenen cols/rows).
const RESIZE_DEBOUNCE_MS = 100;

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
  const debouncedResize = debounceTrailing((cols: number, rows: number) => {
    void resizeSession(id, cols, rows);
  }, RESIZE_DEBOUNCE_MS);
  termPool.ensure(
    id,
    (bytes) => {
      void writeSession(id, bytes);
    },
    debouncedResize,
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
  const query = useSessionStore((s) => s.query);
  const sortBy = useSessionStore((s) => s.sortBy);
  const [busyKey, setBusyKey] = useState<string | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [menuOpenFor, setMenuOpenFor] = useState<string | null>(null);

  // Der Statuswechsel „arbeitet" → „fertig" entsteht durch Stille, also durch Zeitablauf und
  // nicht durch ein Ereignis — ohne eigenen Takt bliebe die Anzeige auf „arbeitet" stehen.
  // Sekundentakt reicht: der Schwellenwert liegt bei zwei Sekunden.
  const now = useNow(1000, openSessions.size > 0);

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

  // Kontextmenü schließen bei Klick außerhalb (einfaches Dropdown, kein Popover-API-Overkill).
  useEffect(() => {
    if (!menuOpenFor) return;
    function handleClick(e: MouseEvent) {
      const target = e.target as HTMLElement;
      if (!target.closest(".session-menu") && !target.closest(".session-menu-trigger")) {
        setMenuOpenFor(null);
      }
    }
    window.addEventListener("mousedown", handleClick);
    return () => window.removeEventListener("mousedown", handleClick);
  }, [menuOpenFor]);

  const openNames = useMemo(() => {
    const names = new Set<string>();
    for (const s of openSessions.values()) names.add(s.name);
    return names;
  }, [openSessions]);

  // Angehängte Sessions kennen ihre Startzeit nicht selbst (`OpenSession` speichert nur Name und
  // Activity) — sie steht in der `running`-Liste, die auch die bereits angehängten enthält.
  // Ohne diese Zuordnung wäre die Sortierung „Startzeit" für genau die Gruppe blind, die man am
  // häufigsten sortiert.
  const createdByName = useMemo(() => {
    const map = new Map<string, number>();
    for (const s of running) map.set(s.name, s.created);
    return map;
  }, [running]);

  const attached = useMemo(() => {
    const entries = Array.from(openSessions.entries()).filter(([, s]) =>
      matchesQuery(s.name, query),
    );
    return sortByKey(entries, sortBy, ([, s]) => ({
      name: s.name,
      createdAt: createdByName.get(s.name) ?? null,
      lastOutputAt: s.activity.lastOutputAt,
    }));
  }, [openSessions, query, sortBy, createdByName]);

  const notAttached = useMemo(() => {
    const list = running.filter((r) => !openNames.has(r.name) && matchesQuery(r.name, query));
    return sortByKey(list, sortBy, (r) => ({
      name: r.name,
      createdAt: r.created,
      lastOutputAt: null,
    }));
  }, [running, openNames, query, sortBy]);

  // Projekte aus den `scan_paths` haben keinerlei Zeitstempel — bei Zeitsortierungen landen sie
  // laut `sortByKey` hinten und werden dort nach Namen geordnet.
  const startableView = useMemo(() => {
    const list = startable.filter((p) => matchesQuery(p.name, query));
    return sortByKey(list, sortBy, (p) => ({
      name: p.name,
      createdAt: null,
      lastOutputAt: null,
    }));
  }, [startable, query, sortBy]);

  function handleActivate(sessionId: string) {
    useSessionStore.getState().activated(sessionId);
  }

  async function handleAttach(name: string) {
    // Fix 4 (Doppel-Attach-Guard): `notAttached` filtert bereits offene Namen aus der Liste,
    // aber zwischen Klick und Re-Render (oder bei einem parallel eintreffenden
    // `sessions-changed`/Fokus-Refresh) kann derselbe Name erneut hier landen — dann statt
    // eines zweiten `open_session`-Attaches nur auf die vorhandene Session umschalten.
    const existingId = findOpenByName(useSessionStore.getState().openSessions, name);
    if (existingId) {
      useSessionStore.getState().activated(existingId);
      return;
    }

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
    try {
      await attachAndTrack((onOutput) =>
        startProject(project.path, FALLBACK_COLS, FALLBACK_ROWS, onOutput),
      );
      // start_project emittiert (anders als kill_session) kein `sessions-changed` — ohne
      // manuellen Refresh bliebe das Projekt fälschlich zusätzlich in der "Startbar"-Gruppe
      // stehen, bis der nächste Fensterfokus oder ein Fremd-Event nachlädt.
      void refresh();
    } catch (err) {
      // Task 6: start_project-Fehler (z.B. tmuxMissing) als Toast statt Inline-Text.
      useToastStore.getState().push(describeApiError(err));
    } finally {
      setBusyKey(null);
    }
  }

  function handleToggleMenu(sessionId: string) {
    setMenuOpenFor((cur) => (cur === sessionId ? null : sessionId));
  }

  function handleToggleNotify(sessionId: string) {
    useSessionStore.getState().notifyToggled(sessionId);
    setMenuOpenFor(null);
  }

  async function handleDetach(sessionId: string) {
    setMenuOpenFor(null);
    const state = useSessionStore.getState();
    const wasActive = state.activeSessionId === sessionId;
    const orderedIds = Array.from(state.openSessions.keys());
    const next = nextActiveSessionId(orderedIds, sessionId);

    await closeSession(sessionId);
    termPool.dispose(sessionId);
    state.closed(sessionId);
    if (wasActive && next) useSessionStore.getState().activated(next);
  }

  async function handleKill(name: string) {
    setMenuOpenFor(null);
    if (!window.confirm(`Session „${name}“ wirklich beenden (tmux kill-session)?`)) return;
    try {
      await killSession(name);
      // kill_session emittiert `sessions-changed` (Refresh der "Läuft"-Liste); ein evtl. noch
      // angehängtes Terminal räumt sich selbst über das reguläre `pty-exit{reason:"exited"}`
      // aus dem echten Prozessende auf (TerminalHost.tsx) — kein manuelles `closed()` hier
      // nötig, sonst würde die Session doppelt entfernt/das Notice fehlen.
    } catch (err) {
      useToastStore.getState().push(describeApiError(err));
    }
  }

  return (
    <aside className="sidebar">
      {error && <p className="error-text sidebar-error">{error}</p>}

      <div className="sidebar-controls">
        <input
          type="search"
          className="sidebar-search"
          placeholder="Suchen…"
          aria-label="Sessions durchsuchen"
          value={query}
          onChange={(e) => useSessionStore.getState().queryChanged(e.target.value)}
        />
        <select
          className="sidebar-sort"
          aria-label="Sortierung"
          value={sortBy}
          onChange={(e) => useSessionStore.getState().sortChanged(e.target.value as SortKey)}
        >
          <option value="name">Name</option>
          <option value="lastActive">Zuletzt aktiv</option>
          <option value="created">Startzeit</option>
        </select>
      </div>

      {attached.length + notAttached.length + startableView.length === 0 && (
        <p className="sidebar-empty sidebar-empty-all">
          {query.trim()
            ? "Kein Treffer für die Suche."
            : "Noch nichts da — sobald Projekte gefunden werden, erscheinen sie hier."}
        </p>
      )}

      <SidebarGroup title="Angehängt" hidden={attached.length === 0}>
        <ul>
          {attached.map(([sessionId, s]) => (
            <li key={sessionId} className="session-row">
              <button
                type="button"
                className={sessionId === activeSessionId ? "session-item active" : "session-item"}
                onClick={() => handleActivate(sessionId)}
              >
                <StatusDot state={activityState(s.activity, now, s.lost)} />
                <span className="session-name">{s.name}</span>
                {s.activity.badge > 0 && <span className="badge">{s.activity.badge}</span>}
              </button>
              <button
                type="button"
                className="session-menu-trigger"
                aria-label={`Menü für ${s.name}`}
                onClick={() => handleToggleMenu(sessionId)}
              >
                ⋮
              </button>
              {menuOpenFor === sessionId && (
                <div className="session-menu">
                  <button type="button" onClick={() => handleToggleNotify(sessionId)}>
                    Benachrichtigungen {s.notifyEnabled ? "aus" : "ein"}
                  </button>
                  <button type="button" onClick={() => void handleDetach(sessionId)}>
                    Detach
                  </button>
                  <button type="button" onClick={() => void handleKill(s.name)}>
                    Kill
                  </button>
                </div>
              )}
            </li>
          ))}
        </ul>
      </SidebarGroup>

      <SidebarGroup title="Läuft" hidden={notAttached.length === 0}>
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

      <SidebarGroup title="Startbar" hidden={startableView.length === 0}>
        <ul>
          {startableView.map((p) => (
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

/**
 * Statusicon einer angehängten Session. Der „fertig"-Haken ist der eigentliche Nutzen: er zeigt
 * auf einen Blick, welche Session auf eine Antwort wartet, ohne dass man hineinschauen muss.
 *
 * Kein `aria-hidden` wie beim alten Punkt — der Zustand ist echte Information, keine Dekoration,
 * und wird deshalb auch vorgelesen.
 */
const STATUS_LABELS: Record<ActivityState, string> = {
  working: "arbeitet",
  waiting: "fertig — wartet auf Eingabe",
  idle: "bereit",
  lost: "Verbindung verloren",
};

const STATUS_GLYPHS: Record<ActivityState, string> = {
  working: "●",
  waiting: "✓",
  idle: "●",
  lost: "⚠",
};

function StatusDot({ state }: { state: ActivityState }) {
  return (
    <span className={`dot dot-${state}`} role="img" aria-label={STATUS_LABELS[state]}>
      {STATUS_GLYPHS[state]}
    </span>
  );
}

/**
 * Eine Gruppe verschwindet komplett, wenn sie leer ist — drei Überschriften mit „–" darunter
 * waren der Hauptgrund, warum die Sidebar voll aussah, ohne etwas zu zeigen. Ist *alles* leer,
 * übernimmt ein einzelner erklärender Leerzustand weiter oben.
 */
function SidebarGroup({
  title,
  hidden,
  children,
}: {
  title: string;
  hidden?: boolean;
  children: ReactNode;
}) {
  if (hidden) return null;
  return (
    <div className="sidebar-group">
      <h3>{title}</h3>
      {children}
    </div>
  );
}
