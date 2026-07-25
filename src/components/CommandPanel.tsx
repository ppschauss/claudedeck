/**
 * Rechtes, ausklappbares Panel: oben die Regler für Model und Arbeitsstärke, darunter ein
 * Akkordeon aller verfügbaren Befehle (Skills, Agents, Slash-Commands, Connectors) mit eigener
 * Suche.
 *
 * Zwei Entwurfsentscheidungen, die im Verhalten sichtbar sind:
 *
 * 1. **Klick fügt ein, sendet aber kein Enter.** Der Nutzer tippt Argumente und drückt selbst
 *    Return — ein Fehlklick startet damit nie ungewollt einen Skill oder Agent.
 * 2. **Der Katalog wird pro Projektpfad geladen** (`needsReload` in `catalogStore.ts`): beim
 *    Sessionwechsel ändern sich die projektlokalen Einträge, nicht aber die globalen.
 *
 * Die Model-/Effort-Regler wirken doppelt: sie fügen das passende Slash-Kommando in die aktive
 * Session ein *und* schreiben die Wahl als Vorgabe in die `config.json`, aus der
 * `start_project` neue Sessions startet.
 */
import { useCallback, useEffect, useMemo, useState } from "react";
import { describeApiError } from "../lib/apiError";
import { filterCatalog, groupByKind } from "../lib/catalogFilter";
import { EFFORT_LEVELS, effortFromIndex, indexOfEffort } from "../lib/effort";
import {
  getConfig,
  listCommands,
  setConfig,
  writeSession,
  type CommandEntry,
  type CommandKind,
  type Config,
} from "../lib/ipc";
import { needsReload, useCatalogStore } from "../stores/catalogStore";
import { useSessionStore } from "../stores/sessionStore";
import { useToastStore } from "../stores/toastStore";

const KIND_TITLES: Record<CommandKind, string> = {
  skill: "Skills",
  agent: "Agents",
  command: "Befehle",
};

/** Agents werden nicht als `/name` aufgerufen, Skills und Slash-Commands schon. */
function insertionText(entry: CommandEntry): string {
  return entry.kind === "agent" ? `${entry.name} ` : `/${entry.name} `;
}

export function CommandPanel() {
  const open = useCatalogStore((s) => s.open);
  const entries = useCatalogStore((s) => s.entries);
  const connectors = useCatalogStore((s) => s.connectors);
  const loading = useCatalogStore((s) => s.loading);
  const error = useCatalogStore((s) => s.error);
  const query = useCatalogStore((s) => s.query);

  const activeSessionId = useSessionStore((s) => s.activeSessionId);
  const running = useSessionStore((s) => s.running);
  const openSessions = useSessionStore((s) => s.openSessions);

  const [config, setLocalConfig] = useState<Config | null>(null);
  const [collapsed, setCollapsed] = useState<Set<string>>(new Set());

  // Das Arbeitsverzeichnis der aktiven Session steht in der `running`-Liste (`SessionInfo.cwd`) —
  // deshalb braucht `list_commands` keinen eigenen `tmux display -p`-Roundtrip.
  const projectDir = useMemo(() => {
    if (!activeSessionId) return null;
    const name = openSessions.get(activeSessionId)?.name;
    if (!name) return null;
    return running.find((s) => s.name === name)?.cwd ?? null;
  }, [activeSessionId, openSessions, running]);

  const refresh = useCallback(async (dir: string | null) => {
    useCatalogStore.getState().loadStarted();
    try {
      useCatalogStore.getState().loaded(await listCommands(dir), dir);
    } catch (err) {
      useCatalogStore.getState().failed(describeApiError(err));
    }
  }, []);

  // Erst laden, wenn das Panel offen ist — ein zugeklapptes Panel soll keine Execs auslösen.
  useEffect(() => {
    if (!open) return;
    if (needsReload(useCatalogStore.getState(), projectDir)) void refresh(projectDir);
  }, [open, projectDir, refresh]);

  useEffect(() => {
    if (!open || config) return;
    void getConfig()
      .then(setLocalConfig)
      .catch(() => setLocalConfig(null));
  }, [open, config]);

  const grouped = useMemo(
    () => groupByKind(filterCatalog(entries, query)),
    [entries, query],
  );
  const shownConnectors = useMemo(
    () => filterCatalog(connectors.map((c) => ({ ...c, description: c.url })), query),
    [connectors, query],
  );

  function toggleGroup(key: string) {
    setCollapsed((prev) => {
      const next = new Set(prev);
      if (next.has(key)) {
        next.delete(key);
      } else {
        next.add(key);
      }
      return next;
    });
  }

  /** Schreibt Text in die aktive Session, ohne Enter zu senden. */
  function insert(text: string) {
    if (!activeSessionId) return;
    void writeSession(activeSessionId, new TextEncoder().encode(text));
  }

  /** Persistiert eine Regler-Änderung und spiegelt sie in die laufende Session. */
  async function applyDefault(patch: Partial<Config["defaults"]>, slashCommand: string) {
    insert(slashCommand);
    if (!config) return;
    const next: Config = { ...config, defaults: { ...config.defaults, ...patch } };
    setLocalConfig(next);
    try {
      await setConfig(next);
    } catch (err) {
      useToastStore.getState().push(describeApiError(err));
    }
  }

  if (!open) return null;

  const effortIndex = indexOfEffort(config?.defaults.effort);
  const models = config?.available_models ?? [];

  return (
    <aside className="command-panel">
      <div className="command-panel-head">
        <div className="command-controls">
          <label className="command-control">
            <span>Model</span>
            <select
              value={config?.defaults.model ?? ""}
              disabled={!config}
              onChange={(e) =>
                void applyDefault(
                  { model: e.target.value || null },
                  e.target.value ? `/model ${e.target.value} ` : "/model ",
                )
              }
            >
              <option value="">(Vorgabe)</option>
              {models.map((m) => (
                <option key={m} value={m}>
                  {m}
                </option>
              ))}
            </select>
          </label>

          <label className="command-control">
            <span>
              Stärke: <strong>{EFFORT_LEVELS[effortIndex]}</strong>
            </span>
            <input
              type="range"
              min={0}
              max={EFFORT_LEVELS.length - 1}
              step={1}
              value={effortIndex}
              disabled={!config}
              aria-label="Arbeitsstärke"
              onChange={(e) => {
                const level = effortFromIndex(Number(e.target.value));
                void applyDefault({ effort: level }, `/effort ${level} `);
              }}
            />
          </label>
        </div>

        <div className="command-panel-search">
          <input
            type="search"
            placeholder="Befehle durchsuchen…"
            aria-label="Befehle durchsuchen"
            value={query}
            onChange={(e) => useCatalogStore.getState().queryChanged(e.target.value)}
          />
          <button
            type="button"
            className="command-refresh"
            aria-label="Katalog neu laden"
            title="Katalog neu laden"
            disabled={loading}
            onClick={() => void refresh(projectDir)}
          >
            ⟳
          </button>
        </div>
      </div>

      {error && <p className="error-text sidebar-error">{error}</p>}
      {loading && <p className="sidebar-empty">Lädt …</p>}
      {!activeSessionId && !loading && (
        <p className="sidebar-empty">Keine Session aktiv — Einfügen ist deaktiviert.</p>
      )}

      {(Object.keys(KIND_TITLES) as CommandKind[]).map((kind) => (
        <Accordion
          key={kind}
          title={KIND_TITLES[kind]}
          count={grouped[kind].length}
          collapsed={collapsed.has(kind)}
          onToggle={() => toggleGroup(kind)}
        >
          {grouped[kind].map((entry) => (
            <li key={`${entry.scope}-${entry.name}`}>
              <button
                type="button"
                className="command-item"
                disabled={!activeSessionId}
                title={entry.description || undefined}
                onClick={() => insert(insertionText(entry))}
              >
                <span className="command-name">
                  {entry.scope === "project" && (
                    <span className="command-scope" title="Nur in dieser Session verfügbar">
                      ●
                    </span>
                  )}
                  {insertionText(entry).trim()}
                </span>
                {entry.description && (
                  <span className="command-desc">{entry.description}</span>
                )}
              </button>
            </li>
          ))}
        </Accordion>
      ))}

      <Accordion
        title="Connectors"
        count={shownConnectors.length}
        collapsed={collapsed.has("connectors")}
        onToggle={() => toggleGroup("connectors")}
      >
        {shownConnectors.map((c) => (
          <li key={c.name} className="connector-item">
            <span className={c.connected ? "dot dot-filled" : "dot dot-lost"} aria-hidden="true">
              {c.connected ? "●" : "○"}
            </span>
            <span className="command-name">{c.name}</span>
            <span className="command-desc">{c.status}</span>
          </li>
        ))}
      </Accordion>
    </aside>
  );
}

function Accordion({
  title,
  count,
  collapsed,
  onToggle,
  children,
}: {
  title: string;
  count: number;
  collapsed: boolean;
  onToggle: () => void;
  children: React.ReactNode;
}) {
  return (
    <div className="command-group">
      <button
        type="button"
        className="command-group-head"
        aria-expanded={!collapsed}
        onClick={onToggle}
      >
        <span aria-hidden="true">{collapsed ? "▸" : "▾"}</span>
        <span>{title}</span>
        <span className="command-count">{count}</span>
      </button>
      {!collapsed &&
        (count === 0 ? <p className="sidebar-empty">–</p> : <ul>{children}</ul>)}
    </div>
  );
}
