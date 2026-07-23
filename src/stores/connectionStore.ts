/**
 * Zustand-Store für den Verbindungsstatus (`connection-state`-Events) + Reconnect-Countdown.
 * Die eigentliche Zustandslogik steckt in zwei reinen Funktionen (`reduceConnectionState`,
 * `tickRetryCountdown`) — der Store selbst ist nur eine dünne Hülle darum, die per
 * `useConnectionStore.getState().eventReceived(...)` ganz ohne React getestet werden kann.
 */
import { create } from "zustand";
import type { ConnectionStateEvent } from "../lib/ipc";

export interface ConnectionState {
  state: ConnectionStateEvent["state"];
  attempt: number | null;
  nextRetryInS: number | null;
}

export const initialConnectionState: ConnectionState = {
  state: "disconnected",
  attempt: null,
  nextRetryInS: null,
};

/** Übernimmt ein `connection-state`-Event 1:1 in den State. `attempt`/`nextRetryInS` sind im
 * Event optional (der Backend-Emitter füllt sie nur bei `reconnecting`) — fehlen sie, werden
 * sie explizit auf `null` zurückgesetzt statt den alten Wert aus einem früheren
 * `reconnecting`-Zyklus mitzuschleppen (sonst würde z.B. nach einem erfolgreichen Reconnect ein
 * stehengebliebener Countdown im UI weiterhängen). */
export function reduceConnectionState(
  _prev: ConnectionState,
  event: ConnectionStateEvent,
): ConnectionState {
  return {
    state: event.state,
    attempt: event.attempt ?? null,
    nextRetryInS: event.nextRetryInS ?? null,
  };
}

/** Zählt den Reconnect-Countdown um eine Sekunde herunter (Aufrufer: `setInterval(1000)` im
 * ReconnectOverlay, Task 6). Kein Countdown aktiv (`null`) oder schon bei 0 → No-Op, nie
 * negativ. */
export function tickRetryCountdown(s: ConnectionState): ConnectionState {
  if (s.nextRetryInS === null || s.nextRetryInS <= 0) return s;
  return { ...s, nextRetryInS: s.nextRetryInS - 1 };
}

export interface ConnectionStore {
  connectionState: ConnectionState;
  eventReceived: (event: ConnectionStateEvent) => void;
  tick: () => void;
}

export const useConnectionStore = create<ConnectionStore>((set) => ({
  connectionState: initialConnectionState,
  eventReceived: (event) =>
    set((s) => ({ connectionState: reduceConnectionState(s.connectionState, event) })),
  tick: () => set((s) => ({ connectionState: tickRetryCountdown(s.connectionState) })),
}));
