#[derive(Debug, Clone)]
pub struct RawSession {
    pub name: String,
    pub created: i64,
    pub attached: u32,
}

#[derive(Debug, Clone)]
pub struct RawPane {
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

pub fn parse_sessions(out: &str) -> Vec<RawSession> {
    out.lines()
        .filter_map(|l| {
            let mut f = l.split('\t');
            let name = f.next()?.to_string();
            let created: i64 = f.next()?.parse().ok()?;
            let attached: u32 = f.next()?.parse().ok()?;
            Some(RawSession {
                name,
                created,
                attached,
            })
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
        let sessions = vec![RawSession {
            name: "cc-x".into(),
            created: 1,
            attached: 0,
        }];
        let panes = vec![
            RawPane {
                session: "cc-x".into(),
                command: "bash".into(),
                cwd: "/a".into(),
            },
            RawPane {
                session: "cc-x".into(),
                command: "claude".into(),
                cwd: "/b".into(),
            },
        ];
        let m = merge(sessions, panes);
        assert_eq!(m[0].kind, SessionKind::Claude);
        assert_eq!(m[0].cwd, "/b"); // cwd der ERSTEN claude-Pane
    }

    #[test]
    fn merge_node_ist_kein_claude() {
        let sessions = vec![RawSession {
            name: "s".into(),
            created: 1,
            attached: 0,
        }];
        let panes = vec![RawPane {
            session: "s".into(),
            command: "node".into(),
            cwd: "/a".into(),
        }];
        assert_eq!(merge(sessions, panes)[0].kind, SessionKind::Shell);
    }

    #[test]
    fn merge_setzt_managed_und_attached() {
        let sessions = vec![
            RawSession {
                name: "cc-x".into(),
                created: 1,
                attached: 2,
            },
            RawSession {
                name: "manuell".into(),
                created: 2,
                attached: 0,
            },
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
