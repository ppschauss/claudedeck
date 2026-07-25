import { useEffect, useState } from "react";
import "./App.css";
import { ConnectGate } from "./components/ConnectGate";
import { RightPanel } from "./components/RightPanel";
import { NotificationManager } from "./components/NotificationManager";
import { ReconnectOverlay } from "./components/ReconnectOverlay";
import { Sidebar } from "./components/Sidebar";
import { SettingsDialog } from "./components/dialogs/SettingsDialog";
import { StatusBar } from "./components/StatusBar";
import { TerminalHost } from "./components/TerminalHost";
import { ToastHost } from "./components/Toast";
import { getConfig, setConfig, onConnectionState, onSessionReattached } from "./lib/ipc";
import { applyDisplay } from "./lib/termPool";
import { clampFontSize, themeById, type TerminalDisplay } from "./lib/terminalTheme";
import { useCatalogStore } from "./stores/catalogStore";
import { useConfigStore } from "./stores/configStore";
import { useConnectionStore } from "./stores/connectionStore";
import { useSessionStore } from "./stores/sessionStore";

function App() {
  const [connected, setConnected] = useState(false);
  const [host, setHost] = useState("");
  const [settingsOpen, setSettingsOpen] = useState(false);

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
      if (!(e.ctrlKey || e.metaKey) || e.altKey) return;
      if (e.key.toLowerCase() === "b") {
        e.preventDefault();
        useCatalogStore.getState().toggled();
      } else if (e.key === ",") {
        e.preventDefault();
        setSettingsOpen((open) => !open);
      }
    }
    window.addEventListener("keydown", handleKeyDown);
    return () => window.removeEventListener("keydown", handleKeyDown);
  }, []);

  useEffect(() => {
    if (!connected) return;
    void getConfig()
      .then((config) => {
        setHost(`${config.profile.user}@${config.profile.host}`);
        useConfigStore.getState().loaded(config);
      })
      .catch(() => setHost(""));
  }, [connected]);

  // Darstellung anwenden, sobald sie sich ändert — beim Laden ebenso wie nach jeder Änderung im
  // Einstellungen-Dialog oder per Zoom.
  const terminal = useConfigStore((s) => s.config?.terminal);
  useEffect(() => {
    if (!terminal) return;
    applyTerminalDisplay(terminal);
  }, [terminal]);

  // Strg + / Strg − / Strg 0 — die mit Abstand häufigste Einstellung, deshalb auf der Tastatur.
  // `!e.altKey` wie bei den übrigen Kürzeln: AltGr meldet sich unter Windows als Strg+Alt.
  useEffect(() => {
    function handleKeyDown(e: KeyboardEvent) {
      if (!(e.ctrlKey || e.metaKey) || e.altKey) return;
      const current = useConfigStore.getState().config;
      if (!current) return;

      let size: number | null = null;
      if (e.key === "+" || e.key === "=") size = current.terminal.fontSize + 1;
      else if (e.key === "-") size = current.terminal.fontSize - 1;
      else if (e.key === "0") size = 14;
      if (size === null) return;

      e.preventDefault();
      const fontSize = clampFontSize(size);
      if (fontSize === current.terminal.fontSize) return;

      const next = { ...current, terminal: { ...current.terminal, fontSize } };
      useConfigStore.getState().loaded(next);
      // Fehler beim Speichern darf den Zoom nicht blockieren — die Größe wirkt sofort, sie
      // überlebt dann nur den Neustart nicht.
      void setConfig(next).catch(() => undefined);
    }
    window.addEventListener("keydown", handleKeyDown);
    return () => window.removeEventListener("keydown", handleKeyDown);
  }, []);

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
        <RightPanel />
      </div>
      <StatusBar host={host} onOpenSettings={() => setSettingsOpen(true)} />
      {settingsOpen && <SettingsDialog onClose={() => setSettingsOpen(false)} />}
      <NotificationManager />
      <ToastHost />
    </div>
  );
}

/**
 * Wendet die Terminal-Darstellung an — auf die Terminals *und* auf die App.
 *
 * Der zweite Teil ist der Grund, warum ein Themenwechsel sich nicht nach „nur das Terminal ist
 * jetzt blau" anfühlt: Sidebar-Auswahl, Badges und Fokusringe hängen bereits an den
 * CSS-Variablen `--accent`/`--accent-bg`, also genügt es, die zu überschreiben. Keine Komponente
 * muss dafür etwas über Themes wissen.
 */
function applyTerminalDisplay(display: TerminalDisplay): void {
  applyDisplay(display);
  const theme = themeById(display.themeId);
  const root = document.documentElement;
  root.style.setProperty("--accent", theme.accent);
  root.style.setProperty("--accent-bg", theme.accentBg);
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
