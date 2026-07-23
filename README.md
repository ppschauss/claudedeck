# ClaudeDeck

Schlanke Windows-Desktop-App zum Verwalten von Claude-Code-Sessions auf einem Homelab-Server:
verbindet sich per SSH, listet tmux-Sessions und öffnet sie als vollwertige Terminals (mehrere
parallel über eine gemultiplexte Verbindung). Ein SFTP-Seitenpanel für Up-/Downloads ist für
M6 geplant.

**Stack:** Tauri v2 · Rust (russh 0.62, russh-sftp) · React + TypeScript · xterm.js

## Status

M0–M5 abgeschlossen: verbinden, mehrere Sessions parallel, Badges/Notifications, Suche,
Auto-Reconnect. Offen: SFTP (M6), Windows-Abnahme, Key-Auth. Design und Pläne unter
[`docs/superpowers/`](docs/superpowers/).

## Benutzung

1. **Erste Verbindung:** Passwort eingeben, optional „im Windows-Anmeldedaten-Speicher
   merken" ankreuzen — bei bekanntem Host verbindet die App danach automatisch. Bei einem
   noch unbekannten Host-Key zeigt ein Dialog den Fingerprint zur Bestätigung.
2. **Session öffnen:** In der Sidebar auf einen laufenden Eintrag (●/○) klicken — das
   Terminal öffnet sich sofort, mehrere Sessions bleiben parallel im Hintergrund aktiv.
3. **Projekt starten:** Einen Eintrag unter „+ startbar" anklicken — legt eine neue
   `claude`-Session im jeweiligen Projektordner an und hängt sich sofort an.
4. **Scrollback durchsuchen:** Strg+F im aktiven Terminal öffnet die Suche
   (Enter/Shift+Enter für nächsten/vorherigen Treffer, Esc schließt).

Details und die manuelle Windows-Abnahme-Checkliste: [`docs/e2e-checklist.md`](docs/e2e-checklist.md).

## Entwicklung

Entwickelt wird auf einem Linux-Host ohne Rust-Toolchain — alle Cargo-Kommandos laufen im
Container:

```bash
./dev.sh cargo test              # Tests (nur claudedeck-core, kein webkit2gtk nötig)
./dev.sh cargo run --example spike -- <host> <user> exec "tmux ls"   # SSH-Spike
npx vitest run                   # Frontend-Tests
```

`secrets.env` (gitignored, chmod 600) stellt `SPIKE_SSH_PASSWORD` für den Spike bereit.

## Build

Windows-Builds entstehen ausschließlich in GitHub Actions (`build.yml`, windows-latest):
`.msi`-Installer + portable `claudedeck.exe` als Artifact `claudedeck-windows`.
