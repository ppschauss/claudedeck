use crate::tmux::commands::FIELD_SEP;

#[derive(Debug, Clone)]
pub struct RawSession {
    /// `#{session_id}` (z. B. `$3`) — separatorfrei, dient `merge()` als Matching-Anker
    /// statt eines Namensvergleichs (Name darf `FIELD_SEP` enthalten).
    pub id: String,
    pub name: String,
    pub created: i64,
    pub attached: u32,
}

#[derive(Debug, Clone)]
pub struct RawPane {
    /// `#{session_id}` der zugehörigen Session (nicht der Name — siehe `RawSession::id`).
    pub session: String,
    pub command: String,
    pub cwd: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SessionKind {
    Claude,
    Shell,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SessionInfo {
    pub name: String,
    pub kind: SessionKind,
    pub cwd: String,
    pub attached: bool,
    pub created: i64,
    pub managed: bool,
}

/// Format: `id|created|attached|name` — der beliebige Name steht LAST, `splitn(4, …)`
/// lässt ihn ungeteilt, auch wenn er selbst `FIELD_SEP` enthält.
pub fn parse_sessions(out: &str) -> Vec<RawSession> {
    out.lines()
        .filter_map(|l| {
            let mut f = l.splitn(4, FIELD_SEP);
            let id = f.next()?.to_string();
            let created: i64 = f.next()?.parse().ok()?;
            let attached: u32 = f.next()?.parse().ok()?;
            let name = f.next()?.to_string();
            Some(RawSession {
                id,
                name,
                created,
                attached,
            })
        })
        .collect()
}

/// Format: `id|command|cwd` — der beliebige Pfad steht LAST, `splitn(3, …)` lässt ihn
/// ungeteilt, auch wenn er selbst `FIELD_SEP` enthält.
pub fn parse_panes(out: &str) -> Vec<RawPane> {
    out.lines()
        .filter_map(|l| {
            let mut f = l.splitn(3, FIELD_SEP);
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
            let mine: Vec<&RawPane> = panes.iter().filter(|p| p.session == s.id).collect();
            let claude = mine.iter().find(|p| p.command == "claude");
            let kind = if claude.is_some() {
                SessionKind::Claude
            } else {
                SessionKind::Shell
            };
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
        let out = "$0|1753100000|1|cc-otakupulse\n$1|1753100100|0|shell 1\n";
        let s = parse_sessions(out);
        assert_eq!(s.len(), 2);
        assert_eq!(s[0].id, "$0");
        assert_eq!(s[0].name, "cc-otakupulse");
        assert_eq!(s[0].created, 1753100000);
        assert_eq!(s[0].attached, 1);
        assert_eq!(s[1].name, "shell 1"); // Name mit Leerzeichen bleibt intakt
        assert_eq!(s[1].attached, 0);
    }

    #[test]
    fn session_name_mit_pipe_bleibt_intakt() {
        let s = parse_sessions("$2|123|0|a|b|c\n");
        assert_eq!(s.len(), 1);
        assert_eq!(s[0].name, "a|b|c"); // splitn(4) lässt das letzte Feld ungeteilt
    }

    #[test]
    fn parse_panes_grundfall() {
        let out = "$0|claude|/mnt/cache/appdata/x\n$0|bash|/root\n";
        let p = parse_panes(out);
        assert_eq!(p.len(), 2);
        assert_eq!(p[0].session, "$0");
        assert_eq!(p[0].command, "claude");
        assert_eq!(p[1].cwd, "/root");
    }

    #[test]
    fn pfad_mit_pipe_bleibt_intakt() {
        let p = parse_panes("$0|bash|/mnt/se|lts|am\n");
        assert_eq!(p.len(), 1);
        assert_eq!(p[0].cwd, "/mnt/se|lts|am"); // splitn(3) lässt das letzte Feld ungeteilt
    }

    fn sess(id: &str, name: &str, created: i64, attached: u32) -> RawSession {
        RawSession {
            id: id.into(),
            name: name.into(),
            created,
            attached,
        }
    }

    fn pane(session: &str, command: &str, cwd: &str) -> RawPane {
        RawPane {
            session: session.into(),
            command: command.into(),
            cwd: cwd.into(),
        }
    }

    #[test]
    fn merge_claude_pane_gewinnt() {
        let sessions = vec![sess("$0", "cc-x", 1, 0)];
        let panes = vec![pane("$0", "bash", "/a"), pane("$0", "claude", "/b")];
        let m = merge(sessions, panes);
        assert_eq!(m[0].kind, SessionKind::Claude);
        assert_eq!(m[0].cwd, "/b"); // cwd der ERSTEN claude-Pane
    }

    #[test]
    fn merge_node_ist_kein_claude() {
        let sessions = vec![sess("$0", "s", 1, 0)];
        let panes = vec![pane("$0", "node", "/a")];
        assert_eq!(merge(sessions, panes)[0].kind, SessionKind::Shell);
    }

    #[test]
    fn merge_matcht_ueber_id_nicht_namen() {
        // Pane gehört per id zu $1, auch wenn der Name der anderen Session gleich wäre
        let sessions = vec![sess("$0", "gleich", 1, 0), sess("$1", "gleich", 2, 0)];
        let panes = vec![pane("$1", "claude", "/b")];
        let m = merge(sessions, panes);
        assert_eq!(m[0].kind, SessionKind::Shell);
        assert_eq!(m[1].kind, SessionKind::Claude);
        assert_eq!(m[1].cwd, "/b");
    }

    #[test]
    fn merge_setzt_managed_und_attached() {
        let sessions = vec![sess("$0", "cc-x", 1, 2), sess("$1", "manuell", 2, 0)];
        let m = merge(sessions, vec![]);
        assert!(m[0].managed && m[0].attached);
        assert!(!m[1].managed && !m[1].attached);
        assert_eq!(m[0].cwd, ""); // keine Pane-Info → leerer cwd
    }

    #[test]
    fn parse_toleriert_kaputte_zeilen() {
        // zu wenig Felder oder nicht-numerisch → Zeile überspringen, kein Panic
        let out = "nur-ein-feld\n$0|123|0|cc-ok\nfoo|bar|baz|qux\n";
        let s = parse_sessions(out);
        assert_eq!(s.len(), 1);
        assert_eq!(s[0].name, "cc-ok");
    }
}
