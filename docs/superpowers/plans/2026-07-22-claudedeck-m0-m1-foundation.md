# ClaudeDeck M0+M1 (Fundament + russh-Spike) Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Projekt-Scaffold (Vite+React+TS, Tauri v2, Rust-Workspace), CI/Build-Pipelines auf GitHub, und ein headless russh-Spike, der die riskanteste Unbekannte (PTY-Streaming über russh 0.62) end-to-end gegen isekai.local validiert.

**Architecture:** Cargo-Workspace mit tauri-freiem `crates/claudedeck-core` (auf Linux ohne webkit2gtk testbar) + `src-tauri` (baut nur in CI auf windows-latest). Rust läuft lokal ausschließlich im Docker-Container (`dev.sh`), da Unraid keine Toolchain hat. Der Spike ist ein `examples/spike.rs` in claudedeck-core.

**Tech Stack:** Rust (russh 0.62, tokio 1), Tauri 2.10, React 19 + Vite + vitest, GitHub Actions.

## Global Constraints

- Spec: `docs/superpowers/specs/2026-07-21-claudedeck-design.md` (freigegeben)
- Repo-Wurzel: `/mnt/cache/appdata/claudedeck` — **niemals** unter `/root` arbeiten (RAM!)
- Rust-Kommandos lokal IMMER über `./dev.sh cargo …` (Docker `rust:1-bookworm`); es gibt keine Host-Toolchain
- `russh = "=0.62.3"` gepinnt; API weicht stark von <0.60 ab, russh-keys ist in `russh::keys` aufgegangen — bei Compile-Fehlern docs.rs/russh/0.62.3 konsultieren, keine alten Blogposts
- Workspace-`default-members = ["crates/claudedeck-core"]`, damit `cargo test` im Linux-Container nie src-tauri (webkit2gtk!) baut
- tmux-Sessions der App tragen Präfix `cc-`; Spike benutzt `cc-spike`
- Secrets nur in gitignored `secrets.env` (chmod 600) bzw. Env-Vars; nie committen
- Commits klein und häufig, Messages auf Deutsch, Format `feat:`/`test:`/`chore:`/`ci:`
- SSH-Ziel für den Spike: `192.168.0.161:22`, User `root`, Passwort aus `SPIKE_SSH_PASSWORD`

---

### Task 1: Repo-Hygiene + Dev-Container-Harness

**Files:**
- Create: `.gitignore`
- Create: `dev.sh`
- Create: `secrets.env` (nicht committen!)

**Interfaces:**
- Produces: `./dev.sh <befehl…>` — führt beliebige Kommandos im Rust-Container aus, cwd `/work` = Repo, Cargo-Caches persistent unter `.cargo-cache/`, reicht `SPIKE_SSH_PASSWORD` durch.

- [ ] **Step 1: .gitignore anlegen**

```gitignore
node_modules/
dist/
target/
.cargo-cache/
secrets.env
*.log
```

- [ ] **Step 2: dev.sh anlegen**

```bash
#!/bin/bash
# Rust-Dev im Container — Unraid hat keine Toolchain (Konvention wie android-build).
# Aufruf: ./dev.sh cargo test   |   ./dev.sh cargo run --example spike -- …
set -euo pipefail
DIR="$(cd "$(dirname "$0")" && pwd)"
mkdir -p "$DIR/.cargo-cache/registry" "$DIR/.cargo-cache/git" "$DIR/.cargo-cache/target"
[ -f "$DIR/secrets.env" ] && set -a && . "$DIR/secrets.env" && set +a
exec docker run --rm -i \
  -v "$DIR":/work -w /work \
  -v "$DIR/.cargo-cache/registry":/usr/local/cargo/registry \
  -v "$DIR/.cargo-cache/git":/usr/local/cargo/git \
  -e CARGO_TARGET_DIR=/work/.cargo-cache/target \
  -e "SPIKE_SSH_PASSWORD=${SPIKE_SSH_PASSWORD:-}" \
  rust:1-bookworm "$@"
```

- [ ] **Step 3: secrets.env anlegen (chmod 600)**

```bash
printf 'SPIKE_SSH_PASSWORD=<lokales Standard-Passwort des Users>\n' > secrets.env
chmod 600 secrets.env
```

- [ ] **Step 4: Verifizieren, dass der Container läuft**

Run: `chmod +x dev.sh && ./dev.sh cargo --version`
Expected: `cargo 1.8x.x` (Image wird beim ersten Mal gezogen)

- [ ] **Step 5: Commit**

```bash
git add .gitignore dev.sh
git commit -m "chore: gitignore + dev.sh (Rust im Container, Unraid ohne Toolchain)"
```

---

### Task 2: Frontend-Scaffold (Vite + React + TS + vitest) mit Seed-Test

**Files:**
- Create: `package.json`, `vite.config.ts`, `tsconfig.json`, `index.html`, `src/main.tsx`, `src/App.tsx` (via Vite-Template)
- Create: `src/lib/paths.ts`
- Test: `src/lib/__tests__/paths.test.ts`

**Interfaces:**
- Produces: `joinRemote(dir: string, name: string): string`, `parentRemote(p: string): string`, `basenameRemote(p: string): string` — Remote-Pfad-Helfer (immer `/`, nie Windows-`\`), werden später vom SFTP-Panel konsumiert.

- [ ] **Step 1: Vite-Template in temporäres Verzeichnis scaffolden und in die Repo-Wurzel übernehmen**

```bash
cd /mnt/cache/appdata/claudedeck
npm create vite@latest tmp-scaffold -- --template react-ts
rsync -a --ignore-existing tmp-scaffold/ ./
rm -rf tmp-scaffold
npm install
npm i -D vitest@^4 @tauri-apps/cli@^2
```

- [ ] **Step 2: vitest in package.json verdrahten**

In `package.json` unter `"scripts"` ergänzen: `"test": "vitest run"`, `"tauri": "tauri"`.

- [ ] **Step 3: Failing Test schreiben**

`src/lib/__tests__/paths.test.ts`:
```ts
import { describe, expect, it } from "vitest";
import { basenameRemote, joinRemote, parentRemote } from "../paths";

describe("remote paths", () => {
  it("joint Verzeichnis und Name", () => {
    expect(joinRemote("/mnt/cache/appdata", "otakupulse")).toBe("/mnt/cache/appdata/otakupulse");
  });
  it("joint an der Wurzel ohne Doppel-Slash", () => {
    expect(joinRemote("/", "etc")).toBe("/etc");
  });
  it("ignoriert trailing Slashes beim Join", () => {
    expect(joinRemote("/tmp/", "x")).toBe("/tmp/x");
  });
  it("liefert das Parent-Verzeichnis", () => {
    expect(parentRemote("/mnt/cache/appdata")).toBe("/mnt/cache");
  });
  it("Parent der Wurzel bleibt die Wurzel", () => {
    expect(parentRemote("/")).toBe("/");
    expect(parentRemote("/etc")).toBe("/");
  });
  it("liefert den Basename", () => {
    expect(basenameRemote("/a/b/c.txt")).toBe("c.txt");
    expect(basenameRemote("/")).toBe("/");
  });
});
```

- [ ] **Step 4: Test laufen lassen — muss fehlschlagen**

Run: `npx vitest run`
Expected: FAIL — `Cannot find module '../paths'` (o.ä.)

- [ ] **Step 5: Implementierung**

`src/lib/paths.ts`:
```ts
/** Remote-Pfade sind IMMER Unix-Pfade mit "/" — nie window-seitige Path-APIs benutzen. */
export function joinRemote(dir: string, name: string): string {
  const base = dir.replace(/\/+$/, "");
  return base === "" ? `/${name}` : `${base}/${name}`;
}

export function parentRemote(p: string): string {
  const t = p.replace(/\/+$/, "");
  const i = t.lastIndexOf("/");
  return i <= 0 ? "/" : t.slice(0, i);
}

export function basenameRemote(p: string): string {
  const t = p.replace(/\/+$/, "");
  if (t === "") return "/";
  return t.slice(t.lastIndexOf("/") + 1);
}
```

- [ ] **Step 6: Tests grün + Build läuft**

Run: `npx vitest run && npm run build`
Expected: alle 6 Tests PASS, `vite build` erzeugt `dist/`

- [ ] **Step 7: Commit**

```bash
git add package.json package-lock.json vite.config.ts tsconfig*.json index.html public src eslint.config.js .gitignore
git commit -m "feat: Vite+React+TS-Scaffold mit vitest und Remote-Pfad-Helfern"
```

---

### Task 3: Rust-Workspace + claudedeck-core mit erstem TDD-Modul

**Files:**
- Create: `Cargo.toml` (Workspace-Wurzel)
- Create: `crates/claudedeck-core/Cargo.toml`
- Create: `crates/claudedeck-core/src/lib.rs`
- Create: `crates/claudedeck-core/src/tmux/mod.rs`
- Create: `crates/claudedeck-core/src/tmux/names.rs` (Tests inline im Modul)

**Interfaces:**
- Produces: `claudedeck_core::tmux::names::sanitize(folder: &str) -> String` und `resolve_collision(base: &str, existing: &HashSet<String>) -> String` — konsumiert später vom `start_project`-Command.

- [ ] **Step 1: Workspace-Wurzel `Cargo.toml`**

```toml
[workspace]
members = ["crates/claudedeck-core"]
default-members = ["crates/claudedeck-core"]
resolver = "2"
```
(src-tauri kommt in Task 4 in `members`, bleibt aber aus `default-members` draußen.)

- [ ] **Step 2: `crates/claudedeck-core/Cargo.toml`**

```toml
[package]
name = "claudedeck-core"
version = "0.1.0"
edition = "2021"

[dependencies]
russh = "=0.62.3"
russh-sftp = "2.1"
tokio = { version = "1", features = ["rt-multi-thread", "macros", "io-util", "io-std", "sync", "time", "signal", "fs"] }
thiserror = "2"

[dev-dependencies]
crossterm = "0.29"
```

- [ ] **Step 3: Modulgerüst + failing Tests**

`crates/claudedeck-core/src/lib.rs`:
```rust
pub mod tmux;
```

`crates/claudedeck-core/src/tmux/mod.rs`:
```rust
pub mod names;
```

`crates/claudedeck-core/src/tmux/names.rs` — zuerst NUR die Tests (Funktionen noch mit `todo!()`):
```rust
use std::collections::HashSet;

/// Ordnername -> tmux-tauglicher Session-Name (ohne "cc-"-Präfix).
pub fn sanitize(folder: &str) -> String {
    todo!()
}

/// Hängt -2, -3, … an, bis der Name nicht in `existing` vorkommt.
pub fn resolve_collision(base: &str, existing: &HashSet<String>) -> String {
    todo!()
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashSet;

    #[test]
    fn ersetzt_sonderzeichen_durch_bindestrich() {
        assert_eq!(sanitize("mein projekt (alt)"), "mein-projekt--alt-");
    }

    #[test]
    fn behaelt_erlaubte_zeichen() {
        assert_eq!(sanitize("Otaku_Pulse-2"), "Otaku_Pulse-2");
    }

    #[test]
    fn ersetzt_umlaute() {
        assert_eq!(sanitize("löffelholz"), "l-ffelholz");
    }

    #[test]
    fn begrenzt_auf_40_zeichen() {
        let long = "a".repeat(50);
        assert_eq!(sanitize(&long).len(), 40);
    }

    #[test]
    fn kollision_haengt_zaehler_an() {
        let existing: HashSet<String> = ["cc-app".into(), "cc-app-2".into()].into();
        assert_eq!(resolve_collision("cc-app", &existing), "cc-app-3");
    }

    #[test]
    fn ohne_kollision_bleibt_name() {
        let existing: HashSet<String> = HashSet::new();
        assert_eq!(resolve_collision("cc-app", &existing), "cc-app");
    }
}
```

- [ ] **Step 4: Tests laufen lassen — müssen fehlschlagen (panic: not yet implemented)**

Run: `./dev.sh cargo test -p claudedeck-core names`
Expected: FAIL, 6 Tests panicken mit `not yet implemented`

- [ ] **Step 5: Implementierung**

In `names.rs` die beiden `todo!()`-Bodies ersetzen:
```rust
pub fn sanitize(folder: &str) -> String {
    let mut s: String = folder
        .chars()
        .map(|c| if c.is_ascii_alphanumeric() || c == '_' || c == '-' { c } else { '-' })
        .collect();
    s.truncate(40);
    s
}

pub fn resolve_collision(base: &str, existing: &HashSet<String>) -> String {
    if !existing.contains(base) {
        return base.to_string();
    }
    for i in 2u32.. {
        let candidate = format!("{base}-{i}");
        if !existing.contains(&candidate) {
            return candidate;
        }
    }
    unreachable!()
}
```
Achtung `sanitize` bei Mehr-Byte-Zeichen: durch das Mapping sind alle Zeichen ASCII, `truncate(40)` ist damit safe.

- [ ] **Step 6: Tests grün**

Run: `./dev.sh cargo test -p claudedeck-core`
Expected: 6 Tests PASS

- [ ] **Step 7: Commit**

```bash
git add Cargo.toml Cargo.lock crates
git commit -m "feat: Rust-Workspace + tmux::names (sanitize, Kollisionsauflösung) per TDD"
```

---

### Task 4: Tauri v2 init + Windows-Build-Workflow

**Files:**
- Create: `src-tauri/` (via `tauri init`)
- Modify: `Cargo.toml` (Workspace-members)
- Modify: `src-tauri/Cargo.toml`, `src-tauri/tauri.conf.json`
- Create: `.github/workflows/build.yml`

**Interfaces:**
- Produces: GitHub-Artifact `claudedeck-windows` mit `.msi` + portabler `claudedeck.exe`. Konsumiert vom User (Download auf den Windows-Rechner).

- [ ] **Step 1: tauri init (non-interactiv)**

```bash
cd /mnt/cache/appdata/claudedeck
npx tauri init --ci --app-name claudedeck --window-title ClaudeDeck \
  --frontend-dist ../dist --dev-url http://localhost:5173 \
  --before-dev-command "npm run dev" --before-build-command "npm run build"
```

- [ ] **Step 2: src-tauri in den Workspace hängen**

Wurzel-`Cargo.toml`: `members = ["crates/claudedeck-core", "src-tauri"]` (default-members unverändert!).
In `src-tauri/Cargo.toml` einen evtl. generierten `[workspace]`-Leereintrag **entfernen** und ergänzen:
```toml
claudedeck-core = { path = "../crates/claudedeck-core" }
```
In `src-tauri/tauri.conf.json`: `"identifier": "com.claudedeck.app"`.

- [ ] **Step 3: Sanity-Check, dass core weiterhin isoliert testbar ist**

Run: `./dev.sh cargo test`
Expected: nur claudedeck-core wird gebaut (kein webkit2gtk-Fehler), 6 Tests PASS

- [ ] **Step 4: build.yml**

`.github/workflows/build.yml`:
```yaml
name: build
on:
  push:
    branches: [main]
  workflow_dispatch:

jobs:
  windows:
    runs-on: windows-latest
    steps:
      - uses: actions/checkout@v4
      - uses: actions/setup-node@v4
        with:
          node-version: 22
      - uses: dtolnay/rust-toolchain@stable
      - uses: Swatinem/rust-cache@v2
      - run: npm ci
      - run: npm run tauri build -- --bundles msi
      - uses: actions/upload-artifact@v4
        with:
          name: claudedeck-windows
          path: |
            target/release/bundle/msi/*.msi
            target/release/claudedeck.exe
```
(Falls das Bundle unter `src-tauri/target/…` statt `target/…` landet, beide Globs eintragen — hängt davon ab, ob tauri das Workspace-target nutzt; beim ersten CI-Lauf im Log prüfen.)

- [ ] **Step 5: Commit**

```bash
git add Cargo.toml src-tauri .github/workflows/build.yml
git commit -m "feat: Tauri-v2-Gerüst + Windows-Build-Workflow (msi + portable exe)"
```

---

### Task 5: CI-Workflow (Linux: fmt/clippy/test + Frontend)

**Files:**
- Create: `.github/workflows/ci.yml`

**Interfaces:**
- Produces: grüner `ci`-Check auf jedem Push; Basis für die Integrationstests in M3 (sshd-Service-Container-Job kommt erst dann dazu).

- [ ] **Step 1: ci.yml**

```yaml
name: ci
on:
  push:
  pull_request:

jobs:
  core:
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@v4
      - uses: dtolnay/rust-toolchain@stable
        with:
          components: rustfmt, clippy
      - uses: Swatinem/rust-cache@v2
      - run: cargo fmt --all --check
      - run: cargo clippy -p claudedeck-core --all-targets -- -D warnings
      - run: cargo test -p claudedeck-core

  frontend:
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@v4
      - uses: actions/setup-node@v4
        with:
          node-version: 22
      - run: npm ci
      - run: npx tsc --noEmit
      - run: npx vitest run
      - run: npm run build
```

- [ ] **Step 2: Lokal vorprüfen, was CI prüfen wird**

Run: `./dev.sh cargo fmt --all --check && ./dev.sh cargo clippy -p claudedeck-core --all-targets -- -D warnings && npx tsc --noEmit && npx vitest run`
Expected: alles grün (fmt-Fehler jetzt fixen, nicht in CI debuggen)

- [ ] **Step 3: Commit**

```bash
git add .github/workflows/ci.yml
git commit -m "ci: Linux-Pipeline (fmt, clippy, cargo test, tsc, vitest, vite build)"
```

---

### Task 6: GitHub-Remote + erste grüne Pipelines

**Voraussetzung (User-Aktion, siehe offene Frage):** Ein GitHub-Repo `claudedeck` existiert und der lokale `gh`-Token darf darauf pushen. Der aktuelle fine-grained PAT (Account `Edgynode`) kann **keine Repos anlegen** — der User muss entweder das Repo im Browser anlegen und es dem PAT freigeben, oder den PAT um „Administration“ erweitern.

- [ ] **Step 1: Remote setzen und pushen**

```bash
cd /mnt/cache/appdata/claudedeck
git branch -M main
git remote add origin https://github.com/<OWNER>/claudedeck.git
git push -u origin main
```

- [ ] **Step 2: Workflows beobachten**

Run: `gh run watch --repo <OWNER>/claudedeck --exit-status` (je Lauf) bzw. `gh run list --repo <OWNER>/claudedeck`
Expected: `ci` und `build` grün; bei rotem `build` zuerst den Bundle-Pfad-Glob aus Task 4/Step 4 prüfen

- [ ] **Step 3: Artifact-Check (User)**

User lädt `claudedeck-windows` herunter, installiert die `.msi` auf dem Windows-Rechner → leeres ClaudeDeck-Fenster öffnet sich. Portable `claudedeck.exe` startet auch ohne Installation.

---

### Task 7: Spike Teil 1 — Connect, Auth, exec_capture (`tmux ls`)

**Files:**
- Create: `crates/claudedeck-core/examples/spike.rs`

**Interfaces:**
- Produces: `cargo run --example spike -- <host> <user> exec "<cmd>"` — verbindet, authentifiziert per Passwort (`SPIKE_SSH_PASSWORD`), führt Kommando aus, druckt stdout/stderr/exit-code. Validiert die russh-0.62-Grundlagen für das spätere `ssh/connection.rs` + `ssh/exec.rs`.

- [ ] **Step 1: spike.rs mit exec-Modus schreiben**

```rust
//! M1-Spike: validiert russh 0.62 end-to-end gegen isekai.local.
//! Modi:
//!   spike <host> <user> exec "<cmd>"     — Kommando ausführen, Output drucken
//!   spike <host> <user> script           — automatisierter PTY-Test (Task 8)
//!   spike <host> <user> attach <name>    — interaktives tmux-Attach (Task 8, echtes TTY)
use russh::client::{self, AuthResult};
use russh::keys::PublicKey;
use russh::ChannelMsg;
use std::sync::Arc;

struct SpikeHandler;

impl client::Handler for SpikeHandler {
    type Error = russh::Error;

    async fn check_server_key(&mut self, _key: &PublicKey) -> Result<bool, Self::Error> {
        Ok(true) // Spike ohne known_hosts-Prüfung — echte Prüfung kommt in M2 (ssh/hostkey.rs)
    }
}

type Handle = client::Handle<SpikeHandler>;

async fn connect(host: &str, user: &str, password: &str) -> Result<Handle, Box<dyn std::error::Error>> {
    let config = Arc::new(client::Config::default());
    let mut handle = client::connect(config, (host, 22), SpikeHandler).await?;
    let res = handle.authenticate_password(user, password).await?;
    if !matches!(res, AuthResult::Success) {
        return Err("Authentifizierung fehlgeschlagen".into());
    }
    Ok(handle)
}

async fn exec_capture(handle: &Handle, cmd: &str) -> Result<(String, String, u32), Box<dyn std::error::Error>> {
    let mut channel = handle.channel_open_session().await?;
    channel.exec(true, cmd).await?;
    let (mut out, mut err, mut code) = (Vec::new(), Vec::new(), 0u32);
    while let Some(msg) = channel.wait().await {
        match msg {
            ChannelMsg::Data { ref data } => out.extend_from_slice(data),
            ChannelMsg::ExtendedData { ref data, .. } => err.extend_from_slice(data),
            ChannelMsg::ExitStatus { exit_status } => code = exit_status,
            _ => {}
        }
    }
    Ok((String::from_utf8_lossy(&out).into_owned(), String::from_utf8_lossy(&err).into_owned(), code))
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args: Vec<String> = std::env::args().collect();
    let (host, user, mode) = (args.get(1), args.get(2), args.get(3));
    let (host, user, mode) = match (host, user, mode) {
        (Some(h), Some(u), Some(m)) => (h.clone(), u.clone(), m.clone()),
        _ => return Err("Usage: spike <host> <user> exec|script|attach [arg]".into()),
    };
    let password = std::env::var("SPIKE_SSH_PASSWORD").map_err(|_| "SPIKE_SSH_PASSWORD fehlt")?;
    let handle = connect(&host, &user, &password).await?;
    match mode.as_str() {
        "exec" => {
            let cmd = args.get(4).ok_or("exec braucht ein Kommando")?;
            let (out, err, code) = exec_capture(&handle, cmd).await?;
            print!("{out}");
            eprint!("{err}");
            println!("[exit {code}]");
        }
        other => return Err(format!("Modus {other} kommt in Task 8").into()),
    }
    Ok(())
}
```

- [ ] **Step 2: Kompilieren und gegen isekai.local laufen lassen**

Run: `./dev.sh cargo run --example spike -- 192.168.0.161 root exec "tmux ls"`
Expected: entweder eine Session-Liste oder `no server running on /tmp/tmux-0/default` + `[exit 1]` — beides beweist: TCP + Auth + exec + Exit-Code funktionieren. Bei Compile-Fehlern: Signaturen gegen docs.rs/russh/0.62.3 abgleichen (z.B. `AuthResult`-Varianten, `check_server_key`-Signatur) und anpassen — genau dafür existiert der Spike.

- [ ] **Step 3: clippy/fmt sauber, Commit**

Run: `./dev.sh cargo fmt --all && ./dev.sh cargo clippy -p claudedeck-core --all-targets -- -D warnings`

```bash
git add crates/claudedeck-core/examples/spike.rs
git commit -m "feat: russh-Spike Teil 1 — connect/auth/exec_capture gegen isekai validiert"
```

---

### Task 8: Spike Teil 2 — PTY-Attach (script-Modus automatisiert, attach-Modus interaktiv)

**Files:**
- Modify: `crates/claudedeck-core/examples/spike.rs`

**Interfaces:**
- Produces: validiertes PTY-Muster (request_pty → exec → `make_writer` + `wait()`-Loop → `window_change`) — die Blaupause für `ssh/pty.rs` in M2. `script`-Modus liefert Exit 0 bei Erfolg (CI-/agentfähig), `attach`-Modus ist das manuelle Abnahme-Werkzeug.

- [ ] **Step 1: PTY-Helfer + script-Modus ergänzen**

In `spike.rs` ergänzen (vor `main`):

```rust
use tokio::io::AsyncWriteExt;
use tokio::time::{sleep, timeout, Duration};

/// Öffnet ein PTY und führt `cmd` darin aus. Gibt den Channel zurück.
async fn open_pty(
    handle: &Handle,
    cmd: &str,
    cols: u32,
    rows: u32,
) -> Result<russh::Channel<client::Msg>, Box<dyn std::error::Error>> {
    let channel = handle.channel_open_session().await?;
    channel
        .request_pty(false, "xterm-256color", cols, rows, 0, 0, &[])
        .await?;
    channel.exec(true, cmd).await?;
    Ok(channel)
}

/// Automatisierter PTY-Beweis: tmux-Session anlegen, attachen, Marker echoen, Marker im
/// PTY-Output wiederfinden. Exit 0 = PASS.
async fn script_mode(handle: &Handle) -> Result<(), Box<dyn std::error::Error>> {
    let marker = "SPIKE-OK-1337";
    // Session idempotent & detached anlegen (Start = exec, Anzeigen = PTY — Spec-Regel)
    exec_capture(handle, "tmux new-session -A -d -s cc-spike").await?;
    let mut channel = open_pty(handle, "tmux attach -t cc-spike", 100, 30).await?;
    let mut writer = channel.make_writer();

    sleep(Duration::from_millis(700)).await; // tmux Zeit zum Zeichnen geben
    writer.write_all(format!("echo {marker}\r").as_bytes()).await?;
    writer.flush().await?;

    let mut seen = Vec::new();
    let found = timeout(Duration::from_secs(10), async {
        while let Some(msg) = channel.wait().await {
            if let ChannelMsg::Data { ref data } = msg {
                seen.extend_from_slice(data);
                // Marker muss als Echo-OUTPUT auftauchen (Zeilenanfang), nicht nur als Tipp-Echo
                if String::from_utf8_lossy(&seen).matches(marker).count() >= 2 {
                    return true;
                }
            }
        }
        false
    })
    .await
    .unwrap_or(false);

    exec_capture(handle, "tmux kill-session -t cc-spike").await.ok();
    if found {
        println!("SPIKE PASS — PTY-Streaming über russh funktioniert");
        Ok(())
    } else {
        println!("--- empfangener Output ---\n{}", String::from_utf8_lossy(&seen));
        Err("SPIKE FAIL — Marker nicht im PTY-Output gefunden".into())
    }
}
```

- [ ] **Step 2: attach-Modus (interaktiv) ergänzen**

```rust
/// Interaktives Attach mit Raw-Terminal. Detach lokal mit Strg+] (0x1D).
async fn attach_mode(handle: &Handle, name: &str) -> Result<(), Box<dyn std::error::Error>> {
    use crossterm::terminal;
    use tokio::io::AsyncReadExt;

    let (cols, rows) = terminal::size().unwrap_or((100, 30));
    exec_capture(handle, &format!("tmux new-session -A -d -s {name}")).await?;
    let mut channel = open_pty(handle, &format!("tmux attach -t {name}"), cols as u32, rows as u32).await?;
    let mut writer = channel.make_writer();

    terminal::enable_raw_mode()?;
    let result: Result<(), Box<dyn std::error::Error>> = async {
        let mut stdin = tokio::io::stdin();
        let mut stdout = tokio::io::stdout();
        let mut buf = [0u8; 4096];
        loop {
            tokio::select! {
                msg = channel.wait() => match msg {
                    Some(ChannelMsg::Data { ref data }) => {
                        stdout.write_all(data).await?;
                        stdout.flush().await?;
                    }
                    Some(ChannelMsg::ExitStatus { .. }) | Some(ChannelMsg::Eof) | None => break,
                    _ => {}
                },
                n = stdin.read(&mut buf) => {
                    let n = n?;
                    if n == 0 || buf[..n].contains(&0x1D) { break; } // Strg+] = lokales Detach
                    writer.write_all(&buf[..n]).await?;
                    writer.flush().await?;
                }
            }
        }
        Ok(())
    }
    .await;
    terminal::disable_raw_mode()?;
    println!("\r\n[detached]");
    result
}
```

In `main` den `match` erweitern:
```rust
        "script" => script_mode(&handle).await?,
        "attach" => {
            let name = args.get(4).map(String::as_str).unwrap_or("cc-spike");
            attach_mode(&handle, name).await?;
        }
```
Und `crossterm` von `[dev-dependencies]` her nutzen (Examples dürfen dev-dependencies verwenden — kein Cargo.toml-Umbau nötig).

- [ ] **Step 3: Automatisierte Verifikation (agentfähig, kein TTY nötig)**

Run: `./dev.sh cargo run --example spike -- 192.168.0.161 root script`
Expected: `SPIKE PASS — PTY-Streaming über russh funktioniert`, Exit 0. Danach existiert `cc-spike` auf Isekai **nicht** mehr (`tmux ls` prüfen).

- [ ] **Step 4: Reattach-Semantik verifizieren (Kern-Feature der App!)**

```bash
./dev.sh cargo run --example spike -- 192.168.0.161 root exec "tmux new-session -A -d -s cc-spike 'watch -n1 date'"
./dev.sh cargo run --example spike -- 192.168.0.161 root script || true   # attach an laufende Session
./dev.sh cargo run --example spike -- 192.168.0.161 root exec "tmux ls"
./dev.sh cargo run --example spike -- 192.168.0.161 root exec "tmux kill-session -t cc-spike"
```
Expected: `tmux ls` zeigt `cc-spike` auch NACH dem Channel-Close aus dem script-Lauf — beweist: Channel schließen ≠ Session töten.

- [ ] **Step 5: Interaktive Abnahme (manuell, User oder via `! `-Kommando mit echtem TTY)**

Run (auf einem echten Terminal, z.B. direkt auf Isekai): `docker run --rm -it -v /mnt/cache/appdata/claudedeck:/work -w /work -e CARGO_TARGET_DIR=/work/.cargo-cache/target -e SPIKE_SSH_PASSWORD=… rust:1-bookworm cargo run --example spike -- 192.168.0.161 root attach`
Expected: interaktive tmux-Session; `htop` rendert farbig und korrekt; Strg+C erreicht die Remote-Seite; Strg+] detacht lokal; erneuter Aufruf attacht an dieselbe Session.

- [ ] **Step 6: Commit + Meilenstein-Abschluss**

```bash
./dev.sh cargo fmt --all && ./dev.sh cargo clippy -p claudedeck-core --all-targets -- -D warnings
git add crates/claudedeck-core/examples/spike.rs
git commit -m "feat: russh-Spike Teil 2 — PTY attach/script-Modus, Reattach-Semantik validiert"
git push
```

---

## Danach

M1 abgeschlossen = russh-API validiert. **Erst dann** werden die Folgepläne geschrieben (mit den im Spike bestätigten Signaturen):
- Plan 2: M2+M3 — Core-Module per TDD (parser, commands, hostkey, config, secrets, reconnect) + Integrationstests mit sshd-Container in CI
- Plan 3: M4+M5 — Tauri-IPC-Brücke, TermPool, Sidebar, Badges/Notifications
- Plan 4: M6–M8 — SFTP-Panel, Reconnect-Härtung, Keyring, Release

Referenz für alle Folgepläne: Plan-Agent-Entwurf mit recherchierten Versionen und Modul-Schnittstellen (im Spec-Ordner abgelegt als `2026-07-22-claudedeck-masterplan-referenz.md`).
