/**
 * Farbschemata und Darstellungsoptionen fürs Terminal — reine Daten plus zwei kleine
 * Hilfsfunktionen, damit die Auswahl ohne DOM testbar bleibt. Angewendet wird sie in
 * `termPool.applyDisplay()`.
 *
 * Jedes Schema bringt zusätzlich eine `accent`-Farbe mit: die schreibt `App.tsx` in die
 * CSS-Variablen `--accent`/`--accent-bg`, an denen Sidebar-Auswahl, Badges und Fokusringe
 * ohnehin schon hängen. Ein Themenwechsel färbt damit die ganze App mit, ohne dass jede
 * Komponente davon wissen muss.
 */
import type { ITheme } from "@xterm/xterm";

export interface TerminalTheme {
  id: string;
  name: string;
  /** App-Akzent (Auswahl, Badges, Fokus) — bewusst kräftiger als die ANSI-Farben. */
  accent: string;
  /** Flächige Variante des Akzents für Hintergründe. */
  accentBg: string;
  xterm: ITheme;
}

/** Cursorformen, die xterm kennt. */
export type CursorStyle = "block" | "underline" | "bar";

export interface TerminalDisplay {
  themeId: string;
  fontFamily: string;
  fontSize: number;
  lineHeight: number;
  letterSpacing: number;
  cursorStyle: CursorStyle;
  cursorBlink: boolean;
  scrollback: number;
}

const MIN_FONT_SIZE = 8;
const MAX_FONT_SIZE = 32;

/**
 * ClaudeDeck Dark leitet sich aus der bestehenden App-Palette ab (`index.css`), damit das
 * Standard-Terminal nicht wie ein Fremdkörper im Fenster sitzt.
 */
export const TERMINAL_THEMES: TerminalTheme[] = [
  {
    id: "claudedeck-dark",
    name: "ClaudeDeck Dark",
    accent: "#c084fc",
    accentBg: "rgba(192, 132, 252, 0.16)",
    xterm: {
      background: "#0d0e12",
      foreground: "#c7c9d1",
      cursor: "#c084fc",
      cursorAccent: "#0d0e12",
      selectionBackground: "rgba(192, 132, 252, 0.30)",
      black: "#1b1c23",
      red: "#f87171",
      green: "#4ade80",
      yellow: "#facc15",
      blue: "#60a5fa",
      magenta: "#c084fc",
      cyan: "#22d3ee",
      white: "#c7c9d1",
      brightBlack: "#4b4e5a",
      brightRed: "#fca5a5",
      brightGreen: "#86efac",
      brightYellow: "#fde047",
      brightBlue: "#93c5fd",
      brightMagenta: "#d8b4fe",
      brightWhite: "#f3f4f6",
      brightCyan: "#67e8f9",
    },
  },
  {
    id: "tokyo-night",
    name: "Tokyo Night",
    accent: "#7aa2f7",
    accentBg: "rgba(122, 162, 247, 0.18)",
    xterm: {
      background: "#1a1b26",
      foreground: "#a9b1d6",
      cursor: "#c0caf5",
      cursorAccent: "#1a1b26",
      selectionBackground: "rgba(122, 162, 247, 0.30)",
      black: "#15161e",
      red: "#f7768e",
      green: "#9ece6a",
      yellow: "#e0af68",
      blue: "#7aa2f7",
      magenta: "#bb9af7",
      cyan: "#7dcfff",
      white: "#a9b1d6",
      brightBlack: "#414868",
      brightRed: "#f7768e",
      brightGreen: "#9ece6a",
      brightYellow: "#e0af68",
      brightBlue: "#7aa2f7",
      brightMagenta: "#bb9af7",
      brightCyan: "#7dcfff",
      brightWhite: "#c0caf5",
    },
  },
  {
    id: "nord",
    name: "Nord",
    accent: "#88c0d0",
    accentBg: "rgba(136, 192, 208, 0.18)",
    xterm: {
      background: "#2e3440",
      foreground: "#d8dee9",
      cursor: "#d8dee9",
      cursorAccent: "#2e3440",
      selectionBackground: "rgba(136, 192, 208, 0.30)",
      black: "#3b4252",
      red: "#bf616a",
      green: "#a3be8c",
      yellow: "#ebcb8b",
      blue: "#81a1c1",
      magenta: "#b48ead",
      cyan: "#88c0d0",
      white: "#e5e9f0",
      brightBlack: "#4c566a",
      brightRed: "#bf616a",
      brightGreen: "#a3be8c",
      brightYellow: "#ebcb8b",
      brightBlue: "#81a1c1",
      brightMagenta: "#b48ead",
      brightCyan: "#8fbcbb",
      brightWhite: "#eceff4",
    },
  },
  {
    id: "gruvbox-dark",
    name: "Gruvbox Dark",
    accent: "#fabd2f",
    accentBg: "rgba(250, 189, 47, 0.18)",
    xterm: {
      background: "#282828",
      foreground: "#ebdbb2",
      cursor: "#ebdbb2",
      cursorAccent: "#282828",
      selectionBackground: "rgba(250, 189, 47, 0.28)",
      black: "#282828",
      red: "#cc241d",
      green: "#98971a",
      yellow: "#d79921",
      blue: "#458588",
      magenta: "#b16286",
      cyan: "#689d6a",
      white: "#a89984",
      brightBlack: "#928374",
      brightRed: "#fb4934",
      brightGreen: "#b8bb26",
      brightYellow: "#fabd2f",
      brightBlue: "#83a598",
      brightMagenta: "#d3869b",
      brightCyan: "#8ec07c",
      brightWhite: "#ebdbb2",
    },
  },
  {
    id: "solarized-dark",
    name: "Solarized Dark",
    accent: "#2aa198",
    accentBg: "rgba(42, 161, 152, 0.20)",
    xterm: {
      background: "#002b36",
      foreground: "#93a1a1",
      cursor: "#93a1a1",
      cursorAccent: "#002b36",
      selectionBackground: "rgba(42, 161, 152, 0.30)",
      black: "#073642",
      red: "#dc322f",
      green: "#859900",
      yellow: "#b58900",
      blue: "#268bd2",
      magenta: "#d33682",
      cyan: "#2aa198",
      white: "#eee8d5",
      brightBlack: "#586e75",
      brightRed: "#cb4b16",
      brightGreen: "#586e75",
      brightYellow: "#657b83",
      brightBlue: "#839496",
      brightMagenta: "#6c71c4",
      brightCyan: "#93a1a1",
      brightWhite: "#fdf6e3",
    },
  },
  {
    id: "catppuccin-mocha",
    name: "Catppuccin Mocha",
    accent: "#cba6f7",
    accentBg: "rgba(203, 166, 247, 0.18)",
    xterm: {
      background: "#1e1e2e",
      foreground: "#cdd6f4",
      cursor: "#f5e0dc",
      cursorAccent: "#1e1e2e",
      selectionBackground: "rgba(203, 166, 247, 0.30)",
      black: "#45475a",
      red: "#f38ba8",
      green: "#a6e3a1",
      yellow: "#f9e2af",
      blue: "#89b4fa",
      magenta: "#cba6f7",
      cyan: "#94e2d5",
      white: "#bac2de",
      brightBlack: "#585b70",
      brightRed: "#f38ba8",
      brightGreen: "#a6e3a1",
      brightYellow: "#f9e2af",
      brightBlue: "#89b4fa",
      brightMagenta: "#cba6f7",
      brightCyan: "#94e2d5",
      brightWhite: "#a6adc8",
    },
  },
];

/** Auswahl für den Schriftart-Regler. JetBrains Mono wird mitgeliefert (siehe `index.css`),
 * die übrigen sind auf Windows üblicherweise vorhanden; die Fallback-Kette fängt den Rest ab. */
export const FONT_CHOICES: { id: string; name: string; stack: string }[] = [
  { id: "jetbrains", name: "JetBrains Mono", stack: '"JetBrains Mono", Consolas, monospace' },
  { id: "cascadia", name: "Cascadia Mono", stack: '"Cascadia Mono", Consolas, monospace' },
  { id: "consolas", name: "Consolas", stack: "Consolas, monospace" },
  { id: "system", name: "System-Monospace", stack: "ui-monospace, monospace" },
];

export const DEFAULT_DISPLAY: TerminalDisplay = {
  themeId: "claudedeck-dark",
  fontFamily: FONT_CHOICES[0].stack,
  fontSize: 14,
  lineHeight: 1.2,
  letterSpacing: 0,
  cursorStyle: "bar",
  cursorBlink: true,
  scrollback: 10000,
};

/**
 * Schema zu einer ID. Unbekannte, leere oder fehlende IDs ergeben das erste Schema — eine in der
 * `config.json` vertippte oder aus einer älteren Version stammende ID darf die App nicht ohne
 * Farben dastehen lassen.
 */
export function themeById(id: string | null | undefined): TerminalTheme {
  return TERMINAL_THEMES.find((t) => t.id === id) ?? TERMINAL_THEMES[0];
}

/**
 * Hält die Schriftgröße im lesbaren Bereich und rundet auf ganze Punkt. Wichtig für den
 * Strg+/−-Zoom, der sich sonst in unlesbar oder absurd hineindrehen ließe; `NaN` (etwa aus einer
 * von Hand verkorksten Config) ergibt die Vorgabe statt eines kaputten Terminals.
 */
export function clampFontSize(size: number): number {
  if (!Number.isFinite(size)) return DEFAULT_DISPLAY.fontSize;
  return Math.min(Math.max(Math.round(size), MIN_FONT_SIZE), MAX_FONT_SIZE);
}
