/**
 * Pool aller xterm.js-Instanzen, eine pro Session-ID — wird beim Sessionwechsel NIE disposed
 * (sofortiges Umschalten zwischen mehreren parallelen Sessions ist das ganze Ziel von M4/M5),
 * nur explizit über `dispose()` beim endgültigen Schließen einer Session entfernt.
 *
 * Braucht ein echtes DOM (xterm rendert in ein `HTMLElement`) — deshalb bewusst NICHT über
 * vitest getestet (jsdom-Canvas/ResizeObserver-Mocking wäre für den Nutzen zu aufwendig), nur
 * über `tsc -b` typgeprüft. Reine Entscheidungslogik lebt stattdessen in `badges.ts`/
 * `sessionStore.ts`.
 *
 * KEIN WebGL-Addon in M4/M5 (YAGNI + Kontext-Limit bei vielen parallelen Terminals) — erst bei
 * konkretem Perf-Bedarf nachrüsten.
 */
// Muss mit — und zwar hier, direkt neben dem Terminal, nicht irgendwo in main.tsx: xterm bringt
// nur Farben und Schriften selbst mit (`DomRenderer._injectCss`), die strukturellen Regeln
// stehen ausschließlich in dieser Datei. Ohne sie bleibt `.xterm-char-measure-element` sichtbar
// im Textfluss stehen und schiebt die Zeilen ~63px (knapp 3 Zeilen) nach unten, während
// `SelectionService` weiter ab Oberkante rechnet — die Maus markiert dann ein paar Zeilen zu
// tief. Außerdem fehlt `.xterm-viewport` sein `overflow-y: scroll`, weshalb das Terminal seinen
// Container sprengt statt intern zu scrollen. `termPool.test.ts` wacht darüber.
import "@xterm/xterm/css/xterm.css";
import { FitAddon } from "@xterm/addon-fit";
import { SearchAddon } from "@xterm/addon-search";
import { Terminal, type IDisposable } from "@xterm/xterm";
import { altGraphChar } from "./keyboard";
import { DEFAULT_DISPLAY, themeById, type TerminalDisplay } from "./terminalTheme";

interface TermEntry {
  term: Terminal;
  fit: FitAddon;
  search: SearchAddon;
  el: HTMLDivElement;
  disposables: IDisposable[];
}

const pool = new Map<string, TermEntry>();

/**
 * Aktuelle Darstellung. Wird von `applyDisplay()` gesetzt und von `ensure()` mitbenutzt, damit
 * ein *neu* geöffnetes Terminal sofort im gewählten Schema erscheint statt kurz in der Vorgabe
 * aufzublitzen.
 */
let currentDisplay: TerminalDisplay = DEFAULT_DISPLAY;

/** Übersetzt die Einstellungen in xterm-Optionen. */
function displayOptions(d: TerminalDisplay) {
  return {
    theme: themeById(d.themeId).xterm,
    fontFamily: d.fontFamily,
    fontSize: d.fontSize,
    lineHeight: d.lineHeight,
    letterSpacing: d.letterSpacing,
    cursorStyle: d.cursorStyle,
    cursorBlink: d.cursorBlink,
    scrollback: d.scrollback,
  };
}

/**
 * Übernimmt Schema und Darstellung für **alle** offenen Terminals und für künftige.
 *
 * Der `fit()`-Aufruf ist der eigentlich kritische Teil: eine geänderte Schriftgröße oder
 * Zeilenhöhe verändert, wie viele Zeichen ins Fenster passen. Ohne erneutes Einpassen behielte
 * tmux die alte Geometrie, und die Ausgabe würde an falschen Stellen umbrechen. `fit()` meldet
 * geänderte `cols`/`rows` über xterms `onResize` — in `ensure()` verdrahtet — von selbst ans
 * Backend weiter.
 *
 * Unsichtbare Terminals werden dabei übersprungen: an einem `display: none`-Element misst
 * `fit()` Unsinn (Höhe 0). Sie werden ohnehin beim nächsten `show()` eingepasst.
 */
export function applyDisplay(display: TerminalDisplay): void {
  currentDisplay = display;
  const opts = displayOptions(display);

  for (const entry of pool.values()) {
    const o = entry.term.options;
    o.theme = opts.theme;
    o.fontFamily = opts.fontFamily;
    o.fontSize = opts.fontSize;
    o.lineHeight = opts.lineHeight;
    o.letterSpacing = opts.letterSpacing;
    o.cursorStyle = opts.cursorStyle;
    o.cursorBlink = opts.cursorBlink;
    o.scrollback = opts.scrollback;

    if (entry.el.style.display !== "none") entry.fit.fit();
  }
}

/**
 * Erzeugt (falls noch nicht vorhanden) das Terminal für `sessionId` in einem noch nicht ins DOM
 * gehängten `<div>`. `onData` bekommt jeden Tastatur-/Paste-Chunk bereits als UTF-8-Bytes
 * (xterm liefert Strings — die Kodierung passiert hier, NICHT beim Aufrufer); `onResize` feuert,
 * wann immer sich `cols`/`rows` ändern (u.a. durch `fit()`). Wiederholte Aufrufe für dieselbe
 * `sessionId` sind ein No-Op und geben den bestehenden Eintrag zurück — `onData`/`onResize` der
 * ursprünglichen `ensure()`-Aufrufs bleiben aktiv, neue Callback-Referenzen werden ignoriert.
 */
export function ensure(
  sessionId: string,
  onData: (bytes: Uint8Array) => void,
  onResize: (cols: number, rows: number) => void,
): TermEntry {
  const existing = pool.get(sessionId);
  if (existing) return existing;

  const term = new Terminal(displayOptions(currentDisplay));
  const fit = new FitAddon();
  const search = new SearchAddon();
  term.loadAddon(fit);
  term.loadAddon(search);

  const el = document.createElement("div");
  el.style.display = "none";
  el.style.width = "100%";
  el.style.height = "100%";
  term.open(el);

  const encoder = new TextEncoder();
  const disposables: IDisposable[] = [
    term.onData((data) => onData(encoder.encode(data))),
    term.onResize(({ cols, rows }) => onResize(cols, rows)),
  ];

  // AltGr-Zeichen (@ { } [ ] \ | ~ €) selbst senden — `keyboard.ts` begründet, warum xterms
  // eigene Behandlung im WebView2 nicht verlässlich greift.
  //
  // Zwei Details, ohne die das Zeichen doppelt ankäme:
  // 1. Der Handler wird auch für `keyup` aufgerufen (`_keyUp` in xterms `CoreBrowserTerminal`
  //    ruft denselben `_customKeyEventHandler`) — ohne den `keydown`-Guard würde jedes Zeichen
  //    zweimal gesendet.
  // 2. `preventDefault()` ist zwingend: ohne es tippt der Browser das Zeichen zusätzlich in
  //    xterms verstecktes `<textarea>`, von wo es ein zweites Mal als Eingabe herauskommt.
  //
  // `false` als Rückgabe heißt „xterm soll dieses Event nicht weiter verarbeiten".
  term.attachCustomKeyEventHandler((ev) => {
    if (ev.type !== "keydown") return true;
    const char = altGraphChar(ev);
    if (char === null) return true;
    ev.preventDefault();
    onData(encoder.encode(char));
    return false;
  });

  const entry: TermEntry = { term, fit, search, el, disposables };
  pool.set(sessionId, entry);
  return entry;
}

/** Hängt das Terminal-`<div>` von `sessionId` in `host`, macht es sichtbar, passt die Größe an
 * `host` an und fokussiert es. No-Op, falls `ensure()` für diese `sessionId` nie aufgerufen
 * wurde (defensiv — der Aufrufer sollte immer erst `ensure()` rufen). */
export function show(sessionId: string, host: HTMLElement): void {
  const entry = pool.get(sessionId);
  if (!entry) return;
  if (entry.el.parentElement !== host) {
    host.appendChild(entry.el);
  }
  entry.el.style.display = "block";
  entry.fit.fit();
  entry.term.focus();
}

/** Blendet das Terminal von `sessionId` aus (bleibt im DOM/Pool, wird nur unsichtbar). */
export function hide(sessionId: string): void {
  const entry = pool.get(sessionId);
  if (!entry) return;
  entry.el.style.display = "none";
}

/**
 * Passt das Terminal von `sessionId` an seine aktuelle Host-Größe an (Task 5:
 * `ResizeObserver` in `TerminalHost`) und liefert die resultierenden `cols`/`rows` zurück.
 * Anders als `show()` OHNE Fokus-Diebstahl — ein reiner Größenwechsel (Fenster-Resize,
 * Sidebar-Toggle) soll nicht den Fokus von woanders ins Terminal reißen. Ändert `fit()`
 * tatsächlich `cols`/`rows`, feuert xterms eigenes `onResize` (in `ensure()` verdrahtet) und
 * meldet die neue Größe darüber ans Backend — diese Funktion selbst löst keinen IPC-Call aus.
 * No-Op (liefert `null`), falls `ensure()` für diese `sessionId` nie aufgerufen wurde.
 */
export function fit(sessionId: string): { cols: number; rows: number } | null {
  const entry = pool.get(sessionId);
  if (!entry) return null;
  entry.fit.fit();
  return { cols: entry.term.cols, rows: entry.term.rows };
}

/** Liefert das `SearchAddon` von `sessionId` (Task 6, Strg+F) — `null`, falls `ensure()` für
 * diese `sessionId` nie aufgerufen wurde. Kein eigener Zustand hier: `SearchBar.tsx` ruft
 * `findNext`/`findPrevious` direkt auf dem zurückgegebenen Addon auf. */
export function search(sessionId: string): SearchAddon | null {
  return pool.get(sessionId)?.search ?? null;
}

/** Schreibt rohe PTY-Bytes direkt ins Terminal — `Terminal.write` akzeptiert `Uint8Array` und
 * puffert intern korrekt über an Chunk-Grenzen aufgetrennte UTF-8-Multibyte-Sequenzen hinweg. */
export function write(sessionId: string, bytes: Uint8Array): void {
  pool.get(sessionId)?.term.write(bytes);
}

/** Messwerte zur Fehlersuche beim Auswahl-Versatz (siehe `diagnose`). */
export interface TerminalDiagnostics {
  devicePixelRatio: number;
  /** Zeilenhöhe, die xterm annimmt — es setzt sie selbst als `style.height` je Zeile. */
  angenommeneZeilenhoehe: number;
  /** Tatsächlicher Abstand zweier aufeinanderfolgender Zeilen im Layout. */
  echterZeilenabstand: number;
  /** Auseinanderdriften über zehn Zeilen. Der Wert, auf den es ankommt. */
  driftNachZehnZeilen: number;
  /** Abstand zwischen Oberkante des Bezugselements der Mausrechnung und der ersten Zeile. */
  versatzObenPx: number;
  schriftart: string;
}

/**
 * Liest die Zahlen aus, die den Auswahl-Versatz erklären würden.
 *
 * `SelectionService` rechnet die Zeile als `(clientY − screenElement.top) / Zellenhöhe`. Zwei
 * Dinge können dabei schiefgehen, und genau die werden hier gemessen:
 * - ein **konstanter** Versatz oben (etwas schiebt die Zeilen nach unten), oder
 * - ein **wachsender** Fehler, wenn der tatsächliche Zeilenabstand von der angenommenen
 *   Zellenhöhe abweicht — dann stimmt die Auswahl oben noch und wird nach unten immer falscher.
 *
 * Bewusst ohne Zugriff auf xterms private Interna: die angenommene Höhe steht als `style.height`
 * an jeder Zeile, weil der DOM-Renderer sie selbst dort hineinschreibt.
 *
 * Existiert nur, weil sich der Fehler in Firefox nicht nachstellen ließ — die Zahlen müssen aus
 * dem echten WebView2 kommen.
 */
export function diagnose(sessionId: string): TerminalDiagnostics | null {
  const entry = pool.get(sessionId);
  if (!entry) return null;

  const screen = entry.el.querySelector(".xterm-screen");
  const rows = entry.el.querySelector(".xterm-rows");
  if (!screen || !rows || rows.children.length < 11) return null;

  const zeile = (i: number) => (rows.children[i] as HTMLElement).getBoundingClientRect();
  const angenommen = parseFloat(
    (rows.children[0] as HTMLElement).style.height || "0",
  );
  const echt = zeile(1).top - zeile(0).top;

  const round = (n: number) => Math.round(n * 100) / 100;
  return {
    devicePixelRatio: window.devicePixelRatio,
    angenommeneZeilenhoehe: round(angenommen),
    echterZeilenabstand: round(echt),
    driftNachZehnZeilen: round(zeile(10).top - zeile(0).top - 10 * angenommen),
    versatzObenPx: round(rows.getBoundingClientRect().top - screen.getBoundingClientRect().top),
    schriftart: entry.term.options.fontFamily ?? "",
  };
}

/** Entfernt das Terminal endgültig (Session wirklich beendet/gekillt, nicht nur detached/
 * ausgeblendet) — disposed xterm + Addons, entfernt das `<div>` aus dem DOM und den Pool-Eintrag. */
export function dispose(sessionId: string): void {
  const entry = pool.get(sessionId);
  if (!entry) return;
  for (const d of entry.disposables) d.dispose();
  entry.term.dispose();
  entry.el.remove();
  pool.delete(sessionId);
}
