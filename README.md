# ClaudeDeck

Schlanke Windows-Desktop-App zum Verwalten von Claude-Code-Sessions auf einem Homelab-Server:
verbindet sich per SSH, listet tmux-Sessions, öffnet sie als vollwertige Terminals (mehrere
parallel über eine gemultiplexte Verbindung) und bietet ein SFTP-Seitenpanel für Up-/Downloads.

**Stack:** Tauri v2 · Rust (russh 0.62, russh-sftp) · React + TypeScript · xterm.js

## Status

M0+M1 abgeschlossen: Projekt-Scaffold, CI, Windows-Build-Workflow und ein headless Spike,
der PTY-Streaming und Reattach-Semantik über russh live validiert. Design und Pläne unter
[`docs/superpowers/`](docs/superpowers/).

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
