/**
 * Reine Such-Logik für den Doppel-Attach-Guard (Review-Fund M4-Task-5, Fix 4): bevor
 * `Sidebar.tsx` für einen Sessionnamen `open_session` ruft, prüft sie erst, ob im Store
 * bereits eine offene Session mit genau diesem Namen existiert — dann reicht `activated()`
 * auf die vorhandene sessionId, statt ein zweites Mal anzuhängen.
 */
import type { OpenSession } from "../stores/sessionStore";

/** Liefert die sessionId der ersten `openSessions`-Session mit Namen `name`, oder `null`. */
export function findOpenByName(map: Map<string, OpenSession>, name: string): string | null {
  for (const [sessionId, entry] of map) {
    if (entry.name === name) return sessionId;
  }
  return null;
}
