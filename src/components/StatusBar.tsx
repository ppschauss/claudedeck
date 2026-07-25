/**
 * Fußzeile: Verbindungsstatus (aus `connection-state`-Events, `connectionStore` Task 4) +
 * Hostname des Profils. Reconnect-Countdown/Overlay-Feinschliff ist Task 6 — hier nur die
 * schlichte Zustandsanzeige laut Task-5-Auftrag.
 */
import { useConnectionStore } from "../stores/connectionStore";
import type { ConnectionStateEvent } from "../lib/ipc";

const LABELS: Record<ConnectionStateEvent["state"], string> = {
  disconnected: "Getrennt",
  connecting: "Verbinde…",
  connected: "Verbunden",
  reconnecting: "Wiederverbinden…",
  failed: "Fehlgeschlagen",
};

interface StatusBarProps {
  host: string;
  onOpenSettings: () => void;
}

export function StatusBar({ host, onOpenSettings }: StatusBarProps) {
  const state = useConnectionStore((s) => s.connectionState);
  return (
    <div className="status-bar">
      <span className={`status-dot status-${state.state}`} aria-hidden="true" />
      <span className="status-label">{LABELS[state.state]}</span>
      {host && <span className="status-host">{host}</span>}
      <button
        type="button"
        className="status-settings"
        aria-label="Einstellungen (Strg+,)"
        title="Einstellungen (Strg+,)"
        onClick={onOpenSettings}
      >
        ⚙
      </button>
    </div>
  );
}
