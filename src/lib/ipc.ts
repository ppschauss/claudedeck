/**
 * Einzige Datei, die `invoke`/`listen`/`Channel` importiert (Global Constraint). Alles andere
 * im Frontend spricht mit dem Rust-Backend ausschließlich über die hier typisierten Wrapper.
 *
 * ## Command-Arg-Mapping (Rust ↔ Tauri-IPC ↔ dieser Wrapper)
 *
 * Tauri übernimmt den `#[tauri::command]`-Funktionsnamen 1:1 (keine Case-Konvertierung) als
 * `invoke()`-Command-Key — die Wrapper hier rufen also z.B. `invoke("open_session", …)`, NICHT
 * `invoke("openSession", …)`. Die *Argumentnamen* dagegen werden von Tauri per Default
 * snake_case→camelCase konvertiert (kein `#[tauri::command(rename_all = "snake_case")]` in
 * `connection.rs`/`sessions.rs` gesetzt) — ein Rust-Parameter `session_id: String` erwartet vom
 * Frontend also den Objekt-Key `sessionId`.
 *
 * | Rust-Command (src-tauri)             | invoke()-Key              | Rust-Args (snake_case)                          | JS-Args (camelCase, wie gesendet)                    |
 * |---------------------------------------|----------------------------|--------------------------------------------------|-------------------------------------------------------|
 * | `connect`                              | `connect`                  | `password: Option<String>`                       | `{ password }`                                         |
 * | `accept_hostkey_and_connect`           | `accept_hostkey_and_connect` | `password: Option<String>`                      | `{ password }`                                         |
 * | `disconnect`                           | `disconnect`                | –                                                | – (`Result<(), ()>`, löst immer auf)                   |
 * | `get_config`                           | `get_config`                 | –                                                | –                                                       |
 * | `set_config`                           | `set_config`                 | `config: Config`                                  | `{ config }`                                           |
 * | `save_secret`                          | `save_secret`                | `kind: SecretArgKind, value: String`              | `{ kind, value }`                                       |
 * | `has_secret`                           | `has_secret`                 | `kind: SecretArgKind`                             | `{ kind }`                                              |
 * | `list_sessions`                        | `list_sessions`              | –                                                | –                                                       |
 * | `open_session`                         | `open_session`               | `name, cols, rows, on_output: Channel<OutputChunk>` | `{ name, cols, rows, onOutput }`                      |
 * | `start_project`                        | `start_project`              | `path, cols, rows, on_output: Channel<OutputChunk>` | `{ path, cols, rows, onOutput }`                      |
 * | `write_session`                        | `write_session`              | `session_id: String, data_b64: String`            | `{ sessionId, dataB64 }`                                |
 * | `resize_session`                       | `resize_session`             | `session_id: String, cols: u16, rows: u16`        | `{ sessionId, cols, rows }`                             |
 * | `close_session`                        | `close_session`              | `session_id: String`                              | `{ sessionId }` (`Result<(), ()>`, löst immer auf)      |
 * | `kill_session`                         | `kill_session`               | `name: String`                                    | `{ name }`                                              |
 *
 * Events (Rust `app.emit(name, payload)` → Frontend `listen(name, …)`), Payload-Felder sind
 * bereits camelCase (`#[serde(rename_all = "camelCase")]` auf den jeweiligen Event-Structs):
 * `connection-state`, `pty-exit`, `sessions-changed`, `session-reattached` (Task 6, Auflage C —
 * erweitert den im Plan dokumentierten Contract um genau dieses eine Event; siehe
 * `reconnect_supervisor.rs`/`commands/sessions.rs::reattach_lost_sessions`).
 *
 * ## Sonderfall `Config`/`Profile`/`NotifySettings`/`AuthMethod` (`get_config`/`set_config`)
 *
 * Anders als alle Command-DTOs in `commands/*.rs` tragen `Config`/`Profile`/`NotifySettings` in
 * `claudedeck_core::config` KEIN `#[serde(rename_all = "camelCase")]` — sie serialisieren daher
 * mit ihren rohen Rust-Feldnamen: `scan_paths`, `key_path`, `silence_ms` bleiben snake_case
 * (nicht `scanPaths`/`keyPath`/`silenceMs`!). `AuthMethod` ist ein einfaches Enum ohne
 * `rename_all` und serialisiert daher als PascalCase-String (`"Key"`/`"Password"`, nicht
 * `"key"`/`"password"` wie bei `SecretArgKind`, das im Gegensatz dazu `rename_all = "camelCase"`
 * hat). Die Typen unten spiegeln das exakt — bewusst NICHT camelCase-normalisiert, weil das den
 * Round-Trip durch `set_config` brechen würde.
 *
 * ## Base64
 *
 * Rust nutzt `data_encoding::BASE64` (Standard-Alphabet, MIT Padding, RFC 4648 §4). Die
 * Helfer unten benutzen bewusst `btoa`/`atob` mit manueller Bytes↔Binärstring-Konvertierung
 * (`String.fromCharCode`/`charCodeAt`) statt `TextEncoder`/`TextDecoder` — Letztere sind für
 * Unicode-Text gedacht und würden beliebige Binärbytes (PTY-Output ist kein garantiert gültiges
 * UTF-8, z.B. mitten in einer Multibyte-Sequenz getrennt) verstümmeln.
 */
import { Channel, invoke } from "@tauri-apps/api/core";
import { listen, type UnlistenFn } from "@tauri-apps/api/event";
import type { TerminalDisplay } from "./terminalTheme";

// ---------------------------------------------------------------------------------------------
// Base64 (Uint8Array ↔ Standard-Base64 mit Padding, kompatibel zu Rusts `data-encoding::BASE64`)
// ---------------------------------------------------------------------------------------------

/** `btoa`/`String.fromCharCode` verarbeiten pro Aufruf begrenzt viele Argumente/Zeichen sauber
 * — in Chunks arbeiten, damit auch große PTY-Batches (mehrere 10 KiB) nicht an Engine-Limits
 * für Funktionsargumente scheitern. */
const CHUNK_SIZE = 0x8000;

export function bytesToB64(bytes: Uint8Array): string {
  let binary = "";
  for (let i = 0; i < bytes.length; i += CHUNK_SIZE) {
    const chunk = bytes.subarray(i, i + CHUNK_SIZE);
    binary += String.fromCharCode(...chunk);
  }
  return btoa(binary);
}

export function b64ToBytes(b64: string): Uint8Array {
  const binary = atob(b64);
  const bytes = new Uint8Array(binary.length);
  for (let i = 0; i < binary.length; i++) {
    bytes[i] = binary.charCodeAt(i);
  }
  return bytes;
}

// ---------------------------------------------------------------------------------------------
// Typen — spiegeln die Serde-Ausgabe der jeweiligen Rust-Typen (siehe Mapping-Tabelle oben)
// ---------------------------------------------------------------------------------------------

export type ApiError =
  | { kind: "authFailed"; message: string }
  | { kind: "hostkeyUnknown"; message: string; fingerprint: string }
  | { kind: "hostkeyChanged"; message: string; fingerprint: string }
  | { kind: "notConnected"; message: string }
  | { kind: "tmuxMissing"; message: string }
  | { kind: "ssh"; message: string }
  | { kind: "io"; message: string };

/** `claudedeck_core::config::AuthMethod` — kein `rename_all`, daher PascalCase-Varianten. */
export type AuthMethod = "Key" | "Password";

/** `claudedeck_core::config::Profile` — kein `rename_all`, Feldnamen bleiben snake_case. */
export interface Profile {
  host: string;
  port: number;
  user: string;
  auth: AuthMethod;
  key_path: string | null;
}

/** `claudedeck_core::config::NotifySettings` — kein `rename_all`. */
export interface NotifySettings {
  enabled: boolean;
  silence_ms: number;
}

/** `claudedeck_core::config::SessionDefaults` — `null` heißt „Flag weglassen". */
export interface SessionDefaults {
  model: string | null;
  effort: string | null;
}

/**
 * `claudedeck_core::config::NamedProfile` — ein benanntes Verbindungsziel.
 *
 * `id` ist der Schlüssel, unter dem Passwort und Passphrase im Anmeldedaten-Speicher liegen,
 * und darf sich deshalb nie ändern — `name` schon.
 */
export interface NamedProfile {
  id: string;
  name: string;
  host: string;
  port: number;
  user: string;
  auth: AuthMethod;
  key_path: string | null;
}

/** `claudedeck_core::config::Config` — kein `rename_all`. */
export interface Config {
  /** Veraltet: Migrationsquelle für `profiles`. Nicht mehr zum Verbinden benutzt. */
  profile: Profile;
  /** Nie leer — das Backend migriert beim Laden. */
  profiles: NamedProfile[];
  active_profile: string | null;
  auto_connect: boolean;
  scan_paths: string[];
  favorites: string[];
  notifications: NotifySettings;
  defaults: SessionDefaults;
  available_models: string[];
  /** `TerminalSettings` — die EINZIGE Teilstruktur mit `rename_all = "camelCase"`, damit sie
   * ohne Umbenennung als `TerminalDisplay` (terminalTheme.ts) durchgereicht werden kann. */
  terminal: TerminalDisplay;
}

/** `save_secret`/`has_secret`-Argument — `SecretArgKind` hat `rename_all = "camelCase"`. */
export type SecretKind = "password" | "keyPassphrase";

/** `commands::sessions::SessionInfoDto`. */
export interface SessionInfo {
  name: string;
  kind: "claude" | "shell";
  cwd: string;
  attached: boolean;
  created: number;
  managed: boolean;
}

/** `commands::sessions::Project` (noch nicht angehängtes scan_paths-Verzeichnis). */
export interface Project {
  path: string;
  name: string;
}

/** `catalog::CommandKind` (serde `rename_all = "camelCase"`). */
export type CommandKind = "skill" | "agent" | "command";

/** `catalog::CommandScope` — `project` = nur in der aktiven Session verfügbar. */
export type CommandScope = "global" | "project";

/** `catalog::CommandEntry`. `name` ist ohne führenden Schrägstrich gespeichert. */
export interface CommandEntry {
  kind: CommandKind;
  name: string;
  description: string;
  scope: CommandScope;
}

/** `catalog::Connector` aus `claude mcp list`. */
export interface Connector {
  name: string;
  url: string;
  status: string;
  connected: boolean;
}

/** `catalog::Catalog`. */
export interface Catalog {
  entries: CommandEntry[];
  connectors: Connector[];
}

/** `commands::sessions::SessionList`. */
export interface SessionList {
  running: SessionInfo[];
  startable: Project[];
}

/** `commands::sessions::StartResult`. */
export interface StartResult {
  sessionId: string;
  sessionName: string;
}

/** `commands::sessions::OutputChunk` — Channel-Payload, base64-kodierte PTY-Bytes. */
export interface OutputChunk {
  dataB64: string;
}

/** Payload des `connection-state`-Events. `attempt`/`nextRetryInS` werden erst vom
 * Reconnect-Supervisor (Task 6) befüllt — bis dahin liefert das Backend nur `state`. */
export interface ConnectionStateEvent {
  state: "disconnected" | "connecting" | "connected" | "reconnecting" | "failed";
  attempt?: number;
  nextRetryInS?: number;
}

/** Payload des `pty-exit`-Events. */
export interface PtyExitEvent {
  sessionId: string;
  reason: "exited" | "connectionLost";
}

/** Payload des `session-reattached`-Events (Task 6). Backend hat serverseitig ein neues PTY auf
 * denselben Channel gelegt — Frontend muss nur noch `sessionStore.reattached(sessionId)` rufen,
 * TermPool/Channel-Callback laufen unverändert weiter (kein erneutes `open_session`). */
export interface SessionReattachedEvent {
  sessionId: string;
}

// ---------------------------------------------------------------------------------------------
// Commands
// ---------------------------------------------------------------------------------------------

export function connect(password?: string): Promise<void> {
  return invoke("connect", { password: password ?? null });
}

export function acceptHostkeyAndConnect(password?: string): Promise<void> {
  return invoke("accept_hostkey_and_connect", { password: password ?? null });
}

export function disconnect(): Promise<void> {
  return invoke("disconnect");
}

export function getConfig(): Promise<Config> {
  return invoke("get_config");
}

export function setConfig(config: Config): Promise<void> {
  return invoke("set_config", { config });
}

/** `profileId` weglassen heißt „aktives Profil" — jedes Profil hat sein eigenes Secret. */
export function saveSecret(
  kind: SecretKind,
  value: string,
  profileId?: string,
): Promise<void> {
  return invoke("save_secret", { kind, value, profileId: profileId ?? null });
}

export function hasSecret(kind: SecretKind, profileId?: string): Promise<boolean> {
  return invoke("has_secret", { kind, profileId: profileId ?? null });
}

export function deleteSecret(kind: SecretKind, profileId?: string): Promise<void> {
  return invoke("delete_secret", { kind, profileId: profileId ?? null });
}

export function listSessions(): Promise<SessionList> {
  return invoke("list_sessions");
}

/**
 * Liest den Befehls-Katalog vom Server. `projectDir` ist das Arbeitsverzeichnis der aktiven
 * Session (`SessionInfo.cwd`) — ohne offene Session `null`, dann kommen nur globale Einträge.
 */
export function listCommands(projectDir: string | null): Promise<Catalog> {
  return invoke("list_commands", { projectDir });
}

/** Öffnet/hängt eine tmux-Session an. `onOutput` wird für jeden (gebatchten,
 * base64-kodierten) PTY-Chunk aufgerufen, solange die Session offen ist. */
export function openSession(
  name: string,
  cols: number,
  rows: number,
  onOutput: (chunk: OutputChunk) => void,
): Promise<string> {
  const channel = new Channel<OutputChunk>();
  channel.onmessage = onOutput;
  return invoke("open_session", { name, cols, rows, onOutput: channel });
}

export function startProject(
  path: string,
  cols: number,
  rows: number,
  onOutput: (chunk: OutputChunk) => void,
): Promise<StartResult> {
  const channel = new Channel<OutputChunk>();
  channel.onmessage = onOutput;
  return invoke("start_project", { path, cols, rows, onOutput: channel });
}

/** Nimmt rohe Bytes entgegen (nicht bereits base64) — kodiert intern, damit Aufrufer
 * (`termPool.ts`) nie selbst mit Base64 hantieren müssen. */
export function writeSession(sessionId: string, bytes: Uint8Array): Promise<void> {
  return invoke("write_session", { sessionId, dataB64: bytesToB64(bytes) });
}

export function resizeSession(sessionId: string, cols: number, rows: number): Promise<void> {
  return invoke("resize_session", { sessionId, cols, rows });
}

export function closeSession(sessionId: string): Promise<void> {
  return invoke("close_session", { sessionId });
}

export function killSession(name: string): Promise<void> {
  return invoke("kill_session", { name });
}

// ---------------------------------------------------------------------------------------------
// Events
// ---------------------------------------------------------------------------------------------

export function onConnectionState(
  handler: (event: ConnectionStateEvent) => void,
): Promise<UnlistenFn> {
  return listen<ConnectionStateEvent>("connection-state", (e) => handler(e.payload));
}

export function onPtyExit(handler: (event: PtyExitEvent) => void): Promise<UnlistenFn> {
  return listen<PtyExitEvent>("pty-exit", (e) => handler(e.payload));
}

export function onSessionsChanged(handler: () => void): Promise<UnlistenFn> {
  return listen<void>("sessions-changed", () => handler());
}

export function onSessionReattached(
  handler: (event: SessionReattachedEvent) => void,
): Promise<UnlistenFn> {
  return listen<SessionReattachedEvent>("session-reattached", (e) => handler(e.payload));
}
