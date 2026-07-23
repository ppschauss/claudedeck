/**
 * Reconnect-UI (Task 6): schlanke Banner-Leiste während `connection-state === "reconnecting"`
 * (Countdown, "Jetzt neu verbinden") und ein Dialog-artiger Block bei `"failed"` (kein
 * Auto-Retry nach AuthFailed, Global Constraint — nur manuelle Optionen). Rendert `null` für
 * alle anderen Zustände.
 *
 * Der Countdown selbst kommt aus `connectionStore`s bereits vorhandenem `tick()`
 * (`tickRetryCountdown`, Task 4) — diese Komponente treibt nur das `setInterval(1000)`, solange
 * `state === "reconnecting"` ist.
 */
import { useEffect } from "react";
import { describeApiError } from "../lib/apiError";
import { connect } from "../lib/ipc";
import { useConnectionStore } from "../stores/connectionStore";
import { useToastStore } from "../stores/toastStore";

interface ReconnectOverlayProps {
  /** Aufgerufen, wenn der Nutzer im "failed"-Zustand explizit zurück zur Anmeldung will (siehe
   * App.tsx — setzt dort den `connected`-Flag zurück auf `false`). */
  onGiveUp: () => void;
}

export function ReconnectOverlay({ onGiveUp }: ReconnectOverlayProps) {
  const connectionState = useConnectionStore((s) => s.connectionState);

  useEffect(() => {
    if (connectionState.state !== "reconnecting") return;
    const id = setInterval(() => useConnectionStore.getState().tick(), 1000);
    return () => clearInterval(id);
  }, [connectionState.state]);

  async function handleManualRetry() {
    try {
      // Weckt einen laufenden Backoff-`sleep` im Rust-Supervisor sofort auf (wake_retry) UND
      // versucht selbst direkt zu verbinden — siehe reconnect_supervisor.rs.
      await connect();
    } catch (err) {
      useToastStore.getState().push(describeApiError(err));
    }
  }

  if (connectionState.state === "reconnecting") {
    return (
      <div className="reconnect-banner" role="status">
        <span>
          Verbindung verloren – neuer Versuch
          {connectionState.attempt !== null && ` (Versuch ${connectionState.attempt})`}
          {connectionState.nextRetryInS !== null && ` in ${connectionState.nextRetryInS}s`}
        </span>
        <button type="button" onClick={() => void handleManualRetry()}>
          Jetzt neu verbinden
        </button>
      </div>
    );
  }

  if (connectionState.state === "failed") {
    return (
      <div className="dialog-backdrop">
        <div className="dialog reconnect-failed-box">
          <h2>Verbindung fehlgeschlagen</h2>
          <p>
            Automatisches Neuverbinden wurde gestoppt (z.B. falsches oder abgelaufenes
            Passwort). Prüfe die Zugangsdaten und versuche es erneut, oder gehe zurück zur
            Anmeldung.
          </p>
          <div className="dialog-actions">
            <button type="button" onClick={onGiveUp}>
              Zur Anmeldung
            </button>
            <button type="button" onClick={() => void handleManualRetry()}>
              Erneut versuchen
            </button>
          </div>
        </div>
      </div>
    );
  }

  return null;
}
