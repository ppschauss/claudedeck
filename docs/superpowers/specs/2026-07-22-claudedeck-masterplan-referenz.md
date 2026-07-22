# ClaudeDeck — Masterplan-Referenz (Plan-Agent-Entwurf, 2026-07-22)

Recherchierte Grundlage für die Implementierungspläne. Plan 1 (M0+M1) ist ausgearbeitet in
`docs/superpowers/plans/2026-07-22-claudedeck-m0-m1-foundation.md`; die Folgepläne (M2–M8)
werden nach dem Spike auf Basis dieses Dokuments geschrieben.

## Verifizierte Versionen (Stand 2026-07-22)

| Paket | Version | Anmerkung |
|---|---|---|
| `russh` | 0.62.3 | russh-keys vollständig absorbiert → `russh::keys` (check_known_hosts_path, load_secret_key, Re-Export ssh_key). Native async traits. API ≠ 0.44 — nur docs.rs 0.62 + Upstream-Examples als Referenz. |
| `russh-sftp` | 2.1.x | `SftpSession::new(channel.into_stream())` |
| `keyring` | 3.6.x | Features `windows-native` (Credential Manager) + `linux-native` (keyutils, headless-tauglich für Dev) |
| `tauri` | 2.10.x / CLI 2.11 | plus `tauri-plugin-notification` 2.x |
| `@xterm/xterm` | 6.0.0 | Addons: fit 0.11, search 0.16, webgl 0.19 |
| React / Vite / vitest / zustand | 19.2 / 8.1 / 4.1 / 5.0 | |

## Rust-Module (crates/claudedeck-core)

- `ssh/hostkey.rs` — `check(known_hosts, host, port, key) -> HostkeyStatus{Known|Unknown{fingerprint}|Changed}`, `append(...)` (selbst schreiben — kein learn_known_hosts in 0.62), `fingerprint_sha256`. Pur, tempfile-testbar. Gehashte known_hosts-Einträge explizit testen.
- `ssh/connection.rs` — EINE `client::Handle<ClientHandler>`; `open_pty`, `exec_capture`, `open_sftp`, `disconnected() -> watch::Receiver<bool>`. Auth: Key (`load_secret_key`, `PrivateKeyWithHashAlg`) oder Passwort. `ConnectError`: HostkeyUnknown/HostkeyChanged/AuthFailed/Io.
- `ssh/pty.rs` — `PtyHandle{write, resize(window_change), take_output() -> mpsc::Receiver<PtyEvent{Data|Exit}>, close}`. Reader-Task über `channel.wait()`.
- `ssh/exec.rs` — `exec_capture -> ExecOutput{stdout, stderr, exit_code}`; exit 127 → `TmuxMissing`.
- `ssh/sftp.rs` — list_dir/upload/download/exists, 64-KiB-Chunks, Progress-Callback, CancellationToken. Remote-Pfade nur als `/`-Strings, nie std::path.
- `tmux/parser.rs` — parse_sessions/parse_panes (tab-separiert) + `merge -> SessionInfo{name, kind: Claude|Shell, cwd, attached, created, managed}`. Testfälle: leer, „no server running“, Namen mit Leerzeichen, mehrere Panes, command node vs claude.
- `tmux/names.rs` — sanitize (fertig, M0), resolve_collision (fertig, M0).
- `tmux/commands.rs` — `shell_quote` + Kommando-Builder (list-sessions/list-panes-Formatstrings, new -A -d, attach, pane_cwd, Projekt-Scan via find -mindepth/-maxdepth 1). Pur testbar.
- `config.rs` — Config{profile, scan_paths, favorites, notifications}, `load_from(path)` mit serde-Defaults, Pfad `dirs::config_dir()/claudedeck/config.json`.
- `secrets.rs` — Trait `SecretStore{get,set,delete}` mit `KeyringStore` (service "claudedeck", user "<profil>:<kind>") + `MemoryStore` für Dev/Tests.
- `reconnect.rs` — `backoff_schedule()` = 3,6,12,30,30,…; Supervisor ohne Auto-Retry nach AuthFailed.

## Tauri-IPC

Commands: connect/accept_hostkey_and_connect/disconnect, save_secret/has_secret, get_config/set_config, list_sessions, open_session(name,cols,rows,onOutput: Channel<PtyChunk>) -> sessionId, start_project, write_session(dataB64), resize_session, close_session (=detach), kill_session, get_pane_cwd, sftp_list/home/upload/download/cancel.

**PTY-Output über `tauri::ipc::Channel`** (nicht globale Events) mit Batching im Rust-Reader: flush alle ~10 ms oder ab 32 KiB — wichtigster Performance-Hebel. Frontend: Base64 → `term.write(uint8array)`.

Events: connection-state{state,attempt,nextRetryInS}, pty-exit{sessionId,reason,exitCode}, sessions-changed, transfer-progress (max ~4/s), transfer-done/-error.

Capabilities: core:default, core:event:default, core:window:default, notification:default. Drag&Drop: `dragDropEnabled: true` + `onDragDropEvent` (echte Windows-Pfade; HTML5-Drop ist in WebView2 tot).

## Frontend

- `lib/termPool.ts` — Map<sessionId, {term, fit, search, el}>; Umschalten = display:none/block + fit + focus, NIE dispose. `scrollback: 10000`. WebGL-Addon NUR am aktiven Terminal (Browser-Limit ~16 Contexts), onContextLoss behandeln.
- `TerminalHost.tsx` (ResizeObserver → fit → resize_session; Strg+F-Suchleiste), `Sidebar.tsx` (Gruppen ●/○/+, Badges, Kontextmenü, Refresh bei window-focus + sessions-changed), `SftpPanel.tsx` (Breadcrumb, Drop-Zone, Progress; Strg+B; folgt aktiver Session via get_pane_cwd).
- Stores (zustand): sessionStore{sessions, openSessions{badge, lastOutputAt, notifyEnabled}, activeSessionId}, connectionStore{state, retryCountdown}.
- `lib/badges.ts` pur: onOutput(state, now, isActive), silenceElapsed(state, now, 2000) — vitest mit fake timers.
- Dialoge: HostKey (Abbrechen = autoFocus/Default), Auth, Overwrite, ReconnectOverlay, Settings, Toast.
- xterm `onData` (nicht onKey) für korrekte Umlaute/IME.

## CI

- ci.yml: core-Job (fmt/clippy/test, ohne webkit dank default-members), frontend-Job (tsc/vitest/build), ab M3 integration-Job: Service-Container `lscr.io/linuxserver/openssh-server` mit `DOCKER_MODS: linuxserver/mods:universal-package-install`, `INSTALL_PACKAGES: tmux`, Port 2222, User testuser/testpass, Readiness-Wait bis 90 s (Package-Mod braucht Startzeit), dann `CLAUDEDECK_TEST_SSH=localhost:2222:testuser:testpass cargo test -p claudedeck-core --test integration_ssh -- --ignored`. Container-User ist non-root → Tests schreiben nach `$HOME`.
- build.yml: windows-latest, `npm run tauri build -- --bundles msi`, Artifacts: msi + rohe `claudedeck.exe` (= die portable Variante; braucht nur WebView2).

## Meilensteine (Rest)

- M2: Core-Module per TDD (Reihenfolge: parser → names ✓ → commands → hostkey → config → reconnect → Frontend-Tests) + Spike-Code nach connection/pty/exec refaktorieren.
- M3: integration_ssh.rs (#[ignore]d): connect → tmux anlegen → PTY attach → Marker → Channel zu → reattach → Marker im Scrollback → SFTP-Roundtrip. CI-Job dazu.
- M4: Tauri-Brücke, eine Session in der UI (ipc::Channel-Streaming, TermPool mit 1 Instanz, Mini-Sidebar). Verifikation: resize via `tmux display -p '#{window_width}'`.
- M5: Multi-Session, Sidebar komplett, Badges + Notifications, Strg+F. Umschalten < 50 ms.
- M6: SFTP-Panel komplett (Progress, Overwrite-Dialog, Downloads-Ordner, 100-MB-Test mit sha256-Vergleich).
- M7: Härtung — Reconnect-Supervisor + Re-Attach aller Sessions, Hostkey-Dialoge, KeyringStore auf Windows (`cmdkey /list`), Fehlermatrix aus der Spec, fail2ban-sicheres Auth-Verhalten.
- M8: Icon, Settings-Dialog, docs/e2e-checklist.md (TUI, Farben, Maus, IME/Umlaute äöü€/AltGr, Resize), Release-Tag.

## Risiken (Kurzliste)

1. russh-API-Drift → Version pinnen `=0.62.3`, Spike zuerst.
2. Gehashte known_hosts-Einträge → explizit testen, ggf. HMAC-SHA1-Vergleich selbst.
3. keyring/Windows: Wincred-Limit ~2560 Bytes — nur Passwörter/Passphrasen, nie Keys.
4. Vergessene Tauri-Capabilities äußern sich als stumme Fehler in der WebView-Konsole.
5. WebGL-Kontext-Limit ~16 → Addon nur am aktiven Terminal.
6. Output-Flut: 10-ms/32-KiB-Batching im Rust-Backend ist der wichtigste Hebel.
7. Remote-Pfade nie durch Windows-Path-APIs; Download-Namen gegen `<>:"|?*` sanitizen.
8. linuxserver/openssh-server-Startzeit → Readiness-Wait, sonst flaky CI.
9. `-A`-Semantik: Start = exec mit `-A -d`, Anzeigen = separates PTY-Attach (nie `-d` im PTY).
10. Vite 8: Doku teils noch auf 7; devUrl-Port 5173 abgleichen.
