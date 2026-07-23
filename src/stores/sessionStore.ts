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
  /** true = pty-exit kam mit reason "connectionLost" (Review-Fund M4-Task-5, Fix 2): Terminal
   * bleibt im TermPool erhalten, Session bleibt in `openSessions` — Task 6 re-attacht mit
   * derselben sessionId ins selbe Terminal, statt (wie bei "exited") disposed/entfernt zu
   * werden. */
  lost: boolean;
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
  /** pty-exit mit reason "connectionLost" (Fix 2): markiert die Session als `lost`, OHNE sie
   * aus `openSessions` zu entfernen — Terminal/Store-Eintrag bleiben für den Re-Attach (Task 6)
   * intakt. `activated()` funktioniert unverändert auch auf einer lost-Session. No-Op für
   * unbekannte sessionId. */
  markLost: (sessionId: string) => void;
  /** Task 6, Auflage C: Gegenstück zu `markLost` — Re-Attach nach erfolgreichem Reconnect
   * (`session-reattached`-Event) setzt `lost` zurück auf `false`. Sidebar zeigt dadurch wieder
   * `●` statt `⚠`, ohne dass `Sidebar.tsx` selbst etwas über Reconnect wissen muss. No-Op für
   * unbekannte sessionId. Setzt KEINE Activity/Badge zurück (das macht bei Bedarf weiterhin
   * `activated()`) — ein Re-Attach allein macht eine Hintergrund-Session nicht aktiv. */
  reattached: (sessionId: string) => void;
  /** Session aus `openSessions` entfernen (Detach/Exit). War sie aktiv, wird `activeSessionId`
   * auf `null` gesetzt — welche Session danach angezeigt wird, entscheidet die UI (Task 5). */
  closed: (sessionId: string) => void;
  /** Kehrt `notifyEnabled` für die Session um (Kontextmenü "Benachrichtigungen aus"). */
  notifyToggled: (sessionId: string) => void;
  /** Task 6: markiert die Session als "für den aktuellen Wartezyklus bereits benachrichtigt"
   * (`NotificationManager` nach einem gesendeten `sendNotification`). Verhindert eine zweite
   * Notification für denselben Zyklus, bis neuer Output (`onOutput` in `badges.ts`) `notified`
   * wieder zurücksetzt. No-Op für unbekannte sessionId. */
  notifiedSent: (sessionId: string) => void;
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
      openSessions.set(sessionId, { name, activity: freshActivity(), notifyEnabled: true, lost: false });
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

  markLost: (sessionId) =>
    set((state) => {
      const entry = state.openSessions.get(sessionId);
      if (!entry) return {};
      const openSessions = new Map(state.openSessions);
      openSessions.set(sessionId, { ...entry, lost: true });
      return { openSessions };
    }),

  reattached: (sessionId) =>
    set((state) => {
      const entry = state.openSessions.get(sessionId);
      if (!entry) return {};
      const openSessions = new Map(state.openSessions);
      openSessions.set(sessionId, { ...entry, lost: false });
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

  notifiedSent: (sessionId) =>
    set((state) => {
      const entry = state.openSessions.get(sessionId);
      if (!entry) return {};
      const openSessions = new Map(state.openSessions);
      openSessions.set(sessionId, { ...entry, activity: { ...entry.activity, notified: true } });
      return { openSessions };
    }),
}));
