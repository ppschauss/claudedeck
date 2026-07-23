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
import { FitAddon } from "@xterm/addon-fit";
import { SearchAddon } from "@xterm/addon-search";
import { Terminal, type IDisposable } from "@xterm/xterm";

interface TermEntry {
  term: Terminal;
  fit: FitAddon;
  search: SearchAddon;
  el: HTMLDivElement;
  disposables: IDisposable[];
}

const pool = new Map<string, TermEntry>();

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

  const term = new Terminal({ scrollback: 10000, fontFamily: "Consolas, monospace" });
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
