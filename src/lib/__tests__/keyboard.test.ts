import { describe, expect, it } from "vitest";
import { altGraphChar, type KeyEventLike } from "../keyboard";

/** Baut ein minimales Event; `altGraph` steuert nur, was `getModifierState("AltGraph")` liefert. */
function ev(
  key: string,
  { ctrl = false, alt = false, meta = false, altGraph = false } = {},
): KeyEventLike {
  return {
    key,
    ctrlKey: ctrl,
    altKey: alt,
    metaKey: meta,
    getModifierState: (name: string) => name === "AltGraph" && altGraph,
  };
}

describe("altGraphChar", () => {
  it("liefert das Zeichen, wenn der AltGraph-Modifier gesetzt ist", () => {
    expect(altGraphChar(ev("@", { altGraph: true }))).toBe("@");
  });

  // Windows meldet AltGr als Strg+Alt; ältere Chromium-Builds setzen getModifierState("AltGraph")
  // dabei nicht — dieser Fallback ist der eigentliche Grund für die eigene Funktion.
  it("erkennt AltGr auch am Strg+Alt-Fallback ohne AltGraph-Modifier", () => {
    expect(altGraphChar(ev("{", { ctrl: true, alt: true }))).toBe("{");
  });

  it("liefert auch Mehrbyte-Zeichen wie €", () => {
    expect(altGraphChar(ev("€", { ctrl: true, alt: true }))).toBe("€");
  });

  // Ohne diese Ausnahme würden ´ ` ^ nicht mehr komponieren, sondern verschluckt.
  it("gibt tote Tasten frei, statt sie selbst zu senden", () => {
    expect(altGraphChar(ev("Dead", { ctrl: true, alt: true }))).toBeNull();
    expect(altGraphChar(ev("Dead", { altGraph: true }))).toBeNull();
  });

  it("ignoriert Tasten, die kein einzelnes Zeichen liefern", () => {
    expect(altGraphChar(ev("ArrowLeft", { ctrl: true, alt: true }))).toBeNull();
    expect(altGraphChar(ev("Enter", { altGraph: true }))).toBeNull();
  });

  // Strg+C muss weiter als SIGINT bei xterm landen, nicht als Zeichen "c".
  it("lässt echtes Strg ohne Alt unangetastet", () => {
    expect(altGraphChar(ev("c", { ctrl: true }))).toBeNull();
  });

  it("lässt Alt ohne Strg unangetastet", () => {
    expect(altGraphChar(ev("b", { alt: true }))).toBeNull();
  });

  it("liefert null für normale Tasten ohne Modifier", () => {
    expect(altGraphChar(ev("a"))).toBeNull();
    expect(altGraphChar(ev("ä"))).toBeNull();
  });

  it("greift nicht bei zusätzlich gedrücktem Meta", () => {
    expect(altGraphChar(ev("@", { ctrl: true, alt: true, meta: true }))).toBeNull();
    expect(altGraphChar(ev("@", { altGraph: true, meta: true }))).toBeNull();
  });

  it("kommt ohne getModifierState aus, falls die Umgebung es nicht anbietet", () => {
    const bare = {
      key: "|",
      ctrlKey: true,
      altKey: true,
      metaKey: false,
    } as KeyEventLike;
    expect(altGraphChar(bare)).toBe("|");
  });
});
