import { describe, expect, it } from "vitest";
import { auswahlVersatz } from "../selectionProbe";

describe("auswahlVersatz", () => {
  it("meldet 0, wenn xterm genau die Zeile unter dem Zeiger markiert", () => {
    expect(auswahlVersatz({ viewportY: 0, zeileUnterZeiger: 3, auswahlStartY: 3 })).toBe(0);
  });

  it("rechnet den Scrollback-Versatz heraus", () => {
    // Der Zeiger steht auf der vierten sichtbaren Zeile; sichtbar beginnt bei Pufferzeile 100.
    expect(auswahlVersatz({ viewportY: 100, zeileUnterZeiger: 3, auswahlStartY: 103 })).toBe(0);
  });

  it("meldet den Fehler in Zeilen, wenn xterm zu weit unten markiert", () => {
    expect(auswahlVersatz({ viewportY: 100, zeileUnterZeiger: 3, auswahlStartY: 106 })).toBe(3);
  });

  it("meldet negativ, wenn xterm zu weit oben markiert", () => {
    expect(auswahlVersatz({ viewportY: 0, zeileUnterZeiger: 8, auswahlStartY: 6 })).toBe(-2);
  });

  it("liefert null, wenn gar nichts markiert wurde", () => {
    expect(auswahlVersatz({ viewportY: 0, zeileUnterZeiger: 3, auswahlStartY: null })).toBeNull();
  });

  it("liefert null, wenn der Zeiger auf keiner gerenderten Zeile lag", () => {
    expect(auswahlVersatz({ viewportY: 0, zeileUnterZeiger: null, auswahlStartY: 3 })).toBeNull();
  });
});
