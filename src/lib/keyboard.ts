/**
 * Tastatur-Sonderfälle, die xterm.js im WebView2 nicht zuverlässig selbst abdeckt — als reine
 * Entscheidungslogik (Hausstil wie `badges.ts`/`sessionSwitch.ts`), damit sie ohne DOM testbar
 * ist. Das ist hier besonders wichtig: real prüfen lässt sich das nur im Windows-Build aus
 * GitHub Actions.
 *
 * Hintergrund AltGr: xterm 6.0 hat mit `_isThirdLevelShift`
 * (`node_modules/@xterm/xterm/src/browser/CoreBrowserTerminal.ts:1105-1117`) durchaus eine
 * Behandlung für Windows-AltGr — sie hängt aber an `isWindows`, das aus dem **deprecated**
 * `navigator.platform` abgeleitet wird (`src/common/Platform.ts:41`). Ob das im WebView2 greift,
 * ist nicht verlässlich; deshalb wird der Fall hier plattformunabhängig selbst entschieden.
 */

/**
 * Strukturelle Teilmenge von `KeyboardEvent` — ein echtes `KeyboardEvent` erfüllt sie von selbst.
 * Bewusst kein `KeyboardEvent` als Parametertyp: die Vitest-Umgebung ist `node` und kennt die
 * DOM-Klasse nicht. `getModifierState` ist optional, weil eine Umgebung sie nicht anbieten muss.
 */
export interface KeyEventLike {
  key: string;
  ctrlKey: boolean;
  altKey: boolean;
  metaKey: boolean;
  getModifierState?: (name: string) => boolean;
}

/**
 * Liefert das Zeichen, das eine AltGr-Kombination erzeugt (`@ { } [ ] \ | ~ €` auf deutschem
 * Layout) — oder `null`, wenn das Event keine AltGr-Eingabe ist und xterm es normal behandeln
 * soll.
 *
 * Zwei Erkennungswege, weil Chromium beide Varianten zeigt: der `AltGraph`-Modifier, und der
 * Windows-Fallback, bei dem AltGr als Strg+Alt ankommt.
 *
 * Bewusste Ausnahmen:
 * - `Dead` (die Akzenttasten ´ ` ^) wird durchgelassen, sonst bricht die Komposition.
 * - Alles, was kein einzelnes Zeichen ist (`ArrowLeft`, `Enter`, …), gehört xterm.
 * - Echtes Strg ohne Alt bleibt unangetastet, sonst käme Strg+C als Buchstabe „c" an statt als
 *   SIGINT.
 * - Zusätzliches Meta schließt aus, dass hier Fenster-/System-Kürzel abgefangen werden.
 */
export function altGraphChar(ev: KeyEventLike): string | null {
  if (ev.metaKey) return null;
  if (ev.key === "Dead") return null;
  if (ev.key.length !== 1) return null;

  const altGraph = ev.getModifierState?.("AltGraph") === true;
  if (!altGraph && !(ev.ctrlKey && ev.altKey)) return null;

  return ev.key;
}
