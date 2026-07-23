/**
 * Reine Auswahl-Logik: welche Session soll aktiv werden, wenn die gerade aktive Session
 * geschlossen wird? `sessionStore.closed()` (Task 4) setzt `activeSessionId` selbst nur auf
 * `null` und überlässt den Wechsel zur nächsten offenen Session bewusst der UI (Review-Fund
 * M4-Task-4) — dieser Helfer ist die dafür verwendete Entscheidung, als pure Funktion getrennt
 * von React/Zustand testbar.
 */

/**
 * Wählt die Session, die nach dem Entfernen von `closingId` aus `orderedIds` aktiv werden soll.
 * `orderedIds` ist der Snapshot der offenen Session-IDs VOR dem Entfernen (z.B.
 * `Array.from(openSessions.keys())`, das die Map-Insertion-Reihenfolge widerspiegelt).
 *
 * Bevorzugt die Session, die nach dem Entfernen an die Stelle von `closingId` nachrückt (die
 * "nächst-jüngere"); war `closingId` die letzte in der Liste, wird stattdessen die davor
 * gewählt. Liefert `null`, wenn danach keine Session mehr übrig ist. War `closingId` gar nicht
 * in `orderedIds` enthalten, wird defensiv die erste verbleibende Session gewählt (statt `null`
 * zurückzugeben) — sicherer Fallback, kein no-op.
 */
export function nextActiveSessionId(orderedIds: string[], closingId: string): string | null {
  const remaining = orderedIds.filter((id) => id !== closingId);
  if (remaining.length === 0) return null;

  const idx = orderedIds.indexOf(closingId);
  if (idx === -1) return remaining[0];

  return idx < remaining.length ? remaining[idx] : remaining[remaining.length - 1];
}
