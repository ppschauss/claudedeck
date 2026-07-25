//! Parser für den Sammel-Exec und für `claude mcp list`.

use super::commands::FILE_MARKER;
use super::{CommandEntry, CommandKind, CommandScope, Connector};

/// Zerlegt den Sammel-Output in Einträge.
///
/// `project_dir` ist das Arbeitsverzeichnis der aktiven Session; Dateien darunter gelten als
/// projektlokal. Alles vor der ersten [`FILE_MARKER`]-Zeile ist Rauschen (z. B. `find`-Meldungen)
/// und wird verworfen. Dateien, deren Pfad zu keiner bekannten Art passt, werden übergangen —
/// lieber ein Eintrag zu wenig als ein erfundener.
pub fn parse_catalog(raw: &str, project_dir: Option<&str>) -> Vec<CommandEntry> {
    let mut entries = Vec::new();
    let mut current: Option<(String, String)> = None;

    for line in raw.lines() {
        if let Some(path) = line.strip_prefix(FILE_MARKER) {
            if let Some((prev_path, body)) = current.take() {
                entries.extend(entry_from(&prev_path, &body, project_dir));
            }
            current = Some((path.trim().to_string(), String::new()));
        } else if let Some((_, body)) = current.as_mut() {
            body.push_str(line);
            body.push('\n');
        }
    }
    if let Some((path, body)) = current.take() {
        entries.extend(entry_from(&path, &body, project_dir));
    }

    entries
}

fn entry_from(path: &str, body: &str, project_dir: Option<&str>) -> Option<CommandEntry> {
    let is_skill = path.ends_with("/SKILL.md");
    let kind = if is_skill {
        CommandKind::Skill
    } else if path.contains("/agents/") {
        CommandKind::Agent
    } else if path.contains("/commands/") {
        CommandKind::Command
    } else {
        return None;
    };

    // Der Schrägstrich im Vergleich verhindert, dass „/mnt/app" auch „/mnt/app2" einfängt.
    let scope = match project_dir {
        Some(dir) if path.starts_with(&format!("{}/", dir.trim_end_matches('/'))) => {
            CommandScope::Project
        }
        _ => CommandScope::Global,
    };

    // Bei SKILL.md trägt der Ordner den sprechenden Namen, sonst die Datei selbst.
    let fallback_name = if is_skill {
        parent_dir_name(path)
    } else {
        file_stem(path)
    };

    Some(CommandEntry {
        kind,
        name: frontmatter_field(body, "name").unwrap_or_else(|| fallback_name.to_string()),
        description: frontmatter_field(body, "description").unwrap_or_default(),
        scope,
    })
}

/// Liest ein Feld aus dem YAML-Frontmatter.
///
/// Bewusst ein Zeilenscan statt einer YAML-Abhängigkeit: gebraucht werden genau zwei skalare
/// Felder. Mehrzeilige Werte (`>-`/`|`-Blöcke) werden dadurch nicht unterstützt — in der Praxis
/// stehen `name` und `description` einzeilig.
fn frontmatter_field(body: &str, key: &str) -> Option<String> {
    let mut lines = body.lines();
    if lines.by_ref().find(|l| !l.trim().is_empty())?.trim() != "---" {
        return None;
    }

    let prefix = format!("{key}:");
    for line in lines {
        if line.trim() == "---" {
            return None;
        }
        // Nur am ersten Doppelpunkt trennen — Werte enthalten oft selbst welche.
        if let Some(rest) = line.strip_prefix(&prefix) {
            return Some(unquote(rest.trim()).to_string());
        }
    }
    None
}

fn unquote(value: &str) -> &str {
    for quote in ['"', '\''] {
        if let Some(inner) = value
            .strip_prefix(quote)
            .and_then(|v| v.strip_suffix(quote))
        {
            return inner;
        }
    }
    value
}

fn file_stem(path: &str) -> &str {
    let file = path.rsplit('/').next().unwrap_or(path);
    file.strip_suffix(".md").unwrap_or(file)
}

fn parent_dir_name(path: &str) -> &str {
    let mut parts = path.rsplit('/');
    parts.next();
    parts.next().unwrap_or("")
}

/// Zerlegt die Ausgabe von `claude mcp list`.
///
/// Zeilenformat: `<name>: <url>[ (<transport>)] - <status>`. Die Kopfzeile
/// („Checking MCP server health…") hat keinen Doppelpunkt und fällt dadurch von selbst heraus.
pub fn parse_mcp_list(raw: &str) -> Vec<Connector> {
    raw.lines().filter_map(parse_mcp_line).collect()
}

fn parse_mcp_line(line: &str) -> Option<Connector> {
    let line = line.trim();
    if line.is_empty() {
        return None;
    }

    let (name, rest) = line.split_once(':')?;
    let name = name.trim();
    if name.is_empty() {
        return None;
    }

    // Der Status hängt hinten; die URL enthält selbst Bindestriche, deshalb von rechts trennen.
    let (url_part, status_part) = rest.rsplit_once(" - ")?;

    let mut url = url_part.trim();
    if url.ends_with(')') {
        if let Some(idx) = url.rfind(" (") {
            url = url[..idx].trim_end();
        }
    }

    // Die CLI stellt dem Status ein Symbol voran (√ / !), das nicht in die UI gehört.
    let status = status_part
        .trim()
        .trim_start_matches(|c: char| !c.is_alphanumeric())
        .trim();

    Some(Connector {
        name: name.to_string(),
        url: url.to_string(),
        status: status.to_string(),
        connected: status.contains("Connected"),
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::catalog::{CommandKind, CommandScope};

    /// Baut den Sammel-Output so, wie ihn `commands::cmd_scan_catalog` erzeugt.
    fn block(path: &str, body: &str) -> String {
        format!("===F:{path}\n{body}\n")
    }

    const SKILL: &str = "---\nname: homelab-service\ndescription: Scaffold, deploy, or modify a self-hosted service.\n---\n\n# Homelab";

    #[test]
    fn liest_name_und_description_aus_dem_frontmatter() {
        let raw = block("/root/.claude/skills/homelab-service/SKILL.md", SKILL);
        let entries = parse_catalog(&raw, None);
        assert_eq!(entries.len(), 1);
        assert_eq!(entries[0].name, "homelab-service");
        assert_eq!(
            entries[0].description,
            "Scaffold, deploy, or modify a self-hosted service."
        );
        assert_eq!(entries[0].kind, CommandKind::Skill);
        assert_eq!(entries[0].scope, CommandScope::Global);
    }

    #[test]
    fn erkennt_agents_und_commands_am_pfad() {
        let raw = format!(
            "{}{}",
            block(
                "/root/.claude/agents/explore.md",
                "---\nname: Explore\ndescription: Sucht.\n---"
            ),
            block(
                "/root/.claude/commands/deploy.md",
                "---\nname: deploy\ndescription: Rollt aus.\n---"
            ),
        );
        let entries = parse_catalog(&raw, None);
        assert_eq!(entries[0].kind, CommandKind::Agent);
        assert_eq!(entries[1].kind, CommandKind::Command);
    }

    #[test]
    fn erkennt_plugin_skills() {
        let raw = block(
            "/root/.claude/plugins/cache/official/superpowers/6.2.0/skills/brainstorming/SKILL.md",
            SKILL,
        );
        let entries = parse_catalog(&raw, None);
        assert_eq!(entries[0].kind, CommandKind::Skill);
    }

    // Ohne diese Unterscheidung wüsste der Nutzer nicht, dass ein Eintrag nur in dieser einen
    // Session existiert.
    #[test]
    fn markiert_projektlokale_eintraege() {
        let raw = format!(
            "{}{}",
            block("/root/.claude/skills/global-skill/SKILL.md", SKILL),
            block(
                "/mnt/cache/appdata/claudedeck/.claude/skills/spike/SKILL.md",
                SKILL
            ),
        );
        let entries = parse_catalog(&raw, Some("/mnt/cache/appdata/claudedeck"));
        assert_eq!(entries[0].scope, CommandScope::Global);
        assert_eq!(entries[1].scope, CommandScope::Project);
    }

    // Ein Eintrag ohne Frontmatter darf nicht verschwinden — er ist ja aufrufbar.
    #[test]
    fn faellt_ohne_frontmatter_auf_den_dateinamen_zurueck() {
        let raw = block("/root/.claude/commands/quickfix.md", "Einfach nur Text.");
        let entries = parse_catalog(&raw, None);
        assert_eq!(entries[0].name, "quickfix");
        assert_eq!(entries[0].description, "");
    }

    /// Bei `SKILL.md` ist der Ordnername der sprechende Teil, nicht der Dateiname.
    #[test]
    fn nutzt_bei_skill_md_den_ordnernamen_als_fallback() {
        let raw = block(
            "/root/.claude/skills/mein-skill/SKILL.md",
            "kein Frontmatter",
        );
        let entries = parse_catalog(&raw, None);
        assert_eq!(entries[0].name, "mein-skill");
    }

    #[test]
    fn entfernt_anfuehrungszeichen_um_frontmatter_werte() {
        let raw = block(
            "/root/.claude/agents/a.md",
            "---\nname: \"Zitiert\"\ndescription: 'Auch hier'\n---",
        );
        let entries = parse_catalog(&raw, None);
        assert_eq!(entries[0].name, "Zitiert");
        assert_eq!(entries[0].description, "Auch hier");
    }

    /// `description:` steht in echten Skills oft direkt neben Feldern, die einen Doppelpunkt im
    /// Wert haben — es darf nur am ersten Doppelpunkt getrennt werden.
    #[test]
    fn behaelt_doppelpunkte_im_wert() {
        let raw = block(
            "/root/.claude/agents/a.md",
            "---\nname: a\ndescription: Nutze dies, wenn: X passiert\n---",
        );
        let entries = parse_catalog(&raw, None);
        assert_eq!(entries[0].description, "Nutze dies, wenn: X passiert");
    }

    #[test]
    fn ignoriert_felder_ausserhalb_des_frontmatters() {
        let raw = block(
            "/root/.claude/agents/a.md",
            "---\nname: echt\n---\n\nname: nur Fließtext\ndescription: auch Fließtext",
        );
        let entries = parse_catalog(&raw, None);
        assert_eq!(entries[0].name, "echt");
        assert_eq!(entries[0].description, "");
    }

    #[test]
    fn liefert_leere_liste_bei_leerer_ausgabe() {
        assert!(parse_catalog("", None).is_empty());
        assert!(parse_catalog("\n\n", None).is_empty());
    }

    #[test]
    fn ueberspringt_vorspann_vor_der_ersten_marke() {
        let raw = format!(
            "find: kein Zugriff\n{}",
            block("/root/.claude/agents/a.md", "---\nname: a\n---")
        );
        let entries = parse_catalog(&raw, None);
        assert_eq!(entries.len(), 1);
        assert_eq!(entries[0].name, "a");
    }

    // --- claude mcp list -------------------------------------------------------------------

    /// Echte Ausgabe von `claude mcp list` (2.1.220), gekürzt.
    const MCP: &str = "Checking MCP server health…\n\nclaude.ai Semrush: https://mcp.semrush.com/claude/v1/mcp - √ Connected\nclaude.ai Ahrefs: https://api.ahrefs.com/mcp/mcp - ! Needs authentication\nhiggsfield: https://mcp.higgsfield.ai/mcp (HTTP) - √ Connected\n";

    #[test]
    fn parst_name_url_und_status() {
        let list = parse_mcp_list(MCP);
        assert_eq!(list.len(), 3);
        assert_eq!(list[0].name, "claude.ai Semrush");
        assert_eq!(list[0].url, "https://mcp.semrush.com/claude/v1/mcp");
        assert_eq!(list[0].status, "Connected");
        assert!(list[0].connected);
    }

    #[test]
    fn erkennt_nicht_verbundene_server() {
        let list = parse_mcp_list(MCP);
        assert_eq!(list[1].name, "claude.ai Ahrefs");
        assert_eq!(list[1].status, "Needs authentication");
        assert!(!list[1].connected);
    }

    /// Der Transport-Zusatz `(HTTP)` gehört nicht in die URL.
    #[test]
    fn entfernt_den_transport_zusatz_aus_der_url() {
        let list = parse_mcp_list(MCP);
        assert_eq!(list[2].name, "higgsfield");
        assert_eq!(list[2].url, "https://mcp.higgsfield.ai/mcp");
    }

    #[test]
    fn ignoriert_kopfzeile_und_leerzeilen() {
        assert!(parse_mcp_list("Checking MCP server health…\n\n").is_empty());
        assert!(parse_mcp_list("").is_empty());
    }

    /// Eine URL enthält selbst `://` — der Name darf nur am *ersten* Doppelpunkt abgetrennt
    /// werden, sonst zerfällt jede Zeile falsch.
    #[test]
    fn trennt_den_namen_am_ersten_doppelpunkt() {
        let list = parse_mcp_list("mein: server: https://x.example/mcp - √ Connected\n");
        assert_eq!(list[0].name, "mein");
        assert_eq!(list[0].url, "server: https://x.example/mcp");
    }
}
