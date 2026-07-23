/**
 * Windows-Notifications für Hintergrund-Sessions (Task 6). Rendert nichts sichtbares — reiner
 * Seiteneffekt-Controller: fragt die OS-Berechtigung einmalig an, reconciled bei jeder
 * `sessionStore`-Änderung über die reine `decideSchedule`-Entscheidung (`notifyScheduler.ts`),
 * welche Timer neu gesetzt/gecancelt werden müssen, und prüft beim Timer-Feuern erneut per
 * `decideFire` (`notifyScheduler.ts`), ob tatsächlich noch benachrichtigt werden soll (Session
 * könnte inzwischen aktiv geworden sein, ohne dass der Timer schon reagiert hat) — lehnt
 * `decideFire` ab, weil seit dem Planen des Timers neuer Output kam (Schwellenwert also von
 * `lastOutputAt` aus noch nicht erreicht), plant es den Timer relativ zu `lastOutputAt` NEU statt
 * ihn ersatzlos verschwinden zu lassen (Fix I-1, Review-Fund Task 7 — sonst geht die
 * Notification verloren, wenn der letzte Chunk eines Output-Bursts <2s vor dem ursprünglich
 * geplanten Feuern eintraf).
 */
import { useEffect, useRef } from "react";
import { isPermissionGranted, requestPermission, sendNotification } from "@tauri-apps/plugin-notification";
import { decideFire, decideSchedule } from "../lib/notifyScheduler";
import { useSessionStore } from "../stores/sessionStore";

/** Gleicher Schwellenwert wie `badges.shouldNotify`s Default — explizit statt implizit, damit
 * Scheduling-Timer (hier) und Schwellenwert-Check (badges.ts) sichtbar dieselbe Zahl teilen. */
const THRESHOLD_MS = 2000;

export function NotificationManager() {
  const timersRef = useRef<Map<string, ReturnType<typeof setTimeout>>>(new Map());
  const permittedRef = useRef(false);

  useEffect(() => {
    let cancelled = false;
    void (async () => {
      let granted = await isPermissionGranted().catch(() => false);
      if (!granted) {
        const permission = await requestPermission().catch(() => "denied");
        granted = permission === "granted";
      }
      if (!cancelled) permittedRef.current = granted;
    })();
    return () => {
      cancelled = true;
    };
  }, []);

  useEffect(() => {
    const timers = timersRef.current;

    function fire(sessionId: string) {
      timers.delete(sessionId);
      const state = useSessionStore.getState();
      const entry = state.openSessions.get(sessionId);
      const decision = decideFire(entry, state.activeSessionId, sessionId, Date.now(), THRESHOLD_MS);
      switch (decision.action) {
        case "notify":
          if (!entry || !permittedRef.current) return;
          state.notifiedSent(sessionId);
          void sendNotification({ title: entry.name, body: "wartet auf Eingabe" });
          return;
        case "reschedule":
          timers.set(
            sessionId,
            setTimeout(() => fire(sessionId), decision.delayMs),
          );
          return;
        case "cancel":
          return;
      }
    }

    function reconcile() {
      const state = useSessionStore.getState();
      const { toSchedule, toCancel } = decideSchedule(
        state.openSessions,
        state.activeSessionId,
        new Set(timers.keys()),
      );

      for (const id of toCancel) {
        const timer = timers.get(id);
        if (timer !== undefined) clearTimeout(timer);
        timers.delete(id);
      }
      for (const id of toSchedule) {
        timers.set(
          id,
          setTimeout(() => fire(id), THRESHOLD_MS),
        );
      }
    }

    reconcile();
    const unsubscribe = useSessionStore.subscribe(reconcile);
    return () => {
      unsubscribe();
      for (const timer of timers.values()) clearTimeout(timer);
      timers.clear();
    };
  }, []);

  return null;
}
