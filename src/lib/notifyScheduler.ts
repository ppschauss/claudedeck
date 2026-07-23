/**
 * Pure Entscheidungslogik (TDD, Task 6): für welche offenen Sessions muss (neu) ein
 * Notification-Timer gesetzt werden, und für welche muss ein bereits laufender Timer
 * gecancelt werden? Trennt die reine Entscheidung von den Seiteneffekten
 * (`setTimeout`/`clearTimeout`/`sendNotification`), die der Aufrufer
 * (`NotificationManager`-Komponente) ausführt — so bleibt die Entscheidung selbst ohne
 * Fake-Timer/React testbar.
 */
import type { OpenSession } from "../stores/sessionStore";

export interface ScheduleDecision {
  /** sessionIds, für die JETZT neu ein Timer gesetzt werden soll. */
  toSchedule: string[];
  /** sessionIds, für die ein laufender Timer JETZT gecancelt werden soll. */
  toCancel: string[];
}

/**
 * `activeSessionId`: die gerade sichtbare Session bekommt nie einen Timer (sie hat ohnehin
 * Badge 0 und braucht keine Notification). `scheduled`: die sessionIds, für die der Aufrufer
 * AKTUELL schon einen laufenden Timer hält.
 *
 * Eine Session ist "notification-würdig" (`eligible`), wenn sie im Hintergrund ist,
 * Notifications aktiviert hat, für den aktuellen Output-Zyklus noch nicht benachrichtigt wurde
 * und überhaupt schon Output hatte (`activity.lastOutputAt !== null` — dieselben
 * Vorbedingungen wie `badges.shouldNotify`, hier aber nur für die Timer-SET/CANCEL-Entscheidung,
 * nicht für den tatsächlichen Schwellenwert-Vergleich, der beim Timer-Feuern separat mit
 * `shouldNotify` geprüft wird).
 *
 * - `eligible && !isScheduled` → `toSchedule` (neuer Timer nötig).
 * - `!eligible && isScheduled` → `toCancel` (Session wurde aktiv, notified, oder verlor
 *   `notifyEnabled` — ein laufender Timer wäre jetzt fehl am Platz).
 * - Jede `scheduled`-sessionId, die es in `openSessions` gar nicht mehr gibt (Session
 *   geschlossen/detached), wird ebenfalls zu `toCancel` hinzugefügt (Leak-Schutz).
 */
export function decideSchedule(
  openSessions: Map<string, OpenSession>,
  activeSessionId: string | null,
  scheduled: ReadonlySet<string>,
): ScheduleDecision {
  const toSchedule: string[] = [];
  const toCancel: string[] = [];

  for (const [id, s] of openSessions) {
    const eligible =
      id !== activeSessionId &&
      s.notifyEnabled &&
      !s.activity.notified &&
      s.activity.lastOutputAt !== null;
    const isScheduled = scheduled.has(id);
    if (eligible && !isScheduled) toSchedule.push(id);
    if (!eligible && isScheduled) toCancel.push(id);
  }

  for (const id of scheduled) {
    if (!openSessions.has(id)) toCancel.push(id);
  }

  return { toSchedule, toCancel };
}
