/**
 * Misst, ob xterms Maus-zu-Zelle-Rechnung mit dem übereinstimmt, was der Benutzer sieht.
 *
 * Hintergrund: Die Meldung „markiert ein paar Zeilen unterhalb des Zeigers" ließ sich hier in
 * keiner Umgebung nachstellen — nicht in Gecko, nicht in Chromium, nicht bei 125/150/175/200 %
 * Anzeigeskalierung, nicht mit der echten App-CSS. Die statische Geometrie (Zellhöhe,
 * Zeilenabstand, Versatz zwischen `.xterm-screen` und `.xterm-rows`) war überall exakt.
 *
 * Deshalb misst diese Stelle nicht mehr die Geometrie, sondern den Fehler selbst: die Zeile,
 * die tatsächlich unter dem Zeiger lag, gegen die Zeile, die xterm daraufhin markiert hat.
 * Ein Wert ungleich 0 ist der gesuchte Fehler, in Zeilen.
 */

export interface AuswahlMessung {
  /** Erste im Terminal sichtbare Pufferzeile — ohne sie wären die Zahlen bei Scrollback unvergleichbar. */
  viewportY: number;
  /** Index der gerenderten Zeile unter dem Zeiger, 0 = oberste sichtbare. `null`, wenn danebengeklickt. */
  zeileUnterZeiger: number | null;
  /** Startzeile der Auswahl in Pufferkoordinaten, wie xterm sie meldet. `null` ohne Auswahl. */
  auswahlStartY: number | null;
}

/**
 * Fehler in Zeilen: positiv heißt „xterm markiert weiter unten als gezeigt", negativ „weiter
 * oben". `null`, wenn eine der beiden Seiten fehlt und ein Vergleich nichts aussagen würde.
 */
export function auswahlVersatz(m: AuswahlMessung): number | null {
  if (m.zeileUnterZeiger === null || m.auswahlStartY === null) return null;
  return m.auswahlStartY - (m.viewportY + m.zeileUnterZeiger);
}
