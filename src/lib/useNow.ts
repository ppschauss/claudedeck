/**
 * Liefert einen Zeitstempel, der sich selbst aktualisiert.
 *
 * Gebraucht für Anzeigen, die sich durch reines *Verstreichen von Zeit* ändern — etwa das
 * Statusicon einer Session, das nach ein paar Sekunden Stille von „arbeitet" auf „fertig"
 * springt. Ohne diesen Takt bliebe es bei „arbeitet" stehen, weil kein Store-Update mehr kommt:
 * das letzte Ereignis war ja gerade der ausbleibende Output.
 *
 * `enabled` hält den Timer aus, wenn es nichts zu takten gibt (keine offene Session) — eine
 * Desktop-App soll nicht im Sekundentakt neu rendern, während sie nur herumsteht.
 */
import { useEffect, useState } from "react";

export function useNow(intervalMs: number, enabled: boolean): number {
  const [now, setNow] = useState(() => Date.now());

  useEffect(() => {
    if (!enabled) return;
    // Sofort einmal setzen: nach einer Pause (Timer war aus) wäre der gemerkte Wert veraltet
    // und die Anzeige für bis zu `intervalMs` falsch.
    setNow(Date.now());
    const id = setInterval(() => setNow(Date.now()), intervalMs);
    return () => clearInterval(id);
  }, [intervalMs, enabled]);

  return now;
}
