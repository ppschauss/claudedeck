import { useEffect, useState } from "react";
import "./App.css";
import { CommandPanel } from "./components/CommandPanel";
import { ConnectGate } from "./components/ConnectGate";
import { NotificationManager } from "./components/NotificationManager";
import { ReconnectOverlay } from "./components/ReconnectOverlay";
import { Sidebar } from "./components/Sidebar";
import { StatusBar } from "./components/StatusBar";
import { TerminalHost } from "./components/TerminalHost";
import { ToastHost } from "./components/Toast";
import { getConfig, onConnectionState, onSessionReattached } from "./lib/ipc";
import { useCatalogStore } from "./stores/catalogStore";
import { useConnectionStore } from "./stores/connectionStore";
import { useSessionStore } from "./stores/sessionStore";

function App() {
  const [connected, setConnected] = useState(false);
  const [host, setHost] = useState("");

  // Über die gesamte App-Lebensdauer aktiv (auch schon während ConnectGate) — sonst würde der
  // erste "connecting"/"connected"-Übergang verpasst, weil der Listener erst nach dem
  // Verbindungsaufbau registriert würde.
  useEffect(() => {
    let cancelled = false;
    let unlisten: (() => void) | undefined;
    void onConnectionState((event) => {
      useConnectionStore.getState().eventReceived(event);
    }).then((fn) => {
      // Cleanup kann (React-StrictMode-Doppel-Mount im Dev-Build) schon gelaufen sein, bevor
      // `listen()` zurückkommt — dann sofort wieder abmelden statt einen zweiten, nie
      // eingesammelten Listener aktiv zu lassen (würde Events doppelt verarbeiten).
      if (cancelled) {
        fn();
      } else {
        unlisten = fn;
      }
    });
    return () => {
      cancelled = true;
      unlisten?.();
    };
  }, []);

  // Task 6, Auflage C: Backend hat nach einem Reconnect serverseitig re-attacht (derselbe
  // Channel, neues PTY) — hier nur `lost` im Store zurücksetzen (⚠ → ●), TermPool/Channel
  // laufen unverändert weiter. Läuft (wie der connection-state-Listener oben) über die gesamte
  // App-Lebensdauer, nicht nur während der Sidebar sichtbar ist.
  useEffect(() => {
    let cancelled = false;
    let unlisten: (() => void) | undefined;
    void onSessionReattached(({ sessionId }) => {
      useSessionStore.getState().reattached(sessionId);
    }).then((fn) => {
      if (cancelled) {
        fn();
      } else {
        unlisten = fn;
      }
    });
    return () => {
      cancelled = true;
      unlisten?.();
    };
  }, []);

  // Strg+B klappt das Befehls-Panel auf/zu. `!e.altKey` aus demselben Grund wie beim Strg+F der
  // Terminal-Suche: AltGr meldet sich unter Windows als Strg+Alt.
  useEffect(() => {
    function handleKeyDown(e: KeyboardEvent) {
      if ((e.ctrlKey || e.metaKey) && !e.altKey && e.key.toLowerCase() === "b") {
        e.preventDefault();
        useCatalogStore.getState().toggled();
      }
    }
    window.addEventListener("keydown", handleKeyDown);
    return () => window.removeEventListener("keydown", handleKeyDown);
  }, []);

  useEffect(() => {
    if (!connected) return;
    void getConfig()
      .then((config) => setHost(`${config.profile.user}@${config.profile.host}`))
      .catch(() => setHost(""));
  }, [connected]);

  if (!connected) {
    return (
      <div className="app-root">
        <ConnectGate onConnected={() => setConnected(true)} />
        <ToastHost />
      </div>
    );
  }

  return (
    <div className="app-root">
      <ReconnectOverlay onGiveUp={() => setConnected(false)} />
      <div className="app-body">
        <Sidebar />
        <TerminalHost />
        <CommandPanelToggle />
        <CommandPanel />
      </div>
      <StatusBar host={host} />
      <NotificationManager />
      <ToastHost />
    </div>
  );
}

/** Schmale Leiste zwischen Terminal und Panel — der einzige immer sichtbare Weg zum Panel. */
function CommandPanelToggle() {
  const open = useCatalogStore((s) => s.open);
  return (
    <button
      type="button"
      className="command-toggle"
      aria-expanded={open}
      aria-label={open ? "Befehls-Panel schließen (Strg+B)" : "Befehls-Panel öffnen (Strg+B)"}
      title={open ? "Befehls-Panel schließen (Strg+B)" : "Befehls-Panel öffnen (Strg+B)"}
      onClick={() => useCatalogStore.getState().toggled()}
    >
      {open ? "›" : "‹"}
    </button>
  );
}

export default App;
