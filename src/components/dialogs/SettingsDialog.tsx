/**
 * Zentrale Einstellungen. Erreichbar über das Zahnrad in der Statusleiste und `Strg+,`.
 *
 * Ein Dialog ist hier ausnahmsweise das richtige Mittel — es geht um selten geänderte,
 * formularartige Konfiguration, nicht um eine Aktion im Arbeitsfluss.
 *
 * Änderungen wirken **sofort** (der Config-Store treibt `applyTerminalDisplay` in `App.tsx`) und
 * werden nebenher gespeichert. Kein „Übernehmen"-Knopf: bei Aussehen will man das Ergebnis
 * sehen, nicht bestätigen.
 *
 * Der Reiter „Profile" kommt mit den Verbindungsprofilen (M8-4); bis dahin bleiben Host und
 * Benutzer in der `config.json`.
 */
import { useEffect, useState } from "react";
import { describeApiError } from "../../lib/apiError";
import { deleteSecret, setConfig, type Config, type NamedProfile } from "../../lib/ipc";
import { EFFORT_LEVELS, effortFromIndex, indexOfEffort } from "../../lib/effort";
import { FONT_CHOICES, TERMINAL_THEMES, clampFontSize } from "../../lib/terminalTheme";
import { useConfigStore } from "../../stores/configStore";
import { useToastStore } from "../../stores/toastStore";

type Tab = "profile" | "terminal" | "sessions" | "hinweise";

const TABS: { id: Tab; label: string }[] = [
  { id: "profile", label: "Profile" },
  { id: "terminal", label: "Terminal" },
  { id: "sessions", label: "Sessions" },
  { id: "hinweise", label: "Hinweise" },
];

/**
 * Eine stabile, kollisionsfreie ID für ein neues Profil. Sie wandert in den
 * Anmeldedaten-Speicher und darf sich danach nie ändern — deshalb aus dem Zeitstempel abgeleitet
 * und nicht aus dem Namen, den man jederzeit umbenennen können soll.
 */
function newProfileId(existing: string[]): string {
  let id = `p${Date.now().toString(36)}`;
  while (existing.includes(id)) id += "x";
  return id;
}

export function SettingsDialog({ onClose }: { onClose: () => void }) {
  const config = useConfigStore((s) => s.config);
  const [tab, setTab] = useState<Tab>("profile");
  const [editing, setEditing] = useState<string | null>(null);

  // Esc schließt — erwartetes Verhalten für einen Dialog; ohne das bliebe nur die Maus.
  useEffect(() => {
    function onKey(e: KeyboardEvent) {
      if (e.key === "Escape") {
        e.stopPropagation();
        onClose();
      }
    }
    window.addEventListener("keydown", onKey);
    return () => window.removeEventListener("keydown", onKey);
  }, [onClose]);

  /** Übernimmt eine Teiländerung sofort in den Store und speichert sie nebenher. */
  function update(patch: Partial<Config>) {
    if (!config) return;
    const next = { ...config, ...patch };
    useConfigStore.getState().loaded(next);
    void setConfig(next).catch((err) => useToastStore.getState().push(describeApiError(err)));
  }

  function updateTerminal(patch: Partial<Config["terminal"]>) {
    if (!config) return;
    update({ terminal: { ...config.terminal, ...patch } });
  }

  function updateProfile(id: string, patch: Partial<NamedProfile>) {
    if (!config) return;
    update({
      profiles: config.profiles.map((p) => (p.id === id ? { ...p, ...patch } : p)),
    });
  }

  function addProfile() {
    if (!config) return;
    const id = newProfileId(config.profiles.map((p) => p.id));
    const fresh: NamedProfile = {
      id,
      name: "Neue Verbindung",
      host: "",
      port: 22,
      user: "root",
      auth: "Password",
      key_path: null,
    };
    update({ profiles: [...config.profiles, fresh] });
    setEditing(id);
  }

  /**
   * Löscht Profil **und** sein Secret. Ohne das zweite bliebe ein Passwort verwaist im
   * Anmeldedaten-Speicher zurück, das niemand mehr sieht und niemand mehr entfernt.
   */
  async function removeProfile(id: string, name: string) {
    if (!config) return;
    if (!window.confirm(`Profil „${name}" wirklich löschen?`)) return;

    const profiles = config.profiles.filter((p) => p.id !== id);
    update({
      profiles,
      active_profile: config.active_profile === id ? profiles[0].id : config.active_profile,
    });
    setEditing(null);
    try {
      await deleteSecret("password", id);
      await deleteSecret("keyPassphrase", id);
    } catch (err) {
      useToastStore.getState().push(describeApiError(err));
    }
  }

  async function forgetSecret(id: string, name: string) {
    try {
      await deleteSecret("password", id);
      useToastStore.getState().push(`Passwort für „${name}" entfernt.`);
    } catch (err) {
      useToastStore.getState().push(describeApiError(err));
    }
  }

  return (
    <div className="dialog-backdrop" onMouseDown={onClose}>
      <div
        className="dialog settings-dialog"
        role="dialog"
        aria-modal="true"
        aria-label="Einstellungen"
        onMouseDown={(e) => e.stopPropagation()}
      >
        <div className="settings-head">
          <div className="settings-tabs" role="tablist">
            {TABS.map((t) => (
              <button
                key={t.id}
                type="button"
                role="tab"
                aria-selected={tab === t.id}
                className={tab === t.id ? "settings-tab active" : "settings-tab"}
                onClick={() => setTab(t.id)}
              >
                {t.label}
              </button>
            ))}
          </div>
          <button type="button" className="settings-close" aria-label="Schließen" onClick={onClose}>
            ×
          </button>
        </div>

        {!config && <p className="sidebar-empty">Einstellungen werden geladen …</p>}

        {config && tab === "profile" && (
          <div className="settings-body">
            <ul className="profile-list">
              {config.profiles.map((p) => {
                const isActive = (config.active_profile ?? config.profiles[0].id) === p.id;
                const isEditing = editing === p.id;
                return (
                  <li key={p.id} className={isActive ? "profile-row active" : "profile-row"}>
                    <div className="profile-head">
                      <button
                        type="button"
                        className="profile-pick"
                        aria-pressed={isActive}
                        onClick={() => update({ active_profile: p.id })}
                      >
                        <span className={isActive ? "dot dot-waiting" : "dot dot-idle"}>
                          {isActive ? "✓" : "○"}
                        </span>
                        <span className="profile-name">{p.name}</span>
                        <span className="profile-target">
                          {p.user}@{p.host}
                          {p.port === 22 ? "" : `:${p.port}`}
                        </span>
                      </button>
                      <button
                        type="button"
                        className="session-menu-trigger"
                        aria-label={isEditing ? "Bearbeiten schließen" : `${p.name} bearbeiten`}
                        onClick={() => setEditing(isEditing ? null : p.id)}
                      >
                        {isEditing ? "▾" : "✎"}
                      </button>
                    </div>

                    {isEditing && (
                      <div className="profile-edit">
                        <Field label="Name">
                          <input
                            value={p.name}
                            onChange={(e) => updateProfile(p.id, { name: e.target.value })}
                          />
                        </Field>
                        <Field label="Host">
                          <input
                            value={p.host}
                            onChange={(e) => updateProfile(p.id, { host: e.target.value })}
                          />
                        </Field>
                        <div className="profile-pair">
                          <Field label="Benutzer">
                            <input
                              value={p.user}
                              onChange={(e) => updateProfile(p.id, { user: e.target.value })}
                            />
                          </Field>
                          <Field label="Port">
                            <input
                              type="number"
                              min={1}
                              max={65535}
                              value={p.port}
                              onChange={(e) =>
                                updateProfile(p.id, { port: Number(e.target.value) || 22 })
                              }
                            />
                          </Field>
                        </div>
                        <Field label="Anmeldung">
                          <select
                            value={p.auth}
                            onChange={(e) =>
                              updateProfile(p.id, { auth: e.target.value as typeof p.auth })
                            }
                          >
                            <option value="Password">Passwort</option>
                            <option value="Key">SSH-Key</option>
                          </select>
                        </Field>
                        {p.auth === "Key" && (
                          <Field label="Pfad zum privaten Schlüssel">
                            <input
                              value={p.key_path ?? ""}
                              placeholder="/root/.ssh/id_ed25519"
                              onChange={(e) =>
                                updateProfile(p.id, { key_path: e.target.value || null })
                              }
                            />
                          </Field>
                        )}

                        <div className="profile-actions">
                          <button
                            type="button"
                            onClick={() => void forgetSecret(p.id, p.name)}
                          >
                            Passwort vergessen
                          </button>
                          <button
                            type="button"
                            className="danger"
                            disabled={config.profiles.length === 1}
                            title={
                              config.profiles.length === 1
                                ? "Das letzte Profil lässt sich nicht löschen"
                                : undefined
                            }
                            onClick={() => void removeProfile(p.id, p.name)}
                          >
                            Profil löschen
                          </button>
                        </div>
                      </div>
                    )}
                  </li>
                );
              })}
            </ul>

            <button type="button" className="profile-add" onClick={addProfile}>
              + Neues Profil
            </button>

            <label className="settings-check">
              <input
                type="checkbox"
                checked={config.auto_connect}
                onChange={(e) => update({ auto_connect: e.target.checked })}
              />
              Beim Start automatisch verbinden
            </label>
            <p className="settings-hint">
              Wirkt nur, wenn für das gewählte Profil ein Passwort gespeichert ist.
            </p>
          </div>
        )}

        {config && tab === "terminal" && (
          <div className="settings-body">
            <Field label="Farbschema">
              <select
                value={config.terminal.themeId}
                onChange={(e) => updateTerminal({ themeId: e.target.value })}
              >
                {TERMINAL_THEMES.map((t) => (
                  <option key={t.id} value={t.id}>
                    {t.name}
                  </option>
                ))}
              </select>
            </Field>

            <Field label="Schriftart">
              <select
                value={config.terminal.fontFamily}
                onChange={(e) => updateTerminal({ fontFamily: e.target.value })}
              >
                {FONT_CHOICES.map((f) => (
                  <option key={f.id} value={f.stack}>
                    {f.name}
                  </option>
                ))}
              </select>
            </Field>

            <Field label={`Schriftgröße — ${config.terminal.fontSize} px`}>
              <input
                type="range"
                min={8}
                max={32}
                step={1}
                value={config.terminal.fontSize}
                onChange={(e) => updateTerminal({ fontSize: clampFontSize(Number(e.target.value)) })}
              />
              <span className="settings-hint">auch per Strg + / Strg − / Strg 0</span>
            </Field>

            <Field label={`Zeilenhöhe — ${config.terminal.lineHeight.toFixed(2)}`}>
              <input
                type="range"
                min={1}
                max={2}
                step={0.05}
                value={config.terminal.lineHeight}
                onChange={(e) => updateTerminal({ lineHeight: Number(e.target.value) })}
              />
            </Field>

            <Field label="Cursor">
              <select
                value={config.terminal.cursorStyle}
                onChange={(e) =>
                  updateTerminal({
                    cursorStyle: e.target.value as Config["terminal"]["cursorStyle"],
                  })
                }
              >
                <option value="bar">Balken</option>
                <option value="block">Block</option>
                <option value="underline">Unterstrich</option>
              </select>
            </Field>

            <label className="settings-check">
              <input
                type="checkbox"
                checked={config.terminal.cursorBlink}
                onChange={(e) => updateTerminal({ cursorBlink: e.target.checked })}
              />
              Cursor blinkt
            </label>

            <Field label="Scrollback (Zeilen)">
              <input
                type="number"
                min={1000}
                max={200000}
                step={1000}
                value={config.terminal.scrollback}
                onChange={(e) =>
                  updateTerminal({ scrollback: Math.max(1000, Number(e.target.value) || 1000) })
                }
              />
            </Field>
          </div>
        )}

        {config && tab === "sessions" && (
          <div className="settings-body">
            <Field label="Model für neue Sessions">
              <select
                value={config.defaults.model ?? ""}
                onChange={(e) =>
                  update({
                    defaults: { ...config.defaults, model: e.target.value || null },
                  })
                }
              >
                <option value="">(Claude-Vorgabe)</option>
                {config.available_models.map((m) => (
                  <option key={m} value={m}>
                    {m}
                  </option>
                ))}
              </select>
            </Field>

            <Field
              label={`Arbeitsstärke — ${EFFORT_LEVELS[indexOfEffort(config.defaults.effort)]}`}
            >
              <input
                type="range"
                min={0}
                max={EFFORT_LEVELS.length - 1}
                step={1}
                value={indexOfEffort(config.defaults.effort)}
                onChange={(e) =>
                  update({
                    defaults: {
                      ...config.defaults,
                      effort: effortFromIndex(Number(e.target.value)),
                    },
                  })
                }
              />
            </Field>

            <Field label="Projektordner (einer pro Zeile)">
              <textarea
                rows={4}
                value={config.scan_paths.join("\n")}
                onChange={(e) =>
                  update({
                    scan_paths: e.target.value
                      .split("\n")
                      .map((p) => p.trim())
                      .filter(Boolean),
                  })
                }
              />
              <span className="settings-hint">
                Diese Verzeichnisse durchsucht „Startbar" nach Projekten.
              </span>
            </Field>

            <Field label="Projekt-Merkmale (eines pro Zeile)">
              <textarea
                rows={3}
                value={config.project_markers.join("\n")}
                onChange={(e) =>
                  update({
                    project_markers: e.target.value
                      .split("\n")
                      .map((m) => m.trim())
                      .filter(Boolean),
                  })
                }
              />
              <span className="settings-hint">
                Ein Ordner gilt nur als Projekt, wenn er eines davon enthält — sonst erscheint
                jedes Docker-Datenverzeichnis unter „Startbar". Leer lassen hebt den Filter auf.
              </span>
            </Field>
          </div>
        )}

        {config && tab === "hinweise" && (
          <div className="settings-body">
            <label className="settings-check">
              <input
                type="checkbox"
                checked={config.notifications.enabled}
                onChange={(e) =>
                  update({
                    notifications: { ...config.notifications, enabled: e.target.checked },
                  })
                }
              />
              Benachrichtigen, wenn eine Hintergrund-Session fertig ist
            </label>
            <p className="settings-hint">
              Eine Session gilt als fertig, wenn {config.notifications.silence_ms} ms lang keine
              Ausgabe mehr kam — dieselbe Regel, aus der auch der grüne Haken in der Sidebar
              entsteht.
            </p>
          </div>
        )}
      </div>
    </div>
  );
}

function Field({ label, children }: { label: string; children: React.ReactNode }) {
  return (
    <label className="settings-field">
      <span>{label}</span>
      {children}
    </label>
  );
}
