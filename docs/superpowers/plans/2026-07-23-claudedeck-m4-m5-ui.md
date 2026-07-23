# ClaudeDeck M4+M5 (Tauri-IPC-Brücke + UI) Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Die App wird benutzbar: Verbinden per Dialog, Sessionliste in der Sidebar, Sessions als xterm.js-Terminals öffnen (mehrere parallel, sofortiges Umschalten), neue Projekte starten, Badges + Windows-Notifications, Scrollback-Suche.

**Architecture:** src-tauri wird dünner Adapter über claudedeck-core: ein `AppState` (tokio::Mutex) hält die eine SshConnection + eine Session-Map (sessionId → PtyHandle-Writer + Kill-Switch). PTY-Output fließt pro Session über einen `tauri::ipc::Channel` (gebatcht ~10 ms/32 KiB) direkt in `term.write()`. Das Frontend hält alle xterm-Instanzen in einem TermPool (nie disposen beim Umschalten); Sidebar = Session-Switcher, kein Tab-Bar. Alle UI-Logik, die Entscheidungen trifft (Badges, Notification-Heuristik, Store-Reducer), ist pure TypeScript mit vitest-Tests.

**Tech Stack:** Tauri 2 (ipc::Channel, Events, plugin-notification), React 19 + zustand 5, @xterm/xterm 6 (+fit, +search; WebGL erst bei Bedarf), claudedeck-core (M2/M3).

## Global Constraints

- Spec `docs/superpowers/specs/2026-07-21-claudedeck-design.md`; Referenz `…/2026-07-22-claudedeck-masterplan-referenz.md`; M4-Notizen am Ende von `.superpowers/sdd/progress.md`
- Repo `/mnt/cache/appdata/claudedeck`, Rust nur über `./dev.sh cargo …`, NIEMALS unter /root
- src-tauri kann auf Linux NICHT gebaut, aber ab Task 1 GECHECKT werden (`cargo check/clippy -p app` im erweiterten Dev-Image); echter Build nur via GitHub Actions windows-latest
- KEINE Netzläufe außer den bestehenden Integrationstests; gegen Isekai max. 2 Auth-Anläufe, nie `falsches_passwort` lokal
- Session-Namen/Pfade laufen ausschließlich durch `tmux::commands` (nie eigenes format!); tmux-Targets `=`-exakt
- PTY-`close()` nie im UI-blockierenden Pfad awaiten (kann bis 2 s dauern) — immer `tokio::spawn`
- `csp` in tauri.conf.json wird in Task 2 gesetzt (nicht mehr null): `"default-src 'self'; style-src 'self' 'unsafe-inline'; img-src 'self' data:"`
- Secrets: Passwort/Passphrase via `secrets::KeyringStore` (Windows) — im Frontend nie speichern, nur per invoke übergeben
- Kein Auto-Retry nach AuthFailed; Reconnect-Backoff 3/6/12/30 aus `reconnect::backoff_schedule()`
- Frontend-Logik (Badges, Reducer, Pfade) pure + vitest-getestet; React-Komponenten selbst werden nicht unit-getestet (E2E via Windows-Abnahme)
- Commits klein, Deutsch, `feat:`/`test:`/`fix:`/`chore:`

## IPC-Contract (bindend für alle Tasks)

Commands (alle `#[tauri::command]`, camelCase-Args via serde rename_all):
```
connect(password: Option<String>) -> Result<(), ApiError>        // nutzt Config-Profil; password überschreibt Keyring
disconnect() -> ()
get_config() -> Config                                            // claudedeck_core::config::Config (Serialize)
set_config(config: Config) -> Result<(), ApiError>
save_secret(kind: "password"|"keyPassphrase", value: String) -> Result<(), ApiError>
has_secret(kind) -> bool
accept_hostkey_and_connect(password: Option<String>) -> Result<(), ApiError>   // wie connect, Policy AcceptNew
list_sessions() -> Result<SessionList, ApiError>                  // { running: SessionInfo[], startable: Project[] }
open_session(name: String, cols: u16, rows: u16, onOutput: Channel<OutputChunk>) -> Result<String, ApiError>  // -> sessionId
start_project(path: String, cols: u16, rows: u16, onOutput: Channel<OutputChunk>) -> Result<StartResult, ApiError> // { sessionId, sessionName }
write_session(session_id: String, data_b64: String) -> Result<(), ApiError>
resize_session(session_id: String, cols: u16, rows: u16) -> Result<(), ApiError>
close_session(session_id: String) -> ()                           // Detach; Session lebt weiter
kill_session(name: String) -> Result<(), ApiError>                // tmux kill-session
```
`OutputChunk = { dataB64: string }` (gebatcht). `ApiError` = serialisierbares Enum
`{ kind: "authFailed"|"hostkeyUnknown"|"hostkeyChanged"|"notConnected"|"tmuxMissing"|"ssh"|"io", message: string, fingerprint?: string }`.

Events (Rust → Frontend): `connection-state` `{ state: "disconnected"|"connecting"|"connected"|"reconnecting"|"failed", attempt?: number, nextRetryInS?: number }`; `pty-exit` `{ sessionId, reason: "exited"|"connectionLost" }`; `sessions-changed` `{}`.

Frontend-Typen dazu in `src/lib/ipc.ts` (einzige Datei, die `invoke`/`listen` importiert).

---

### Task 1: Dev-Image für src-tauri-Checks + Workspace-Verdrahtung

**Files:** Modify `Dockerfile.dev`, `src-tauri/Cargo.toml`, `src-tauri/src/lib.rs`, Wurzel-`Cargo.toml`

- Dockerfile.dev: `apt-get install -y libwebkit2gtk-4.1-dev libgtk-3-dev libayatana-appindicator3-dev librsvg2-dev libxdo-dev libssl-dev pkg-config` (eine RUN-Schicht, mit `rm -rf /var/lib/apt/lists/*`)
- src-tauri: Template-Greet-Command entfernen; `tauri-plugin-notification = "2"` zu Cargo.toml + `.plugin(tauri_plugin_notification::init())` in lib.rs; `tauri-plugin-opener` (Template-Rest) entfernen falls ungenutzt
- Wurzel-Cargo.toml: default-members bleibt NUR core; aber neues Alias-Ziel dokumentieren: `./dev.sh cargo check -p app` bzw. clippy
- Verifikation: `./dev.sh cargo clippy -p app -- -D warnings` läuft durch (Image-Rebuild nötig: `docker rmi claudedeck-dev` zu Beginn); `./dev.sh cargo test` weiterhin nur core; npm-Seite unberührt
- Commit `chore: Dev-Image mit webkit2gtk für src-tauri-Checks + Notification-Plugin`

### Task 2: AppState + Verbindungs-Commands + CSP

**Files:** Create `src-tauri/src/state.rs`, `src-tauri/src/error.rs`, `src-tauri/src/commands/mod.rs`, `src-tauri/src/commands/connection.rs`; Modify `src-tauri/src/lib.rs`, `src-tauri/tauri.conf.json`, `src-tauri/capabilities/default.json`

- `error.rs`: `ApiError` (thiserror + Serialize, `#[serde(tag="kind", rename_all="camelCase")]`), `From<ConnectError>`-Impl (HostkeyUnknown trägt fingerprint)
- `state.rs`: `pub struct AppState { inner: tokio::sync::Mutex<AppInner> }`; `AppInner { conn: Option<SshConnection>, sessions: HashMap<String, SessionEntry>, next_id: u64 }`; `SessionEntry { pty: PtyHandle-Writer-Teil oder das ganze Handle, name: String }` — exakte Aufteilung richtet sich nach PtyHandle-API (write braucht &mut → Handle in eigenem Mutex pro Session oder als tokio::sync::mpsc-Kommando-Task)
- `connection.rs`: connect (lädt Config, Auth aus Keyring oder Parameter, `HostkeyPolicy::Strict`, known_hosts = app-eigene Datei `dirs::config_dir()/claudedeck/known_hosts` — Entscheidung aus Final-Review: umgeht die fail-open-Ecke bei fremden Einträgen), accept_hostkey_and_connect (AcceptNew, danach sofort Strict-Semantik, gleiche Datei), disconnect, get/set_config, save/has_secret; `connection-state`-Events bei jedem Übergang
- tauri.conf.json: CSP setzen (Global Constraints); capabilities: `core:default`, `core:event:default`, `notification:default`
- Verifikation: `./dev.sh cargo clippy -p app -- -D warnings`; `./dev.sh cargo test` (core unverändert grün)
- Commit `feat: AppState, Verbindungs-Commands, ApiError, CSP + Capabilities`

### Task 3: Session-Streaming-Commands (Kern von M4)

**Files:** Create `src-tauri/src/commands/sessions.rs`; Modify `src-tauri/src/commands/mod.rs`, `src-tauri/src/lib.rs`

- `open_session`: `conn.open_pty(&cmd_attach(name), cols, rows)` → sessionId vergeben → Forwarder-Task: liest `PtyEvent` aus `take_output()`, batcht (flush bei 32 KiB ODER 10 ms seit erstem ungeflushten Byte — `tokio::select!` mit `tokio::time::sleep_until`), sendet `Channel<OutputChunk>` (base64); bei `PtyEvent::Exit` → Event `pty-exit` + Session aus Map räumen
- `start_project`: Name = `names::resolve_collision(&format!("cc-{}", names::sanitize(basename)), &laufende)` → `exec_capture(cmd_new_detached(name, path, "claude"))` → wie open_session; Rückgabe inkl. sessionName
- `list_sessions`: `exec_capture(cmd_list_sessions/panes)` + parser::merge → running; Scan: `exec_capture(cmd_scan_projects(&config.scan_paths))` → startable (Pfad + basename), bereits laufende `cc-<name>` herausfiltern
- `write_session` (base64-decode → pty.write), `resize_session`, `close_session` (spawn!), `kill_session` (`cmd_kill` + `sessions-changed`-Event)
- Verifikation: clippy -p app sauber; PLUS Smoke-Test der Batching-Logik: die Batch-Funktion als pure Funktion (`fn should_flush(buffered: usize, elapsed_ms: u64) -> bool` o.ä.) in claudedeck-core::util oder src-tauri mit Unit-Test (im core, damit testbar: `./dev.sh cargo test`)
- Commit `feat: Session-Commands mit Channel-Streaming und Output-Batching`

### Task 4: Frontend-Fundament — ipc.ts, Stores, TermPool (TDD für Logik)

**Files:** Create `src/lib/ipc.ts`, `src/lib/termPool.ts`, `src/lib/badges.ts`, `src/stores/sessionStore.ts`, `src/stores/connectionStore.ts`; Tests `src/lib/__tests__/badges.test.ts`, `src/stores/__tests__/sessionStore.test.ts`; Modify `package.json` (falls @xterm fehlt: `npm i @xterm/xterm@^6 @xterm/addon-fit @xterm/addon-search zustand @tauri-apps/plugin-notification`)

- `badges.ts` (pure, TDD): `interface Activity { badge: number; lastOutputAt: number|null; notified: boolean }`; `onOutput(a, now, isActive): Activity` (aktiv → badge 0, sonst +1, lastOutputAt=now, notified=false); `shouldNotify(a, now, enabled, thresholdMs=2000): boolean` (nur wenn enabled, !notified, lastOutputAt gesetzt, now-lastOutputAt >= threshold); Tests mit expliziten now-Werten (keine Timer nötig): Output→1,9s→false; 2,1s→true; nach Notify kein zweites Mal; aktive Session nie
- `sessionStore.ts` (zustand, TDD über Store-Actions als pure Aufrufe): running/startable-Listen, openSessions: Map<sessionId, {name, activity, notifyEnabled}>, activeSessionId; Actions: sessionsLoaded, opened, activated (badge reset), outputReceived(id, now), closed, notifyToggled
- `termPool.ts`: Map<sessionId, {term, fit, search, el}>; `ensure(sessionId, onData, onResize)` erzeugt Terminal({scrollback:10000, fontFamily:'Consolas, monospace'}) in detached div; `show(sessionId, host: HTMLElement)` hängt el um / display, fit, focus; `hide`, `write(id, bytes)`, `dispose(id)`; KEIN WebGL in M4/M5 (Kontext-Limit, erst bei Perf-Bedarf)
- `ipc.ts`: typisierte Wrapper für alle Commands/Events aus dem IPC-Contract; base64-Helpers (atob/btoa mit Uint8Array, korrekt für Binärdaten)
- Verifikation: `npx vitest run` (6 alte + ~10 neue Tests grün), `npx tsc -b`, `npm run build`
- Commit `feat: Frontend-Fundament — IPC-Wrapper, TermPool, Badge-Logik, Stores (TDD)`

### Task 5: UI zusammenstecken — App-Layout, Connect-Flow, Terminal (M4-Abschluss)

**Files:** Create `src/components/Sidebar.tsx`, `src/components/TerminalHost.tsx`, `src/components/ConnectGate.tsx`, `src/components/dialogs/HostKeyDialog.tsx`, `src/components/StatusBar.tsx`; Rewrite `src/App.tsx`, `src/App.css` (Template-Reste raus: hero.png, icons.svg, Counter)

- App: ConnectGate (fragt Passwort ab, wenn kein Secret; ruft connect; HostKeyDialog bei hostkeyUnknown mit Fingerprint, Abbrechen = Default-Button/autoFocus, Bestätigen → accept_hostkey_and_connect; hostkeyChanged → reine Fehlanzeige ohne Bestätigungsoption) → Hauptlayout Sidebar(240px) + TerminalHost
- Sidebar: Gruppen ● angehängt / ○ läuft / + startbar, Klick öffnet/wechselt (open_session mit Channel → termPool.write), Badge-Zahl, Refresh bei window-focus + sessions-changed
- TerminalHost: ResizeObserver → fit() → resize_session; onData → write_session (base64)
- StatusBar: connection-state (verbunden/reconnecting-Anzeige, Task 6 verfeinert)
- Dark-Theme (schlicht: Systemfarben, kein Theme-System — YAGNI)
- Verifikation: `npx tsc -b && npx vitest run && npm run build`; `./dev.sh cargo clippy -p app -- -D warnings`; danach ERSTER ECHTER FUNKTIONSTEST: Windows-Build via GitHub Actions (setzt gepushtes Repo voraus — falls noch nicht möglich: lokaler `npm run tauri build`-Ersatz entfällt, dann als "offen: Windows-Abnahme" markieren und weiter)
- Commit `feat: UI — Connect-Flow, Sidebar, TerminalHost, Hostkey-Dialog`

### Task 6: M5 — Multi-Session-Komfort: Badges live, Notifications, Suche, Start-Projekte, Reconnect-Overlay

**Files:** Modify `src/components/*`, `src/App.tsx`; Create `src/components/SearchBar.tsx`, `src/components/ReconnectOverlay.tsx`; Rust: `src-tauri/src/reconnect_supervisor.rs` (Modify lib.rs)

- Badges/Notifications verdrahten: outputReceived aus Channel-Callback; Notification-Timer pro Hintergrund-Session (setTimeout auf threshold, shouldNotify-Check, `sendNotification({title: name, body: "wartet auf Eingabe"})`); Kontextmenü-Eintrag "Benachrichtigungen aus" pro Session
- Strg+F: SearchBar über aktivem Terminal (search-addon findNext/Previous, Esc schließt)
- Startbare Projekte: Klick → start_project → erscheint als angehängt; Fehler (tmuxMissing etc.) als Toast
- Reconnect: Rust-Supervisor beobachtet `conn.disconnected()`-watch (falls in M2 vorhanden — sonst: Fehler aus write/exec als Trigger), Events reconnecting/attempt; nach erfolgreichem Reconnect alle offenen Sessions automatisch re-attachen (neue PTYs, gleiche sessionIds, Frontend schreibt weiter ins selbe Terminal); ReconnectOverlay mit Countdown + manuellem Button; kein Retry nach AuthFailed
- Verifikation: vitest (Notification-Edge-Cases), tsc, build, clippy -p app; Reconnect-Logik: Integrationstest-Erweiterung optional (nur wenn ohne Netz-Risiko machbar), sonst Windows-Abnahme-Checkliste
- Commit(s) `feat: Badges+Notifications`, `feat: Scrollback-Suche`, `feat: Auto-Reconnect mit Re-Attach`

### Task 7: Abschluss — E2E-Checkliste, Windows-Artifact, Doku

**Files:** Create `docs/e2e-checklist.md`; Modify `README.md` (Status M4/M5)

- Checkliste: Verbinden (Passwort/Keyring), Hostkey-Dialog beim Erstkontakt, Claude-TUI-Rendering (Farben, Maus, Umlaute äöü€, AltGr), 3+ Sessions parallel + Umschaltzeit, Badge/Notification, Strg+F, Resize, WLAN-Trennung → Overlay → Auto-Reattach, Projekt starten, kill-session
- GitHub: master + Branch pushen, `gh run watch` bis alle Jobs grün, Artifact-Link für den User
- Commit `docs: E2E-Checkliste + README-Update`

## Verifikation Gesamt
1. `./dev.sh cargo test` grün (core), `./dev.sh cargo clippy -p app -- -D warnings` sauber
2. `npx vitest run` (~16+ Tests) + `npx tsc -b` + `npm run build` grün
3. CI komplett grün auf GitHub (core, frontend, integration) + build.yml-Artifact
4. User-Abnahme auf Windows anhand docs/e2e-checklist.md — erst dann ist M5 abgeschlossen

## Danach
Plan 4 (M6–M8): SFTP-Panel, Keyring-Windows-Abnahme, Fehlermatrix-Härtung, Release-Feinschliff.
