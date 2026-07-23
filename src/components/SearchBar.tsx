/**
 * Scrollback-Suche (Task 6, Strg+F): dünne Hülle um `@xterm/addon-search`, über
 * `termPool.search(sessionId)` (kein eigener Suchzustand hier). `Enter`/`↓` = nächster Treffer,
 * `Shift+Enter`/`↑` = vorheriger, `Esc` schließt (an `TerminalHost`, das den Ctrl+F-Listener
 * hält und diese Komponente conditional rendert).
 */
import { useEffect, useRef, useState } from "react";
import * as termPool from "../lib/termPool";

interface SearchBarProps {
  sessionId: string;
  onClose: () => void;
}

export function SearchBar({ sessionId, onClose }: SearchBarProps) {
  const [term, setTerm] = useState("");
  const inputRef = useRef<HTMLInputElement>(null);

  useEffect(() => {
    inputRef.current?.focus();
    inputRef.current?.select();
  }, []);

  function next(value: string) {
    termPool.search(sessionId)?.findNext(value);
  }

  function prev(value: string) {
    termPool.search(sessionId)?.findPrevious(value);
  }

  function handleChange(value: string) {
    setTerm(value);
    // Inkrementell suchen, während getippt wird — springt zum nächsten Treffer relativ zur
    // aktuellen Scroll-Position statt bei jedem Tastendruck von vorn zu beginnen.
    termPool.search(sessionId)?.findNext(value, { incremental: true });
  }

  function handleKeyDown(e: React.KeyboardEvent<HTMLInputElement>) {
    if (e.key === "Escape") {
      e.preventDefault();
      onClose();
      return;
    }
    if (e.key === "Enter") {
      e.preventDefault();
      if (e.shiftKey) prev(term);
      else next(term);
    }
  }

  return (
    <div className="search-bar">
      <input
        ref={inputRef}
        type="text"
        value={term}
        placeholder="Im Scrollback suchen…"
        onChange={(e) => handleChange(e.target.value)}
        onKeyDown={handleKeyDown}
      />
      <button type="button" onClick={() => prev(term)} aria-label="Vorheriger Treffer">
        ↑
      </button>
      <button type="button" onClick={() => next(term)} aria-label="Nächster Treffer">
        ↓
      </button>
      <button type="button" onClick={onClose} aria-label="Suche schließen">
        ×
      </button>
    </div>
  );
}
