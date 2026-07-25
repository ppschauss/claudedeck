import { describe, expect, it } from "vitest";
import { EFFORT_LEVELS, effortFromIndex, indexOfEffort } from "../effort";

describe("EFFORT_LEVELS", () => {
  // Reihenfolge und Werte stammen aus `claude --help` (2.1.220): --effort <low|medium|high|xhigh|max>
  it("bildet die fünf Stufen der CLI in aufsteigender Reihenfolge ab", () => {
    expect(EFFORT_LEVELS).toEqual(["low", "medium", "high", "xhigh", "max"]);
  });
});

describe("effortFromIndex", () => {
  it("bildet jede Reglerposition auf ihre Stufe ab", () => {
    expect(effortFromIndex(0)).toBe("low");
    expect(effortFromIndex(2)).toBe("high");
    expect(effortFromIndex(4)).toBe("max");
  });

  // Ein <input type="range"> kann durch Tastatur oder fremde Werte außerhalb landen.
  it("begrenzt Werte außerhalb des Bereichs auf die Randstufen", () => {
    expect(effortFromIndex(-3)).toBe("low");
    expect(effortFromIndex(99)).toBe("max");
  });

  it("rundet Nachkommastellen ab", () => {
    expect(effortFromIndex(1.9)).toBe("medium");
  });
});

describe("indexOfEffort", () => {
  it("findet die Position einer bekannten Stufe", () => {
    expect(indexOfEffort("low")).toBe(0);
    expect(indexOfEffort("xhigh")).toBe(3);
  });

  // Kein Effort in der Config = Claude Codes eigene Vorgabe; der Regler muss trotzdem
  // irgendwo stehen. "high" ist die dokumentierte API-Vorgabe.
  it("fällt bei unbekanntem oder fehlendem Wert auf high zurück", () => {
    expect(indexOfEffort(null)).toBe(2);
    expect(indexOfEffort(undefined)).toBe(2);
    expect(indexOfEffort("turbo")).toBe(2);
    expect(indexOfEffort("")).toBe(2);
  });

  it("ist zu effortFromIndex invers", () => {
    for (const level of EFFORT_LEVELS) {
      expect(effortFromIndex(indexOfEffort(level))).toBe(level);
    }
  });
});
