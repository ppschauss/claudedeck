/**
 * Pure Badge-/Notification-Logik pro Session — keine Timer, keine Seiteneffekte. Aufrufer
 * (Task 6: Channel-Output-Callback bzw. setTimeout-Poll) übergeben `now` explizit, damit die
 * Logik hier vollständig deterministisch bleibt und ohne Fake-Timer getestet werden kann.
 */
export interface Activity {
  /** Ungesehene Output-"Ereignisse" seit dem letzten Aktivieren der Session; 0 solange aktiv. */
  badge: number;
  /** Zeitstempel (ms, z.B. `Date.now()`) des letzten Outputs, oder `null` vor dem ersten Output. */
  lastOutputAt: number | null;
  /** Ob für den aktuellen "wartet auf Eingabe"-Zustand bereits eine Notification geschickt wurde. */
  notified: boolean;
}

/**
 * Reaktion auf einen eingetroffenen Output-Chunk. Aktive Session: Badge bleibt 0 (sie ist ja
 * sichtbar). Inaktive Session: Badge zählt hoch. In beiden Fällen: `lastOutputAt` wird auf
 * `now` gesetzt und `notified` zurückgesetzt — neuer Output bedeutet einen neuen
 * "wartet auf Eingabe"-Zyklus, für den ggf. wieder benachrichtigt werden darf.
 */
export function onOutput(a: Activity, now: number, isActive: boolean): Activity {
  return {
    badge: isActive ? 0 : a.badge + 1,
    lastOutputAt: now,
    notified: false,
  };
}

/**
 * Ob jetzt für eine Hintergrund-Session eine Notification geschickt werden soll: nur wenn
 * Notifications aktiviert sind, die Session NICHT gerade `lost` ist (Fix Minor, Review-Fund
 * Task 6 — eine Session, die auf Reconnect wartet, hat keinen laufenden Prozess, der "auf
 * Eingabe wartet"; eine Notification dafür wäre irreführend und der Nutzer könnte ohnehin nicht
 * antworten, solange die PTY noch nicht re-attacht ist), noch nicht benachrichtigt wurde, es
 * überhaupt schon Output gab und seit dem letzten Output mindestens `thresholdMs` vergangen sind
 * (Standard 2000ms — "wartet vermutlich auf Eingabe"). Ob eine Session aktiv ist, entscheidet
 * nicht diese Funktion, sondern der Aufrufer (Task 6: Notification-Timer läuft nur für
 * Hintergrund-Sessions).
 */
export function shouldNotify(
  a: Activity,
  now: number,
  enabled: boolean,
  lost: boolean,
  thresholdMs = 2000,
): boolean {
  if (!enabled || lost || a.notified || a.lastOutputAt === null) {
    return false;
  }
  return now - a.lastOutputAt >= thresholdMs;
}
