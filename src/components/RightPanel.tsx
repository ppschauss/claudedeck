/**
 * Rahmen des rechten Panels: `<aside>`, Reiterleiste und Auf-/Zuklappen. Der Inhalt kommt aus
 * `CommandPanel` (Befehle) oder `FilePanel` (Ablage).
 *
 * Reiter statt einer vierten Spalte: `Strg+B`, der Umschalter und das vorhandene CSS bleiben
 * damit unverändert, und zwei schmale Panels nebeneinander wären auf einem Laptop-Bildschirm
 * ohnehin zu eng.
 */
import { useMemo, useState } from "react";
import { useCatalogStore } from "../stores/catalogStore";
import { useSessionStore } from "../stores/sessionStore";
import { CommandPanel } from "./CommandPanel";
import { FilePanel } from "./FilePanel";

type Tab = "commands" | "files";

export function RightPanel() {
  const open = useCatalogStore((s) => s.open);
  const [tab, setTab] = useState<Tab>("commands");

  const activeSessionId = useSessionStore((s) => s.activeSessionId);
  const running = useSessionStore((s) => s.running);
  const openSessions = useSessionStore((s) => s.openSessions);

  // Arbeitsverzeichnis der aktiven Session — dieselbe Quelle, aus der das Befehls-Panel seine
  // projektlokalen Einträge zieht (`SessionInfo.cwd`), damit beide Reiter dasselbe Projekt
  // meinen.
  const projectDir = useMemo(() => {
    if (!activeSessionId) return null;
    const name = openSessions.get(activeSessionId)?.name;
    if (!name) return null;
    return running.find((s) => s.name === name)?.cwd ?? null;
  }, [activeSessionId, openSessions, running]);

  if (!open) return null;

  return (
    <aside className="command-panel">
      <div className="settings-tabs panel-tabs" role="tablist">
        <button
          type="button"
          role="tab"
          aria-selected={tab === "commands"}
          className={tab === "commands" ? "settings-tab active" : "settings-tab"}
          onClick={() => setTab("commands")}
        >
          Befehle
        </button>
        <button
          type="button"
          role="tab"
          aria-selected={tab === "files"}
          className={tab === "files" ? "settings-tab active" : "settings-tab"}
          onClick={() => setTab("files")}
        >
          Ablage
        </button>
      </div>

      {/* Beide Inhalte bleiben montiert wäre teurer als nötig: die Ablage würde bei jedem
          Panelwechsel neu laden. Stattdessen bewusst nur der aktive Reiter — der Ladevorgang
          ist ein einzelner SFTP-Aufruf. */}
      {tab === "commands" ? <CommandPanel /> : <FilePanel projectDir={projectDir} />}
    </aside>
  );
}
