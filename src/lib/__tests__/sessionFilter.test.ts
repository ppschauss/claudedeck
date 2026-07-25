import { describe, expect, it } from "vitest";
import { matchesQuery, sortByKey, type SortMeta } from "../sessionFilter";

describe("matchesQuery", () => {
  it("matcht alles bei leerer Query", () => {
    expect(matchesQuery("cc-claudedeck", "")).toBe(true);
  });

  it("matcht alles bei Query aus reinem Weißraum", () => {
    expect(matchesQuery("cc-claudedeck", "   ")).toBe(true);
  });

  it("matcht Teilstrings unabhängig von Groß-/Kleinschreibung", () => {
    expect(matchesQuery("cc-ClaudeDeck", "claude")).toBe(true);
    expect(matchesQuery("cc-claudedeck", "DECK")).toBe(true);
  });

  it("ignoriert Weißraum am Rand der Query", () => {
    expect(matchesQuery("cc-claudedeck", "  deck  ")).toBe(true);
  });

  it("liefert false, wenn der Teilstring fehlt", () => {
    expect(matchesQuery("cc-claudedeck", "wordpress")).toBe(false);
  });

  it("matcht Umlaute unabhängig von der Schreibung", () => {
    expect(matchesQuery("cc-löffelholz", "LÖFFEL")).toBe(true);
  });
});

/** Kurzschreibweise: Name plus optionale Zeitstempel. */
function meta(name: string, createdAt: number | null = null, lastOutputAt: number | null = null) {
  return { name, createdAt, lastOutputAt };
}
const identity = (m: SortMeta) => m;

describe("sortByKey", () => {
  it("sortiert nach Namen aufsteigend", () => {
    const items = [meta("charlie"), meta("alpha"), meta("bravo")];
    expect(sortByKey(items, "name", identity).map((m) => m.name)).toEqual([
      "alpha",
      "bravo",
      "charlie",
    ]);
  });

  it("sortiert Namen unabhängig von Groß-/Kleinschreibung", () => {
    const items = [meta("Beta"), meta("alpha"), meta("Gamma")];
    expect(sortByKey(items, "name", identity).map((m) => m.name)).toEqual([
      "alpha",
      "Beta",
      "Gamma",
    ]);
  });

  it("ordnet Umlaute beim Namen neben den Grundbuchstaben ein", () => {
    const items = [meta("zulu"), meta("änderung"), meta("beta")];
    expect(sortByKey(items, "name", identity).map((m) => m.name)).toEqual([
      "änderung",
      "beta",
      "zulu",
    ]);
  });

  it("sortiert nach Startzeit, neueste zuerst", () => {
    const items = [meta("alt", 100), meta("neu", 300), meta("mittel", 200)];
    expect(sortByKey(items, "created", identity).map((m) => m.name)).toEqual([
      "neu",
      "mittel",
      "alt",
    ]);
  });

  // Projekte aus scan_paths haben keinen Zeitstempel — sie dürfen nicht verschwinden oder
  // zufällig zwischen den Sessions landen.
  it("stellt Einträge ohne Zeitstempel hinten an und sortiert sie nach Namen", () => {
    const items = [meta("ohne-b"), meta("mit", 100), meta("ohne-a")];
    expect(sortByKey(items, "created", identity).map((m) => m.name)).toEqual([
      "mit",
      "ohne-a",
      "ohne-b",
    ]);
  });

  it("sortiert bei gleicher Startzeit nach Namen", () => {
    const items = [meta("bravo", 100), meta("alpha", 100)];
    expect(sortByKey(items, "created", identity).map((m) => m.name)).toEqual(["alpha", "bravo"]);
  });

  it("sortiert nach letzter Aktivität, neueste zuerst", () => {
    const items = [meta("a", 1, 100), meta("b", 1, 300), meta("c", 1, 200)];
    expect(sortByKey(items, "lastActive", identity).map((m) => m.name)).toEqual(["b", "c", "a"]);
  });

  // Nur angehängte Sessions kennen ein echtes lastOutputAt; für die übrigen ist die Startzeit
  // die beste verfügbare Näherung.
  it("nutzt die Startzeit, wenn keine letzte Aktivität bekannt ist", () => {
    const items = [meta("nur-start", 500), meta("mit-output", 100, 400)];
    expect(sortByKey(items, "lastActive", identity).map((m) => m.name)).toEqual([
      "nur-start",
      "mit-output",
    ]);
  });

  it("wendet die Meta-Funktion auf beliebige Objekte an", () => {
    const sessions = [
      { id: "1", label: "zulu" },
      { id: "2", label: "alpha" },
    ];
    const sorted = sortByKey(sessions, "name", (s) => meta(s.label));
    expect(sorted.map((s) => s.id)).toEqual(["2", "1"]);
  });

  it("lässt die Eingabeliste unverändert", () => {
    const items = [meta("charlie"), meta("alpha")];
    sortByKey(items, "name", identity);
    expect(items.map((m) => m.name)).toEqual(["charlie", "alpha"]);
  });

  it("kommt mit leerer Liste klar", () => {
    expect(sortByKey([], "name", identity)).toEqual([]);
  });
});
