import { describe, expect, it } from "vitest";
import {
  DEFAULT_DISPLAY,
  TERMINAL_THEMES,
  clampFontSize,
  themeById,
  type TerminalTheme,
} from "../terminalTheme";

const HEX = /^#[0-9a-f]{6}$/i;

/** Die 16 ANSI-Farben, die ein Terminal-Farbschema vollständig machen. */
const ANSI = [
  "black",
  "red",
  "green",
  "yellow",
  "blue",
  "magenta",
  "cyan",
  "white",
  "brightBlack",
  "brightRed",
  "brightGreen",
  "brightYellow",
  "brightBlue",
  "brightMagenta",
  "brightCyan",
  "brightWhite",
] as const;

describe("TERMINAL_THEMES", () => {
  it("enthält mehrere Schemata", () => {
    expect(TERMINAL_THEMES.length).toBeGreaterThanOrEqual(5);
  });

  it("vergibt eindeutige IDs", () => {
    const ids = TERMINAL_THEMES.map((t) => t.id);
    expect(new Set(ids).size).toBe(ids.length);
  });

  it("vergibt einen sichtbaren Namen je Schema", () => {
    for (const t of TERMINAL_THEMES) {
      expect(t.name.trim().length).toBeGreaterThan(0);
    }
  });

  // Ein unvollständiges Schema fällt erst auf, wenn ein Programm ausgerechnet diese Farbe
  // benutzt — deshalb hier vollständig geprüft statt stichprobenartig.
  it("definiert alle 16 ANSI-Farben plus Grundfarben als Hex", () => {
    for (const t of TERMINAL_THEMES) {
      for (const key of [...ANSI, "background", "foreground", "cursor"] as const) {
        expect(t.xterm[key], `${t.id}.${key}`).toMatch(HEX);
      }
    }
  });

  it("liefert je Schema eine App-Akzentfarbe", () => {
    for (const t of TERMINAL_THEMES) {
      expect(t.accent, t.id).toMatch(HEX);
    }
  });

  // Die Auswahl darf nicht am Hintergrund kleben, sonst verschwindet die aktive Session.
  it("hebt den Akzent deutlich vom Terminal-Hintergrund ab", () => {
    for (const t of TERMINAL_THEMES) {
      expect(t.accent.toLowerCase(), t.id).not.toBe(t.xterm.background?.toLowerCase());
    }
  });
});

describe("themeById", () => {
  it("findet ein Schema an seiner ID", () => {
    const wanted = TERMINAL_THEMES[2] as TerminalTheme;
    expect(themeById(wanted.id).id).toBe(wanted.id);
  });

  // Ein in der config.json vertipptes oder aus einer älteren Version stammendes Schema darf die
  // App nicht ohne Farben dastehen lassen.
  it("fällt bei unbekannter, leerer oder fehlender ID auf das erste Schema zurück", () => {
    const fallback = TERMINAL_THEMES[0].id;
    expect(themeById("gibts-nicht").id).toBe(fallback);
    expect(themeById("").id).toBe(fallback);
    expect(themeById(null).id).toBe(fallback);
    expect(themeById(undefined).id).toBe(fallback);
  });
});

describe("clampFontSize", () => {
  it("lässt übliche Größen unverändert", () => {
    expect(clampFontSize(14)).toBe(14);
    expect(clampFontSize(9)).toBe(9);
  });

  // Strg+/- darf sich nicht in unlesbar oder absurd hineindrehen lassen.
  it("begrenzt nach unten und oben", () => {
    expect(clampFontSize(2)).toBe(8);
    expect(clampFontSize(-10)).toBe(8);
    expect(clampFontSize(999)).toBe(32);
  });

  it("rundet Zwischenwerte auf ganze Punkt", () => {
    expect(clampFontSize(13.6)).toBe(14);
  });

  it("fällt bei ungültigen Werten auf die Vorgabe zurück", () => {
    expect(clampFontSize(Number.NaN)).toBe(DEFAULT_DISPLAY.fontSize);
  });
});

describe("DEFAULT_DISPLAY", () => {
  it("verweist auf ein vorhandenes Schema", () => {
    expect(TERMINAL_THEMES.some((t) => t.id === DEFAULT_DISPLAY.themeId)).toBe(true);
  });

  it("liegt mit der Schriftgröße im erlaubten Bereich", () => {
    expect(clampFontSize(DEFAULT_DISPLAY.fontSize)).toBe(DEFAULT_DISPLAY.fontSize);
  });
});
