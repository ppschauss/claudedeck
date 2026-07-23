import { useEffect, useState } from "react";
import "./App.css";
import { ConnectGate } from "./components/ConnectGate";
import { Sidebar } from "./components/Sidebar";
import { StatusBar } from "./components/StatusBar";
import { TerminalHost } from "./components/TerminalHost";
import { getConfig, onConnectionState } from "./lib/ipc";
import { useConnectionStore } from "./stores/connectionStore";

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
      </div>
    );
  }

  return (
    <div className="app-root">
      <div className="app-body">
        <Sidebar />
        <TerminalHost />
      </div>
      <StatusBar host={host} />
    </div>
  );
}

export default App;
