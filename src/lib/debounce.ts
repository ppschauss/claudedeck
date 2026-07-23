/**
 * Reine Trailing-Debounce-Hilfsfunktion (Review-Fund M4-Task-5, Fix 3 "Resize drosseln"):
 * mehrere Aufrufe innerhalb von `ms` lösen nur einen einzigen Aufruf von `fn` aus — mit den
 * Argumenten des LETZTEN Aufrufs im Fenster — sobald `ms` seit diesem letzten Aufruf ohne
 * weiteren Aufruf vergangen sind. Kein `now()`-Parameter nötig (anders als `badges.ts`): der
 * einzige Aufrufer (Resize-IPC in `Sidebar.tsx`) braucht keine deterministische Fake-Zeit-
 * Steuerung im Store, nur im eigenen Test hier (vitest fake timers).
 */
export function debounceTrailing<Args extends unknown[]>(
  fn: (...args: Args) => void,
  ms: number,
): (...args: Args) => void {
  let timer: ReturnType<typeof setTimeout> | undefined;

  return (...args: Args) => {
    if (timer !== undefined) clearTimeout(timer);
    timer = setTimeout(() => {
      timer = undefined;
      fn(...args);
    }, ms);
  };
}
