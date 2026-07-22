# ClaudeDeck M2+M3 (Core-Module + Integrationstests) Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Alle tauri-freien Core-Module von claudedeck-core per TDD (tmux-Parser/Kommandos, hostkey, config, secrets, reconnect, ssh-Verbindungsschicht aus dem Spike refaktoriert) plus Integrationstests, die in CI gegen einen echten sshd-Container laufen.

**Architecture:** claudedeck-core bleibt tauri-frei und linux-testbar. Der validierte Spike-Code (examples/spike.rs) wird in Module zerlegt (`ssh/connection.rs`, `ssh/pty.rs`, `ssh/exec.rs`); der Spike selbst bleibt als dünner Konsument der Module bestehen (beweist, dass die Refaktorierung nichts bricht). Pure Logik (Parser, Kommando-Builder, Backoff, Pfade) ist unit-getestet; alles Netzwerkige läuft als `#[ignore]`-Integrationstests, lokal gegen Isekai und in CI gegen einen sshd-Service-Container.

**Tech Stack:** Rust (russh =0.62.3 — API in M1 live validiert, tokio 1, thiserror 2, keyring 3.6, serde 1, dirs 6), GitHub Actions Service-Container `lscr.io/linuxserver/openssh-server`.

## Global Constraints

- Spec: `docs/superpowers/specs/2026-07-21-claudedeck-design.md`; Referenz: `docs/superpowers/specs/2026-07-22-claudedeck-masterplan-referenz.md`
- Repo `/mnt/cache/appdata/claudedeck`, Rust NUR über `./dev.sh cargo …`; NIEMALS unter /root arbeiten
- `russh = "=0.62.3"` bleibt gepinnt; Muster aus dem validierten Spike übernehmen (Handler nativ async, `AuthResult::Success`, `channel.wait()`-Loop, `make_writer()`)
- tmux-Targets IMMER exakt mit `-t =name` (tmux matcht sonst per Präfix — Review-Finding M1)
- Session-/Pfad-Werte NIE unquoted in Shell-Strings interpolieren — ausschließlich über `tmux::commands` mit getestetem `shell_quote`
- `ExecOutput.exit_code` ist `Option<u32>` — ein abrupt geschlossener Channel liefert keinen ExitStatus (Review-Finding M1)
- Remote-Pfade sind reine `/`-Strings, niemals `std::path` für Remote-Seite
- Kein Auto-Retry nach fehlgeschlagener Auth (fail2ban); Verifikationen gegen Isekai max. 2 Auth-Versuche
- Integrationstests: `#[ignore]`, konfiguriert über Env `CLAUDEDECK_TEST_SSH=host:port:user:pass`; lokal = `192.168.0.161:22:root:$SPIKE_SSH_PASSWORD`
- Commits klein, Deutsch, `feat:`/`test:`/`chore:`/`ci:`; TDD strikt (FAIL zeigen → implementieren → PASS zeigen)
- Neue Dependencies nur die hier genannten: `thiserror`, `keyring` (features windows-native + linux-native), `serde`+`serde_json`, `dirs`; dev: `tempfile`

---

### Task 1: Dev-Image mit rustfmt/clippy (Tooling-Fix aus M1)

**Files:**
- Create: `Dockerfile.dev`
- Modify: `dev.sh`

**Interfaces:**
- Produces: `./dev.sh …` wie bisher, aber mit Image `claudedeck-dev` (enthält rustfmt+clippy dauerhaft, plus `-t` bei echtem TTY). Alle Folge-Tasks nutzen das.

- [ ] **Step 1: Dockerfile.dev**

```dockerfile
FROM rust:1-bookworm
RUN rustup component add rustfmt clippy
```

- [ ] **Step 2: dev.sh erweitern**

Nach der `mkdir`-Zeile einfügen (vor dem `[ -f …secrets.env ]`-Block):
```bash
docker image inspect claudedeck-dev >/dev/null 2>&1 || \
  docker build -q -t claudedeck-dev -f "$DIR/Dockerfile.dev" "$DIR"
TTY_FLAG=""
[ -t 0 ] && TTY_FLAG="-t"
```
In der `docker run`-Zeile `rust:1-bookworm` durch `claudedeck-dev` ersetzen und `-i` durch `-i $TTY_FLAG` (unquoted, damit leer verschwindet — dafür in der exec-Zeile `$TTY_FLAG` direkt nach `--rm -i` einsetzen).

- [ ] **Step 3: Verifizieren**

Run: `./dev.sh cargo fmt --all --check && ./dev.sh cargo clippy -p claudedeck-core --all-targets -- -D warnings`
Expected: beide Exit 0 OHNE vorheriges `rustup component add` (Image-Build beim ersten Aufruf ist OK)

- [ ] **Step 4: Commit**

```bash
git add Dockerfile.dev dev.sh
git commit -m "chore: Dev-Image claudedeck-dev mit rustfmt/clippy + TTY-Durchreichung"
```

---

### Task 2: tmux::commands — Kommando-Builder mit Quoting (TDD)

**Files:**
- Create: `crates/claudedeck-core/src/tmux/commands.rs`
- Modify: `crates/claudedeck-core/src/tmux/mod.rs` (`pub mod commands;` ergänzen)

**Interfaces:**
- Consumes: nichts (pur).
- Produces:
  - `shell_quote(s: &str) -> String` (POSIX-Single-Quote-Escaping: `'` → `'\''`, Ergebnis immer gequotet)
  - `cmd_list_sessions() -> &'static str`
  - `cmd_list_panes() -> &'static str`
  - `cmd_new_detached(name: &str, cwd: &str, command: &str) -> String`
  - `cmd_attach(name: &str) -> String`
  - `cmd_kill(name: &str) -> String`
  - `cmd_has_session(name: &str) -> String`
  - `cmd_pane_cwd(name: &str) -> String`
  - `cmd_scan_projects(paths: &[String]) -> String`

- [ ] **Step 1: Failing Tests schreiben** (`commands.rs` mit `todo!()`-Bodies + Tests)

```rust
//! Baut tmux-Kommandozeilen. Einzige Stelle im Projekt, die Shell-Strings zusammensetzt —
//! alle Werte laufen durch shell_quote, Targets sind mit `=` exakt (tmux matcht sonst Präfixe).

/// POSIX-sicheres Single-Quoting: immer gequotet, eingebettete ' als '\''.
pub fn shell_quote(s: &str) -> String {
    todo!()
}

pub fn cmd_list_sessions() -> &'static str {
    "tmux list-sessions -F '#{session_name}\t#{session_created}\t#{session_attached}' 2>/dev/null || true"
}

pub fn cmd_list_panes() -> &'static str {
    "tmux list-panes -a -F '#{session_name}\t#{pane_current_command}\t#{pane_current_path}' 2>/dev/null || true"
}

pub fn cmd_new_detached(name: &str, cwd: &str, command: &str) -> String {
    todo!()
}

pub fn cmd_attach(name: &str) -> String {
    todo!()
}

pub fn cmd_kill(name: &str) -> String {
    todo!()
}

pub fn cmd_has_session(name: &str) -> String {
    todo!()
}

pub fn cmd_pane_cwd(name: &str) -> String {
    todo!()
}

pub fn cmd_scan_projects(paths: &[String]) -> String {
    todo!()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn quote_einfacher_string() {
        assert_eq!(shell_quote("abc"), "'abc'");
    }

    #[test]
    fn quote_mit_leerzeichen_und_dollar() {
        assert_eq!(shell_quote("a b$c"), "'a b$c'");
    }

    #[test]
    fn quote_mit_single_quote() {
        assert_eq!(shell_quote("it's"), r"'it'\''s'");
    }

    #[test]
    fn new_detached_quotet_alles_und_nutzt_a_d() {
        assert_eq!(
            cmd_new_detached("cc-x", "/mnt/cache/app data", "claude"),
            "tmux new-session -A -d -s 'cc-x' -c '/mnt/cache/app data' 'claude'"
        );
    }

    #[test]
    fn attach_nutzt_exaktes_target() {
        assert_eq!(cmd_attach("cc-x"), "tmux attach -t '=cc-x'");
    }

    #[test]
    fn kill_nutzt_exaktes_target() {
        assert_eq!(cmd_kill("cc-x"), "tmux kill-session -t '=cc-x'");
    }

    #[test]
    fn has_session_nutzt_exaktes_target() {
        assert_eq!(cmd_has_session("cc-x"), "tmux has-session -t '=cc-x'");
    }

    #[test]
    fn pane_cwd_nutzt_exaktes_target() {
        assert_eq!(
            cmd_pane_cwd("cc-x"),
            "tmux display -p -t '=cc-x' '#{pane_current_path}'"
        );
    }

    #[test]
    fn scan_projects_joint_mehrere_pfade() {
        assert_eq!(
            cmd_scan_projects(&["/mnt/a".into(), "/mnt/b c".into()]),
            "find '/mnt/a' '/mnt/b c' -mindepth 1 -maxdepth 1 -type d 2>/dev/null | sort"
        );
    }
}
```

- [ ] **Step 2: FAIL zeigen**

Run: `./dev.sh cargo test -p claudedeck-core commands`
Expected: Tests panicken mit `not yet implemented` (die beiden `&'static str`-Fns testfrei OK)

- [ ] **Step 3: Implementieren**

```rust
pub fn shell_quote(s: &str) -> String {
    let mut out = String::with_capacity(s.len() + 2);
    out.push('\'');
    for c in s.chars() {
        if c == '\'' {
            out.push_str(r"'\''");
        } else {
            out.push(c);
        }
    }
    out.push('\'');
    out
}

pub fn cmd_new_detached(name: &str, cwd: &str, command: &str) -> String {
    format!(
        "tmux new-session -A -d -s {} -c {} {}",
        shell_quote(name),
        shell_quote(cwd),
        shell_quote(command)
    )
}

pub fn cmd_attach(name: &str) -> String {
    format!("tmux attach -t {}", shell_quote(&format!("={name}")))
}

pub fn cmd_kill(name: &str) -> String {
    format!("tmux kill-session -t {}", shell_quote(&format!("={name}")))
}

pub fn cmd_has_session(name: &str) -> String {
    format!("tmux has-session -t {}", shell_quote(&format!("={name}")))
}

pub fn cmd_pane_cwd(name: &str) -> String {
    format!(
        "tmux display -p -t {} '#{{pane_current_path}}'",
        shell_quote(&format!("={name}"))
    )
}

pub fn cmd_scan_projects(paths: &[String]) -> String {
    let quoted: Vec<String> = paths.iter().map(|p| shell_quote(p)).collect();
    format!(
        "find {} -mindepth 1 -maxdepth 1 -type d 2>/dev/null | sort",
        quoted.join(" ")
    )
}
```

- [ ] **Step 4: PASS zeigen**

Run: `./dev.sh cargo test -p claudedeck-core commands`
Expected: 9 Tests PASS

- [ ] **Step 5: Commit**

```bash
git add crates/claudedeck-core/src/tmux
git commit -m "feat: tmux::commands — gequotete Kommando-Builder mit =-exakten Targets (TDD)"
```

---

### Task 3: tmux::parser — Session-/Pane-Parsing + Merge (TDD)

**Files:**
- Create: `crates/claudedeck-core/src/tmux/parser.rs`
- Modify: `crates/claudedeck-core/src/tmux/mod.rs` (`pub mod parser;`)

**Interfaces:**
- Consumes: Output der Kommandos aus Task 2.
- Produces:
```rust
pub struct RawSession { pub name: String, pub created: i64, pub attached: u32 }
pub struct RawPane { pub session: String, pub command: String, pub cwd: String }
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SessionKind { Claude, Shell }
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SessionInfo { pub name: String, pub kind: SessionKind, pub cwd: String, pub attached: bool, pub created: i64, pub managed: bool }
pub fn parse_sessions(out: &str) -> Vec<RawSession>;
pub fn parse_panes(out: &str) -> Vec<RawPane>;
pub fn merge(sessions: Vec<RawSession>, panes: Vec<RawPane>) -> Vec<SessionInfo>;
```

- [ ] **Step 1: Failing Tests** (im Modul, Fns mit `todo!()`)

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_sessions_leerer_output() {
        assert!(parse_sessions("").is_empty());
        assert!(parse_sessions("\n").is_empty());
    }

    #[test]
    fn parse_sessions_no_server_stderr_wird_ignoriert() {
        // exec liefert stdout getrennt; falls doch die Fehlzeile ankommt: keine Panik, leeres Ergebnis
        assert!(parse_sessions("no server running on /tmp/tmux-0/default").is_empty());
    }

    #[test]
    fn parse_sessions_zwei_zeilen() {
        let out = "cc-otakupulse\t1753100000\t1\nshell 1\t1753100100\t0\n";
        let s = parse_sessions(out);
        assert_eq!(s.len(), 2);
        assert_eq!(s[0].name, "cc-otakupulse");
        assert_eq!(s[0].created, 1753100000);
        assert_eq!(s[0].attached, 1);
        assert_eq!(s[1].name, "shell 1"); // Name mit Leerzeichen bleibt intakt (tab-separiert!)
        assert_eq!(s[1].attached, 0);
    }

    #[test]
    fn parse_panes_grundfall() {
        let out = "cc-x\tclaude\t/mnt/cache/appdata/x\ncc-x\tbash\t/root\n";
        let p = parse_panes(out);
        assert_eq!(p.len(), 2);
        assert_eq!(p[0].command, "claude");
        assert_eq!(p[1].cwd, "/root");
    }

    #[test]
    fn merge_claude_pane_gewinnt() {
        let sessions = vec![RawSession { name: "cc-x".into(), created: 1, attached: 0 }];
        let panes = vec![
            RawPane { session: "cc-x".into(), command: "bash".into(), cwd: "/a".into() },
            RawPane { session: "cc-x".into(), command: "claude".into(), cwd: "/b".into() },
        ];
        let m = merge(sessions, panes);
        assert_eq!(m[0].kind, SessionKind::Claude);
        assert_eq!(m[0].cwd, "/b"); // cwd der ERSTEN claude-Pane
    }

    #[test]
    fn merge_node_ist_kein_claude() {
        let sessions = vec![RawSession { name: "s".into(), created: 1, attached: 0 }];
        let panes = vec![RawPane { session: "s".into(), command: "node".into(), cwd: "/a".into() }];
        assert_eq!(merge(sessions, panes)[0].kind, SessionKind::Shell);
    }

    #[test]
    fn merge_setzt_managed_und_attached() {
        let sessions = vec![
            RawSession { name: "cc-x".into(), created: 1, attached: 2 },
            RawSession { name: "manuell".into(), created: 2, attached: 0 },
        ];
        let m = merge(sessions, vec![]);
        assert!(m[0].managed && m[0].attached);
        assert!(!m[1].managed && !m[1].attached);
        assert_eq!(m[0].cwd, ""); // keine Pane-Info → leerer cwd
    }

    #[test]
    fn parse_toleriert_kaputte_zeilen() {
        // zu wenig Felder oder nicht-numerisch → Zeile überspringen, kein Panic
        let out = "nur-ein-feld\ncc-ok\t123\t0\nfoo\tbar\tbaz\n";
        let s = parse_sessions(out);
        assert_eq!(s.len(), 1);
        assert_eq!(s[0].name, "cc-ok");
    }
}
```

- [ ] **Step 2: FAIL zeigen** — `./dev.sh cargo test -p claudedeck-core parser` → panics

- [ ] **Step 3: Implementieren**

```rust
pub fn parse_sessions(out: &str) -> Vec<RawSession> {
    out.lines()
        .filter_map(|l| {
            let mut f = l.split('\t');
            let name = f.next()?.to_string();
            let created: i64 = f.next()?.parse().ok()?;
            let attached: u32 = f.next()?.parse().ok()?;
            Some(RawSession { name, created, attached })
        })
        .collect()
}

pub fn parse_panes(out: &str) -> Vec<RawPane> {
    out.lines()
        .filter_map(|l| {
            let mut f = l.split('\t');
            Some(RawPane {
                session: f.next()?.to_string(),
                command: f.next()?.to_string(),
                cwd: f.next()?.to_string(),
            })
        })
        .collect()
}

pub fn merge(sessions: Vec<RawSession>, panes: Vec<RawPane>) -> Vec<SessionInfo> {
    sessions
        .into_iter()
        .map(|s| {
            let mine: Vec<&RawPane> = panes.iter().filter(|p| p.session == s.name).collect();
            let claude = mine.iter().find(|p| p.command == "claude");
            let kind = if claude.is_some() { SessionKind::Claude } else { SessionKind::Shell };
            let cwd = claude
                .or(mine.first())
                .map(|p| p.cwd.clone())
                .unwrap_or_default();
            SessionInfo {
                managed: s.name.starts_with("cc-"),
                attached: s.attached > 0,
                name: s.name,
                kind,
                cwd,
                created: s.created,
            }
        })
        .collect()
}
```
(Struct-Definitionen wie unter "Interfaces"; `parse_panes`-Zeile mit nur einem Feld liefert `command`/`cwd` als None → wird durch `?` übersprungen. Achtung Feldreihenfolge im Struct-Literal vs. Moves — `name: s.name` zuletzt bei `kind`-Borrow.)

- [ ] **Step 4: PASS zeigen** — `./dev.sh cargo test -p claudedeck-core parser` → 8 Tests PASS

- [ ] **Step 5: Commit**

```bash
git add crates/claudedeck-core/src/tmux
git commit -m "feat: tmux::parser — Sessions/Panes parsen und zu SessionInfo mergen (TDD)"
```

---

### Task 4: ssh::hostkey — known_hosts-Prüfung (TDD)

**Files:**
- Create: `crates/claudedeck-core/src/ssh/mod.rs` (`pub mod hostkey;`)
- Create: `crates/claudedeck-core/src/ssh/hostkey.rs`
- Modify: `crates/claudedeck-core/src/lib.rs` (`pub mod ssh;`)
- Modify: `crates/claudedeck-core/Cargo.toml` (dev-dependency `tempfile = "3"` und dependency `data-encoding = "2"`)

**Interfaces:**
- Produces:
```rust
#[derive(Debug, PartialEq, Eq)]
pub enum HostkeyStatus {
    Known,
    Unknown { fingerprint: String },
    Changed { fingerprint: String },
}
pub fn check(known_hosts: &std::path::Path, host: &str, port: u16, key: &russh::keys::PublicKey) -> HostkeyStatus;
pub fn append(known_hosts: &std::path::Path, host: &str, port: u16, key: &russh::keys::PublicKey) -> std::io::Result<()>;
pub fn fingerprint_sha256(key: &russh::keys::PublicKey) -> String; // "SHA256:<base64-ohne-padding>"
```

- [ ] **Step 1: Failing Tests**

Testschlüssel im Test generieren: `russh::keys::PrivateKey::random(&mut rand_core::OsRng, russh::keys::Algorithm::Ed25519)` — falls diese API nicht existiert, per docs.rs/russh/0.62.3 die aktuelle Schlüsselgenerierung nachschlagen (`ssh_key::private::PrivateKey::random` Re-Export); Fallback: zwei feste Ed25519-known_hosts-Zeilen als String-Fixtures einchecken und `PublicKey::from_openssh` (ssh_key-Re-Export) zum Laden nutzen — dann braucht es keine Generierung.

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;

    // Zwei echte, feste Ed25519-Testschlüssel (nur Testdaten, keine Secrets):
    const KEY_A: &str = "AAAAC3NzaC1lZDI1NTE5AAAAIGb0eNSXSGcE8YG5RuRhZs2NM4Z2zAtxKT9d6sPCLsdE";
    const KEY_B: &str = "AAAAC3NzaC1lZDI1NTE5AAAAIODJol6WSDGaX8DJHfF9O5B84vLdU21LMc0dGE0hMh8I";

    fn pk(b64: &str) -> russh::keys::PublicKey {
        russh::keys::PublicKey::from_openssh(&format!("ssh-ed25519 {b64} test")).unwrap()
    }

    fn kh(content: &str) -> tempfile::NamedTempFile {
        let mut f = tempfile::NamedTempFile::new().unwrap();
        f.write_all(content.as_bytes()).unwrap();
        f
    }

    #[test]
    fn unbekannter_host_liefert_unknown_mit_fingerprint() {
        let f = kh("");
        let st = check(f.path(), "isekai.local", 22, &pk(KEY_A));
        match st {
            HostkeyStatus::Unknown { fingerprint } => assert!(fingerprint.starts_with("SHA256:")),
            other => panic!("erwartet Unknown, war {other:?}"),
        }
    }

    #[test]
    fn bekannter_host_liefert_known() {
        let f = kh(&format!("isekai.local ssh-ed25519 {KEY_A}\n"));
        assert_eq!(check(f.path(), "isekai.local", 22, &pk(KEY_A)), HostkeyStatus::Known);
    }

    #[test]
    fn geaenderter_key_liefert_changed() {
        let f = kh(&format!("isekai.local ssh-ed25519 {KEY_A}\n"));
        match check(f.path(), "isekai.local", 22, &pk(KEY_B)) {
            HostkeyStatus::Changed { .. } => {}
            other => panic!("erwartet Changed, war {other:?}"),
        }
    }

    #[test]
    fn nichtstandard_port_nutzt_klammer_notation() {
        let f = kh(&format!("[isekai.local]:2222 ssh-ed25519 {KEY_A}\n"));
        assert_eq!(check(f.path(), "isekai.local", 2222, &pk(KEY_A)), HostkeyStatus::Known);
    }

    #[test]
    fn append_schreibt_und_check_findet() {
        let f = kh("");
        append(f.path(), "neu.local", 2222, &pk(KEY_A)).unwrap();
        assert_eq!(check(f.path(), "neu.local", 2222, &pk(KEY_A)), HostkeyStatus::Known);
        let content = std::fs::read_to_string(f.path()).unwrap();
        assert!(content.contains("[neu.local]:2222 ssh-ed25519 "));
    }

    #[test]
    fn fehlende_datei_ist_unknown_nicht_panic() {
        let st = check(std::path::Path::new("/nonexistent/known_hosts"), "x", 22, &pk(KEY_A));
        assert!(matches!(st, HostkeyStatus::Unknown { .. }));
    }
}
```

- [ ] **Step 2: FAIL zeigen** — `./dev.sh cargo test -p claudedeck-core hostkey`

- [ ] **Step 3: Implementieren**

Basis: `russh::keys::check_known_hosts_path(host, port, key, path)` → `Ok(true)`=Known, `Ok(false)`=Unknown, `Err(russh::keys::Error::KeyChanged{..})`=Changed; jeder andere Err (Datei fehlt) → Unknown. Fingerprint: `key.fingerprint(Default::default()).to_string()` liefert bereits "SHA256:…" (ssh_key-API; falls Signatur abweicht: docs.rs). `append`: Zeile `host ssh-ed25519 <b64>` bzw. `[host]:port …` bei Port ≠ 22, via `key.to_openssh()`-Äquivalent (`PublicKey::to_string()` liefert "ssh-ed25519 <b64> comment"); Datei mit `OpenOptions::append(true).create(true)`, sicherstellen dass mit `\n` terminiert.

```rust
use russh::keys::PublicKey;
use std::io::Write;
use std::path::Path;

pub fn fingerprint_sha256(key: &PublicKey) -> String {
    key.fingerprint(Default::default()).to_string()
}

pub fn check(known_hosts: &Path, host: &str, port: u16, key: &PublicKey) -> HostkeyStatus {
    match russh::keys::check_known_hosts_path(host, port, key, known_hosts) {
        Ok(true) => HostkeyStatus::Known,
        Ok(false) => HostkeyStatus::Unknown { fingerprint: fingerprint_sha256(key) },
        Err(russh::keys::Error::KeyChanged { .. }) => {
            HostkeyStatus::Changed { fingerprint: fingerprint_sha256(key) }
        }
        Err(_) => HostkeyStatus::Unknown { fingerprint: fingerprint_sha256(key) },
    }
}

pub fn append(known_hosts: &Path, host: &str, port: u16, key: &PublicKey) -> std::io::Result<()> {
    if let Some(dir) = known_hosts.parent() {
        std::fs::create_dir_all(dir)?;
    }
    let host_field = if port == 22 { host.to_string() } else { format!("[{host}]:{port}") };
    let key_str = key.to_openssh().map_err(std::io::Error::other)?;
    let mut f = std::fs::OpenOptions::new().append(true).create(true).open(known_hosts)?;
    writeln!(f, "{host_field} {key_str}")?;
    Ok(())
}
```
(Exakte ssh_key-Methodennamen — `to_openssh`, `fingerprint(HashAlg)` — beim Kompilieren gegen docs.rs verifizieren; Funktionalität bindend, Signaturen flexibel. Falls `check_known_hosts_path` die `[host]:port`-Notation nicht selbst matcht: eigenen Zeilen-Vergleich implementieren, Testfälle bleiben unverändert.)

- [ ] **Step 4: PASS zeigen** — 6 Tests PASS

- [ ] **Step 5: Commit**

```bash
git add crates/claudedeck-core
git commit -m "feat: ssh::hostkey — known_hosts-Prüfung mit Known/Unknown/Changed (TDD)"
```

---

### Task 5: config + reconnect (TDD)

**Files:**
- Create: `crates/claudedeck-core/src/config.rs`, `crates/claudedeck-core/src/reconnect.rs`
- Modify: `crates/claudedeck-core/src/lib.rs` (`pub mod config; pub mod reconnect;`)
- Modify: `crates/claudedeck-core/Cargo.toml` (dependencies `serde = { version = "1", features = ["derive"] }`, `serde_json = "1"`, `dirs = "6"`)

**Interfaces:**
- Produces:
```rust
// config.rs
#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)] pub struct Config { … }  // mit #[serde(default)] überall
pub struct Profile { pub host: String, pub port: u16, pub user: String, pub auth: AuthMethod, pub key_path: Option<String> }
pub enum AuthMethod { Key, Password }
pub struct NotifySettings { pub enabled: bool, pub silence_ms: u64 }
pub fn config_path() -> std::path::PathBuf;                      // dirs::config_dir()/claudedeck/config.json
pub fn load_from(path: &Path) -> Config;                          // fehlend/kaputt → Default
pub fn save_to(path: &Path, cfg: &Config) -> std::io::Result<()>;
// Defaults: host "isekai.local", port 22, user "root", auth Password, scan_paths ["/mnt/cache/appdata"],
// favorites [], notifications { enabled: true, silence_ms: 2000 }
// reconnect.rs
pub fn backoff_schedule() -> impl Iterator<Item = std::time::Duration>;  // 3,6,12,30,30,30,…
```

- [ ] **Step 1: Failing Tests**

config-Tests: (a) `load_from` auf nicht-existentem Pfad → kompletter Default (alle Felder prüfen), (b) Roundtrip save→load == Original, (c) Teil-JSON `{"profile":{"host":"other"}}` → host "other", Rest Default (serde-defaults greifen feldweise), (d) kaputtes JSON → Default statt Panic.
reconnect-Tests: `backoff_schedule().take(6)` == `[3,6,12,30,30,30]` Sekunden.

```rust
// in reconnect.rs
#[cfg(test)]
mod tests {
    use super::*;
    use std::time::Duration;

    #[test]
    fn backoff_ist_3_6_12_dann_dauerhaft_30() {
        let v: Vec<Duration> = backoff_schedule().take(6).collect();
        assert_eq!(v, vec![
            Duration::from_secs(3), Duration::from_secs(6), Duration::from_secs(12),
            Duration::from_secs(30), Duration::from_secs(30), Duration::from_secs(30),
        ]);
    }
}
```

- [ ] **Step 2: FAIL zeigen** — `./dev.sh cargo test -p claudedeck-core 'config|reconnect'` (oder zwei Läufe)

- [ ] **Step 3: Implementieren**

```rust
// reconnect.rs
pub fn backoff_schedule() -> impl Iterator<Item = std::time::Duration> {
    [3u64, 6, 12].into_iter().chain(std::iter::repeat(30)).map(std::time::Duration::from_secs)
}
```
config.rs: Structs mit `#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]` und `#[serde(default)]` auf allen Feldern; `impl Default` je Struct mit den o.g. Werten; `load_from` = `fs::read_to_string(path).ok().and_then(|s| serde_json::from_str(&s).ok()).unwrap_or_default()`; `save_to` = `create_dir_all(parent)` + `serde_json::to_string_pretty` + write.

- [ ] **Step 4: PASS zeigen** — 5 Tests PASS

- [ ] **Step 5: Commit**

```bash
git add crates/claudedeck-core
git commit -m "feat: config (serde-Defaults, Roundtrip) + reconnect-Backoff 3/6/12/30 (TDD)"
```

---

### Task 6: secrets — SecretStore-Trait mit Keyring- und Memory-Impl (TDD)

**Files:**
- Create: `crates/claudedeck-core/src/secrets.rs`
- Modify: `crates/claudedeck-core/src/lib.rs` (`pub mod secrets;`)
- Modify: `crates/claudedeck-core/Cargo.toml` (`keyring = { version = "3.6", features = ["windows-native", "linux-native"] }`)

**Interfaces:**
- Produces:
```rust
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SecretKind { Password, KeyPassphrase }
pub trait SecretStore: Send + Sync {
    fn get(&self, profile: &str, kind: SecretKind) -> Option<String>;
    fn set(&self, profile: &str, kind: SecretKind, value: &str) -> Result<(), String>;
    fn delete(&self, profile: &str, kind: SecretKind) -> Result<(), String>;
}
pub struct KeyringStore;   // keyring::Entry::new("claudedeck", &format!("{profile}:{kind:?}"))
pub struct MemoryStore(std::sync::Mutex<std::collections::HashMap<(String, SecretKind), String>>);
impl MemoryStore { pub fn new() -> Self; }
```

- [ ] **Step 1: Failing Tests** — nur gegen `MemoryStore` (KeyringStore ist plattformabhängig, wird in M7 auf Windows abgenommen): set→get liefert Wert; get auf Unbekanntes = None; delete entfernt; Password und KeyPassphrase sind getrennte Slots.

- [ ] **Step 2: FAIL zeigen** — `./dev.sh cargo test -p claudedeck-core secrets`

- [ ] **Step 3: Implementieren** — MemoryStore über die Mutex-HashMap; KeyringStore via `keyring::Entry` (`get_password`/`set_password`/`delete_credential`, Fehler → `Err(e.to_string())` bzw. `None`).

- [ ] **Step 4: PASS zeigen** — 4 Tests PASS. Zusätzlich prüfen: `./dev.sh cargo test -p claudedeck-core` gesamt grün (keyring-Crate muss im Container zumindest KOMPILIEREN; linux-native/keyutils braucht keinen Daemon).

- [ ] **Step 5: Commit**

```bash
git add crates/claudedeck-core
git commit -m "feat: secrets — SecretStore-Trait mit Memory- und Keyring-Implementierung (TDD)"
```

---

### Task 7: ssh-Verbindungsschicht — Spike in Module refaktorieren

**Files:**
- Create: `crates/claudedeck-core/src/ssh/connection.rs`, `ssh/exec.rs`, `ssh/pty.rs`
- Modify: `crates/claudedeck-core/src/ssh/mod.rs` (drei `pub mod` + Re-Exports)
- Modify: `crates/claudedeck-core/src/lib.rs` (falls nötig)
- Modify: `crates/claudedeck-core/examples/spike.rs` (nutzt jetzt NUR noch die Module)
- Modify: `crates/claudedeck-core/Cargo.toml` (`thiserror = "2"` falls noch nicht drin)

**Interfaces:**
- Consumes: validierte Muster aus spike.rs (Commit 48ac979) — Handler, connect, exec-Loop, PTY-Loop, make_writer.
- Produces:
```rust
// connection.rs
pub enum Auth { Password(String), Key { path: std::path::PathBuf, passphrase: Option<String> } }
#[derive(Debug, thiserror::Error)]
pub enum ConnectError {
    #[error("Authentifizierung fehlgeschlagen")] AuthFailed,
    #[error("Host-Key unbekannt: {fingerprint}")] HostkeyUnknown { fingerprint: String },
    #[error("HOST-KEY GEÄNDERT: {fingerprint}")] HostkeyChanged { fingerprint: String },
    #[error(transparent)] Ssh(#[from] russh::Error),
}
pub struct SshConnection { /* handle: russh::client::Handle<ClientHandler> */ }
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HostkeyPolicy { Strict, AcceptNew, InsecureAcceptAll }  // Strict für App, InsecureAcceptAll nur Tests/Spike
pub struct ConnectParams { pub host: String, pub port: u16, pub user: String, pub auth: Auth,
                           pub known_hosts: std::path::PathBuf, pub policy: HostkeyPolicy }
impl SshConnection {
    pub async fn connect(params: ConnectParams) -> Result<Self, ConnectError>;
    pub async fn exec_capture(&self, cmd: &str) -> Result<ExecOutput, russh::Error>;
    pub async fn open_pty(&self, cmd: &str, cols: u32, rows: u32) -> Result<PtyHandle, russh::Error>;
}
// exec.rs
pub struct ExecOutput { pub stdout: String, pub stderr: String, pub exit_code: Option<u32> }
impl ExecOutput { pub fn success(&self) -> bool { self.exit_code == Some(0) } }
// pty.rs
pub enum PtyEvent { Data(Vec<u8>), Exit(Option<u32>) }
pub struct PtyHandle { … }
impl PtyHandle {
    pub async fn write(&mut self, data: &[u8]) -> Result<(), std::io::Error>;   // über make_writer
    pub async fn resize(&self, cols: u32, rows: u32) -> Result<(), russh::Error>; // window_change — NEU, bisher unvalidiert!
    pub fn take_output(&mut self) -> tokio::sync::mpsc::Receiver<PtyEvent>;     // Reader-Task gestartet in open_pty
    pub async fn close(self) -> Result<(), russh::Error>;
}
```
Hostkey-Verhalten in `connect`: Der `ClientHandler` bekommt known_hosts-Pfad + policy + einen `std::sync::Mutex<Option<HostkeyStatus>>`-Slot; `check_server_key` ruft `hostkey::check` und entscheidet: Known→true; Unknown→ bei AcceptNew/InsecureAcceptAll `hostkey::append`+true, bei Strict Status merken+false; Changed→ bei InsecureAcceptAll true, sonst Status merken+false. `connect` übersetzt einen gemerkten Status in `ConnectError::HostkeyUnknown/Changed` (russh liefert bei false einen generischen Fehler — der gemerkte Status ist die präzise Ursache).

- [ ] **Step 1: Module anlegen, Code aus spike.rs verschieben und generalisieren** (exec-Loop → exec.rs-Hilfsfn, PTY-open + Reader-Task mit mpsc (Kanalgröße 256) → pty.rs, connect+Handler → connection.rs). `resize` implementieren: `channel.window_change(cols, rows, 0, 0).await` — Signatur bei Compile-Fehler gegen docs.rs prüfen. Der Reader-Task läuft in `tokio::spawn` und sendet `PtyEvent::Data`/`Exit`; `PtyHandle` hält Writer (`make_writer`) und die Channel-Referenz für resize/close.

- [ ] **Step 2: spike.rs auf die Module umstellen** — exec/script/attach-Modi behalten ihre CLI, rufen aber `SshConnection::connect` (policy InsecureAcceptAll, known_hosts /dev/null), `exec_capture`, `open_pty`+`take_output` auf. Der attach-Modus nutzt jetzt `handle.resize()` bei SIGWINCH (`tokio::signal::unix::signal(SignalKind::window_change())` + `crossterm::terminal::size()`) — damit wird die letzte unvalidierte API real getestet.

- [ ] **Step 3: Kompilieren + Unit-Suite grün**

Run: `./dev.sh cargo test -p claudedeck-core && ./dev.sh cargo clippy -p claudedeck-core --all-targets -- -D warnings`
Expected: alle bisherigen Tests PASS (32+), clippy sauber

- [ ] **Step 4: Spike-Regression gegen Isekai**

Run: `./dev.sh cargo run --example spike -- 192.168.0.161 root script`
Expected: `SPIKE PASS …` — beweist, dass die Refaktorierung das validierte Verhalten erhält

- [ ] **Step 5: Commit**

```bash
git add crates/claudedeck-core
git commit -m "refactor: Spike in ssh::{connection,exec,pty} zerlegt, window_change + Hostkey-Policy ergänzt"
```

---

### Task 8: Integrationstests + CI-Job (M3)

**Files:**
- Create: `crates/claudedeck-core/tests/integration_ssh.rs`
- Modify: `.github/workflows/ci.yml` (integration-Job)

**Interfaces:**
- Consumes: `SshConnection` (Task 7), `tmux::commands` (Task 2), `tmux::parser` (Task 3).
- Produces: in CI laufende End-to-End-Beweise gegen echten sshd; Env-Contract `CLAUDEDECK_TEST_SSH=host:port:user:pass`.

- [ ] **Step 1: integration_ssh.rs schreiben** (alle Tests `#[ignore]`, `#[tokio::test]`)

```rust
//! Integrationstests gegen einen echten sshd. Lokal:
//!   CLAUDEDECK_TEST_SSH=192.168.0.161:22:root:$SPIKE_SSH_PASSWORD ./dev.sh cargo test -p claudedeck-core --test integration_ssh -- --ignored
//! In CI: Service-Container (siehe ci.yml).
use claudedeck_core::ssh::connection::{Auth, ConnectParams, HostkeyPolicy, SshConnection};
use claudedeck_core::tmux::{commands, parser};

fn params() -> ConnectParams {
    let raw = std::env::var("CLAUDEDECK_TEST_SSH").expect("CLAUDEDECK_TEST_SSH fehlt");
    let p: Vec<&str> = raw.splitn(4, ':').collect();
    ConnectParams {
        host: p[0].into(),
        port: p[1].parse().unwrap(),
        user: p[2].into(),
        auth: Auth::Password(p[3].into()),
        known_hosts: std::path::PathBuf::from("/dev/null"),
        policy: HostkeyPolicy::InsecureAcceptAll,
    }
}

const S: &str = "cc-inttest";

async fn cleanup(conn: &SshConnection) {
    let _ = conn.exec_capture(&commands::cmd_kill(S)).await;
}

#[tokio::test]
#[ignore]
async fn exec_liefert_stdout_und_exitcode() {
    let conn = SshConnection::connect(params()).await.unwrap();
    let out = conn.exec_capture("echo hallo && exit 3").await.unwrap();
    assert_eq!(out.stdout.trim(), "hallo");
    assert_eq!(out.exit_code, Some(3));
}

#[tokio::test]
#[ignore]
async fn tmux_roundtrip_liste_und_parser() {
    let conn = SshConnection::connect(params()).await.unwrap();
    cleanup(&conn).await;
    conn.exec_capture(&commands::cmd_new_detached(S, "/tmp", "sh")).await.unwrap();
    let ls = conn.exec_capture(commands::cmd_list_sessions()).await.unwrap();
    let sessions = parser::parse_sessions(&ls.stdout);
    assert!(sessions.iter().any(|s| s.name == S), "Session fehlt in: {}", ls.stdout);
    cleanup(&conn).await;
}

#[tokio::test]
#[ignore]
async fn pty_attach_marker_und_reattach_semantik() {
    let conn = SshConnection::connect(params()).await.unwrap();
    cleanup(&conn).await;
    conn.exec_capture(&commands::cmd_new_detached(S, "/tmp", "sh")).await.unwrap();

    // Attach 1: Marker tippen
    let mut pty = conn.open_pty(&commands::cmd_attach(S), 100, 30).await.unwrap();
    let mut rx = pty.take_output();
    tokio::time::sleep(std::time::Duration::from_millis(700)).await;
    pty.write(b"echo INT-MARKER-1\r").await.unwrap();
    let mut seen = Vec::new();
    let ok = tokio::time::timeout(std::time::Duration::from_secs(10), async {
        while let Some(ev) = rx.recv().await {
            if let claudedeck_core::ssh::pty::PtyEvent::Data(d) = ev {
                seen.extend_from_slice(&d);
                if String::from_utf8_lossy(&seen).matches("INT-MARKER-1").count() >= 2 {
                    return true;
                }
            }
        }
        false
    })
    .await
    .unwrap_or(false);
    assert!(ok, "Marker nicht gesehen: {}", String::from_utf8_lossy(&seen));
    pty.close().await.unwrap();

    // Session lebt nach Channel-Close weiter (Kern-Semantik)
    let has = conn.exec_capture(&commands::cmd_has_session(S)).await.unwrap();
    assert_eq!(has.exit_code, Some(0), "Session starb mit dem Channel!");

    // Attach 2: Marker steht im Scrollback/Screen
    let mut pty2 = conn.open_pty(&commands::cmd_attach(S), 100, 30).await.unwrap();
    let mut rx2 = pty2.take_output();
    let mut seen2 = Vec::new();
    let ok2 = tokio::time::timeout(std::time::Duration::from_secs(10), async {
        while let Some(ev) = rx2.recv().await {
            if let claudedeck_core::ssh::pty::PtyEvent::Data(d) = ev {
                seen2.extend_from_slice(&d);
                if String::from_utf8_lossy(&seen2).contains("INT-MARKER-1") {
                    return true;
                }
            }
        }
        false
    })
    .await
    .unwrap_or(false);
    assert!(ok2, "Marker nach Reattach nicht sichtbar");
    cleanup(&conn).await;
}

#[tokio::test]
#[ignore]
async fn resize_aendert_tmux_fensterbreite() {
    let conn = SshConnection::connect(params()).await.unwrap();
    cleanup(&conn).await;
    conn.exec_capture(&commands::cmd_new_detached(S, "/tmp", "sh")).await.unwrap();
    let pty = conn.open_pty(&commands::cmd_attach(S), 80, 24).await.unwrap();
    tokio::time::sleep(std::time::Duration::from_millis(700)).await;
    pty.resize(123, 40).await.unwrap();
    tokio::time::sleep(std::time::Duration::from_millis(700)).await;
    let w = conn
        .exec_capture("tmux display -p -t '=cc-inttest' '#{window_width}'")
        .await
        .unwrap();
    assert_eq!(w.stdout.trim(), "123");
    cleanup(&conn).await;
}

#[tokio::test]
#[ignore]
async fn falsches_passwort_ist_authfailed_ohne_retry() {
    let mut p = params();
    p.auth = Auth::Password("definitiv-falsch".into());
    match SshConnection::connect(p).await {
        Err(claudedeck_core::ssh::connection::ConnectError::AuthFailed) => {}
        other => panic!("erwartet AuthFailed, war {other:?}"),
    }
}
```
(Anmerkung für CI-Container: dort ist der User non-root und tmux frisch installiert — `/tmp` als cwd funktioniert überall. `falsches_passwort`-Test ist gegen den CI-Container gedacht; lokal gegen Isekai NICHT laufen lassen (fail2ban) — der Task verifiziert lokal nur die anderen vier per Testnamen-Filter.)

- [ ] **Step 2: Lokal gegen Isekai verifizieren (ohne den Falsch-Passwort-Test!)**

Run: `./dev.sh cargo test -p claudedeck-core --test integration_ssh -- --ignored --skip falsches_passwort` — dev.sh reicht SPIKE_SSH_PASSWORD durch; CLAUDEDECK_TEST_SSH im Aufruf setzen: `./dev.sh sh -c 'CLAUDEDECK_TEST_SSH="192.168.0.161:22:root:$SPIKE_SSH_PASSWORD" cargo test -p claudedeck-core --test integration_ssh -- --ignored --skip falsches_passwort'`
Expected: 4 Tests PASS (exec, roundtrip, reattach, resize)

- [ ] **Step 3: ci.yml um integration-Job erweitern**

```yaml
  integration:
    runs-on: ubuntu-latest
    services:
      sshd:
        image: lscr.io/linuxserver/openssh-server:latest
        env:
          PASSWORD_ACCESS: "true"
          USER_NAME: testuser
          USER_PASSWORD: testpass
          DOCKER_MODS: linuxserver/mods:universal-package-install
          INSTALL_PACKAGES: tmux
        ports:
          - 2222:2222
    steps:
      - uses: actions/checkout@v4
      - uses: dtolnay/rust-toolchain@stable
      - uses: Swatinem/rust-cache@v2
      - name: Auf sshd + tmux warten (Package-Mod braucht Startzeit)
        run: |
          sudo apt-get update && sudo apt-get install -y sshpass
          for i in $(seq 1 45); do
            if sshpass -p testpass ssh -o StrictHostKeyChecking=no -o ConnectTimeout=2 -p 2222 testuser@localhost 'command -v tmux' >/dev/null 2>&1; then
              echo "bereit nach $((i*2))s"; exit 0
            fi
            sleep 2
          done
          echo "sshd/tmux nicht bereit nach 90s"; exit 1
      - run: CLAUDEDECK_TEST_SSH=localhost:2222:testuser:testpass cargo test -p claudedeck-core --test integration_ssh -- --ignored
```

- [ ] **Step 4: fmt/clippy + Commit**

```bash
./dev.sh cargo fmt --all --check && ./dev.sh cargo clippy -p claudedeck-core --all-targets -- -D warnings
git add crates/claudedeck-core/tests .github/workflows/ci.yml
git commit -m "test: SSH-Integrationstests (exec/tmux/reattach/resize/auth) + CI-Job mit sshd-Container"
```

---

## Verifikation Gesamt (Meilenstein-Abschluss M2+M3)

1. `./dev.sh cargo test -p claudedeck-core` → alle Unit-Tests grün (~35)
2. `npx vitest run` → weiterhin 6/6 (unberührt)
3. Integrationstests lokal gegen Isekai grün (Task 8 Step 2)
4. Spike-script-Modus weiterhin PASS (Regression, Task 7 Step 4)
5. Nach GitHub-Push: alle drei CI-Jobs grün (core, frontend, integration) + build.yml liefert Artifact — erst dann ist M3 wirklich abgeschlossen

## Danach

Plan 3 (M4+M5): Tauri-IPC-Brücke, TermPool, Sidebar, Badges — siehe Masterplan-Referenz. Dort außerdem einzuplanen: CSP setzen (statt `csp: null`), src-tauri nutzt dann claudedeck-core wirklich.
