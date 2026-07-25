/**
 * Abbildung zwischen der Reglerposition und Claude Codes Effort-Stufen — pure Funktionen, damit
 * der Regler ohne DOM testbar bleibt.
 *
 * Stufen und Reihenfolge sind aus `claude --help` (2.1.220) übernommen:
 * `--effort <level>  Effort level for the current session (low, medium, high, xhigh, max)`.
 */

export const EFFORT_LEVELS = ["low", "medium", "high", "xhigh", "max"] as const;

export type EffortLevel = (typeof EFFORT_LEVELS)[number];

/** Vorgabe, wenn nichts konfiguriert ist — entspricht der dokumentierten API-Vorgabe. */
const DEFAULT_INDEX = EFFORT_LEVELS.indexOf("high");

/**
 * Reglerposition → Stufe. Werte außerhalb des Bereichs werden auf die Randstufen begrenzt, statt
 * `undefined` zu liefern: ein `<input type="range">` kann per Tastatur oder fremdem Wert
 * durchaus daneben landen.
 */
export function effortFromIndex(index: number): EffortLevel {
  const clamped = Math.min(Math.max(Math.floor(index), 0), EFFORT_LEVELS.length - 1);
  return EFFORT_LEVELS[clamped];
}

/**
 * Stufe → Reglerposition. Unbekannte, leere oder fehlende Werte ergeben die Position von `high`,
 * damit der Regler auch bei leerer Config eine sinnvolle Stellung hat.
 */
export function indexOfEffort(level: string | null | undefined): number {
  if (!level) return DEFAULT_INDEX;
  const index = EFFORT_LEVELS.indexOf(level as EffortLevel);
  return index === -1 ? DEFAULT_INDEX : index;
}
