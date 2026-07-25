import { describe, expect, it } from "vitest";
// `?raw` statt `node:fs`: die App-Konfiguration kennt bewusst nur `vite/client`-Typen, und
// Browser-Code soll keine Node-Typen hereinziehen.
import source from "../termPool.ts?raw";

/**
 * `termPool.ts` selbst ist bewusst nicht unit-getestet (es braucht ein echtes DOM, siehe
 * Modulkommentar dort). Diese Prüfung sichert stattdessen eine Eigenschaft ab, die dem übrigen
 * Testlauf zwangsläufig entgeht — und die genau einmal sehr teuer war.
 */

describe("termPool: xterm-Stylesheet", () => {
  /**
   * Ohne `@xterm/xterm/css/xterm.css` fehlen die strukturellen Regeln, die xterm nicht selbst
   * einschleust — `DomRenderer` liefert nur Farben und Schriften nach.
   *
   * Messbare Folgen (im Browser nachgestellt, xterm 6.0):
   * - `.xterm-char-measure-element` bleibt sichtbar im Textfluss und schiebt die Zeilen **63px**
   *   nach unten. Bei 22px Zellenhöhe sind das 2,9 Zeilen — und weil `SelectionService` die
   *   Zeile als `(clientY − screenElement.top) / Zellenhöhe` berechnet, markiert die Maus
   *   entsprechend weit **unterhalb** des Zeigers.
   * - `.xterm-viewport` bekommt kein `overflow-y: scroll; position: absolute`, wodurch das
   *   Terminal seinen Container sprengt statt intern zu scrollen.
   *
   * Beides ist im laufenden Betrieb sichtbar, aber für Typprüfung und Unit-Tests unsichtbar —
   * deshalb diese Wache.
   */
  it("bindet das Stylesheet ein", () => {
    expect(source).toMatch(/import\s+["']@xterm\/xterm\/css\/xterm\.css["']/);
  });
});
