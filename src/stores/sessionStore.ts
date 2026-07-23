/**
 * Zustand-Store für Sessionliste + offene Terminals. Die Actions sind bewusst als pure
 * Zustandsübergänge geschrieben (kein IPC hier drin — das übernehmen die Aufrufer in
 * `src/components/*`, Task 5) und darüber direkt testbar: `useSessionStore.getState().opened(…)`
 * usw., ganz ohne React.
 */
import { create } from "zustand";
import type { Activity } from "../lib/badges";
import { onOutput } from "../lib/badges";
import type { Project, SessionInfo } from "../lib/ipc";

export interface OpenSession {
  name: string;
  activity: Activity;
  notifyEnabled: boolean;
}

export interface SessionState {
  /** Bereits laufende (tmux-)Sessions, wie zuletzt von `list_sessions` gemeldet. */
  running: SessionInfo[];
  /** Noch nicht angehängte Projektverzeichnisse aus den `scan_paths`. */
  startable: Project[];
  /** Sessions, für die im TermPool bereits ein xterm-Terminal existiert — nie beim
   * Umschalten disposed, nur hier aus der Liste entfernt (`closed`). */
  openSessions: Map<string, OpenSession>;
  /** Welche `openSessions`-Session gerade sichtbar ist (`null` = keine). */
  activeSessionId: string | null;

  /** Übernimmt das Ergebnis von `list_sessions` unverändert. */
  sessionsLoaded: (list: { running: SessionInfo[]; startable: Project[] }) => void;
  /** Neue Session im TermPool angekommen (frisch geöffnet oder gestartet) — wird sofort aktiv,
   * mit frischer (Badge-0-)Activity. */
  opened: (sessionId: string, name: string) => void;
  /** Nutzer wechselt zu einer bereits offenen Session — setzt sie aktiv und ihren Badge auf 0. */
  activated: (sessionId: string) => void;
  /** Ein Output-Chunk ist für `sessionId` eingetroffen; `now` i.d.R. `Date.now()`. Badge zählt
   * nur hoch, wenn die Session gerade nicht aktiv ist. No-Op für unbekannte sessionId. */
  outputReceived: (sessionId: string, now: number) => void;
  /** Session aus `openSessions` entfernen (Detach/Exit). War sie aktiv, wird `activeSessionId`
   * auf `null` gesetzt — welche Session danach angezeigt wird, entscheidet die UI (Task 5). */
  closed: (sessionId: string) => void;
  /** Kehrt `notifyEnabled` für die Session um (Kontextmenü "Benachrichtigungen aus"). */
  notifyToggled: (sessionId: string) => void;
}

const freshActivity = (): Activity => ({ badge: 0, lastOutputAt: null, notified: false });

export const useSessionStore = create<SessionState>((set) => ({
  running: [],
  startable: [],
  openSessions: new Map(),
  activeSessionId: null,

  sessionsLoaded: (list) =>
    set({ running: list.running, startable: list.startable }),

  opened: (sessionId, name) =>
    set((state) => {
      const openSessions = new Map(state.openSessions);
      openSessions.set(sessionId, { name, activity: freshActivity(), notifyEnabled: true });
      return { openSessions, activeSessionId: sessionId };
    }),

  activated: (sessionId) =>
    set((state) => {
      const entry = state.openSessions.get(sessionId);
      if (!entry) return { activeSessionId: sessionId };
      const openSessions = new Map(state.openSessions);
      openSessions.set(sessionId, { ...entry, activity: { ...entry.activity, badge: 0 } });
      return { openSessions, activeSessionId: sessionId };
    }),

  outputReceived: (sessionId, now) =>
    set((state) => {
      const entry = state.openSessions.get(sessionId);
      if (!entry) return {};
      const isActive = state.activeSessionId === sessionId;
      const openSessions = new Map(state.openSessions);
      openSessions.set(sessionId, { ...entry, activity: onOutput(entry.activity, now, isActive) });
      return { openSessions };
    }),

  closed: (sessionId) =>
    set((state) => {
      if (!state.openSessions.has(sessionId)) return {};
      const openSessions = new Map(state.openSessions);
      openSessions.delete(sessionId);
      const activeSessionId = state.activeSessionId === sessionId ? null : state.activeSessionId;
      return { openSessions, activeSessionId };
    }),

  notifyToggled: (sessionId) =>
    set((state) => {
      const entry = state.openSessions.get(sessionId);
      if (!entry) return {};
      const openSessions = new Map(state.openSessions);
      openSessions.set(sessionId, { ...entry, notifyEnabled: !entry.notifyEnabled });
      return { openSessions };
    }),
}));
