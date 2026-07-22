use std::collections::HashSet;

/// Ordnername -> tmux-tauglicher Session-Name (ohne "cc-"-Präfix).
pub fn sanitize(folder: &str) -> String {
    let mut s: String = folder
        .chars()
        .map(|c| {
            if c.is_ascii_alphanumeric() || c == '_' || c == '-' {
                c
            } else {
                '-'
            }
        })
        .collect();
    s.truncate(40);
    s
}

/// Hängt -2, -3, … an, bis der Name nicht in `existing` vorkommt.
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
