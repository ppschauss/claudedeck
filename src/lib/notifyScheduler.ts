/**
 * Pure Entscheidungslogik (TDD, Task 6): für welche offenen Sessions muss (neu) ein
 * Notification-Timer gesetzt werden, und für welche muss ein bereits laufender Timer
 * gecancelt werden? Trennt die reine Entscheidung von den Seiteneffekten
 * (`setTimeout`/`clearTimeout`/`sendNotification`), die der Aufrufer
 * (`NotificationManager`-Komponente) ausführt — so bleibt die Entscheidung selbst ohne
 * Fake-Timer/React testbar.
 */
import { shouldNotify } from "./badges";
import type { OpenSession } from "../stores/sessionStore";

/**
 * Ob eine Session gerade "notification-würdig" ist (siehe `decideSchedule`-Doku unten für die
 * Kriterien) — als eigene Funktion, damit `decideSchedule` (Timer setzen/cancel) und
 * `decideFire` (Timer-Feuern erneut prüfen, Fix I-1) dieselbe Eligibility-Logik teilen statt sie
 * zweimal zu pflegen.
 */
function isEligible(
  s: Pick<OpenSession, "notifyEnabled" | "lost" | "activity">,
  id: string,
  activeSessionId: string | null,
): boolean {
  return (
    id !== activeSessionId &&
    s.notifyEnabled &&
    !s.lost &&
    !s.activity.notified &&
    s.activity.lastOutputAt !== null
  );
}

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
 * Notifications aktiviert hat, NICHT `lost` ist (Fix Minor, Review-Fund Task 6 — eine Session,
 * die auf Reconnect wartet, wartet nicht "auf Eingabe"; ein Timer dafür wäre sowieso witzlos,
 * weil `shouldNotify` beim Feuern ohnehin ablehnen würde, aber gar nicht erst einen laufenden
 * Timer zu halten ist sauberer als einen zu planen, der garantiert nichts tut), für den
 * aktuellen Output-Zyklus noch nicht benachrichtigt wurde und überhaupt schon Output hatte
 * (`activity.lastOutputAt !== null` — dieselben Vorbedingungen wie `badges.shouldNotify`, hier
 * aber nur für die Timer-SET/CANCEL-Entscheidung, nicht für den tatsächlichen
 * Schwellenwert-Vergleich, der beim Timer-Feuern separat mit `shouldNotify` geprüft wird).
 *
 * - `eligible && !isScheduled` → `toSchedule` (neuer Timer nötig).
 * - `!eligible && isScheduled` → `toCancel` (Session wurde aktiv, notified, verlor
 *   `notifyEnabled`, oder wurde `lost` — ein laufender Timer wäre jetzt fehl am Platz).
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
    const eligible = isEligible(s, id, activeSessionId);
    const isScheduled = scheduled.has(id);
    if (eligible && !isScheduled) toSchedule.push(id);
    if (!eligible && isScheduled) toCancel.push(id);
  }

  for (const id of scheduled) {
    if (!openSessions.has(id)) toCancel.push(id);
  }

  return { toSchedule, toCancel };
}

/** Ergebnis von `decideFire`: was der Aufrufer beim Timer-Feuern tun soll. */
export type FireDecision =
  /** Schwellenwert erreicht, Session weiter eligible → jetzt benachrichtigen. */
  | { action: "notify" }
  /** Schwellenwert (noch) nicht erreicht (neuerer Output kam rein, seit der Timer geplant
   * wurde), Session ist aber weiter eligible → Timer NEU planen, relativ zum tatsächlich
   * letzten Output (Fix I-1: Review-Fund Task 7 — sonst geht die Notification verloren, wenn
   * der letzte Chunk eines Output-Bursts <2s vor dem ursprünglich geplanten Feuern eintraf). */
  | { action: "reschedule"; delayMs: number }
  /** Session nicht (mehr) eligible (aktiv geworden, notified, notifyEnabled aus, lost, oder gar
   * nicht mehr offen) → nichts tun, kein neuer Timer. */
  | { action: "cancel" };

/**
 * Pure Entscheidung für den Moment, in dem ein Notification-Timer feuert (Fix I-1, Task 7).
 * Ersetzt das bisherige "shouldNotify ablehnt → Timer verschwindet ersatzlos"-Verhalten: statt
 * nur ja/nein zu prüfen, wird bei einer Ablehnung zusätzlich geprüft, ob die Session weiter
 * eligible ist (also potenziell noch benachrichtigt werden muss) und — falls ja — die
 * verbleibende Zeit bis `lastOutputAt + thresholdMs` als neuer Timer-Delay zurückgegeben.
 *
 * `entry` fehlt (Session inzwischen geschlossen/detached) → `cancel`.
 */
export function decideFire(
  entry: Pick<OpenSession, "notifyEnabled" | "lost" | "activity"> | undefined,
  activeSessionId: string | null,
  sessionId: string,
  now: number,
  thresholdMs = 2000,
): FireDecision {
  if (!entry) return { action: "cancel" };

  if (shouldNotify(entry.activity, now, entry.notifyEnabled, entry.lost, thresholdMs)) {
    return { action: "notify" };
  }

  if (isEligible(entry, sessionId, activeSessionId)) {
    const lastOutputAt = entry.activity.lastOutputAt as number;
    return { action: "reschedule", delayMs: Math.max(0, lastOutputAt + thresholdMs - now) };
  }

  return { action: "cancel" };
}
