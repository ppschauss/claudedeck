/**
 * Filter- und Sortierlogik für die Session-Sidebar — pure Funktionen (Hausstil wie `badges.ts`),
 * damit die Reihenfolge ohne React testbar bleibt. `Sidebar.tsx` ruft sie nur auf.
 *
 * `matchesQuery` ist absichtlich generisch über einen „Heuhaufen"-String gehalten, damit das
 * Befehls-Panel dieselbe Suche über Name *und* Beschreibung verwenden kann (`catalogFilter.ts`).
 */

export type SortKey = "name" | "created" | "lastActive";

/**
 * Was zum Sortieren eines Eintrags gebraucht wird. Beide Zeitstempel sind optional, weil die
 * Sidebar drei ungleich informierte Quellen mischt:
 * - angehängte Sessions kennen `lastOutputAt` (aus `badges.ts`) **und** `createdAt`,
 * - laufende, nicht angehängte Sessions kennen nur `createdAt` (`SessionInfo.created`),
 * - startbare Projekte (`Project` aus den `scan_paths`) kennen keinen von beiden.
 */
export interface SortMeta {
  name: string;
  createdAt: number | null;
  lastOutputAt: number | null;
}

/**
 * Case- und diakritika-insensitiver Teilstring-Test. Leere oder nur aus Weißraum bestehende
 * Query matcht alles — ein leeres Suchfeld darf die Liste nicht leeren.
 */
export function matchesQuery(haystack: string, query: string): boolean {
  const needle = query.trim().toLowerCase();
  if (needle === "") return true;
  return haystack.toLowerCase().includes(needle);
}

/**
 * `sensitivity: "base"` sortiert „Beta" neben „beta" und „änderung" neben „anderung" — für eine
 * deutschsprachige Sessionliste die erwartete Reihenfolge, anders als bei einem reinen
 * Codepoint-Vergleich, der alle Großbuchstaben vorziehen würde.
 */
function compareNames(a: string, b: string): number {
  return a.localeCompare(b, "de", { sensitivity: "base" });
}

/**
 * Der für `key` maßgebliche Zeitstempel. Bei `lastActive` fällt ein Eintrag ohne bekannten Output
 * auf seine Startzeit zurück — das ist eine *Näherung*, kein echter „letzter Zugriff", weil nur
 * angehängte Sessions überhaupt Output melden. Die Sortierung heißt in der UI deshalb
 * „zuletzt aktiv" und nicht „letzter Zugriff".
 */
function timeOf(meta: SortMeta, key: SortKey): number | null {
  return key === "created" ? meta.createdAt : (meta.lastOutputAt ?? meta.createdAt);
}

/**
 * Sortiert eine Kopie von `items`; die Eingabeliste bleibt unverändert (die Aufrufer halten
 * Store-Arrays, die nicht in place mutiert werden dürfen).
 *
 * Zeitsortierungen laufen absteigend (neueste zuerst). Einträge ohne den nötigen Zeitstempel
 * landen hinten statt zu verschwinden, und bei Gleichstand entscheidet immer der Name — damit ist
 * die Reihenfolge deterministisch und nicht von der Eingabereihenfolge abhängig.
 */
export function sortByKey<T>(
  items: readonly T[],
  key: SortKey,
  meta: (item: T) => SortMeta,
): T[] {
  const decorated = items.map((item) => ({ item, meta: meta(item) }));

  decorated.sort((a, b) => {
    if (key !== "name") {
      const ta = timeOf(a.meta, key);
      const tb = timeOf(b.meta, key);
      if (ta !== tb) {
        if (ta === null) return 1;
        if (tb === null) return -1;
        return tb - ta;
      }
    }
    return compareNames(a.meta.name, b.meta.name);
  });

  return decorated.map((entry) => entry.item);
}
