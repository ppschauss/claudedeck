//! Lesender Dateizugriff über SFTP — die Grundlage der „Ablage".
//!
//! Läuft über **dieselbe** SSH-Verbindung wie Terminals und Kommandos: SFTP ist ein Subsystem
//! auf einem gewöhnlichen Session-Kanal, es entsteht also keine zweite Anmeldung.
//!
//! **Bewusst nur lesend.** Kein Schreiben, Umbenennen oder Löschen: der Server läuft unter
//! `root`, und ein Dateimanager mit Schreibrechten braucht eine Sicherheitsbetrachtung, die
//! dieser Auftrag nicht führt. `russh_sftp` könnte es — die Methoden werden hier schlicht nicht
//! angeboten.
//!
//! Pfade gehen **nicht** durch eine Shell (SFTP überträgt sie als eigenes Protokollfeld), das
//! Quoting-Thema aus `tmux::commands` entfällt hier also.

use russh_sftp::client::SftpSession;

/// Ein Eintrag in einem entfernten Verzeichnis.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RemoteEntry {
    pub name: String,
    /// Vollständiger Pfad — die Oberfläche baut nie selbst welche zusammen, sondern nutzt den
    /// hier gelieferten. Das hält Pfadlogik an einer Stelle.
    pub path: String,
    pub is_dir: bool,
    pub size: u64,
    /// Unix-Sekunden der letzten Änderung; `0`, wenn der Server nichts meldet.
    pub modified: i64,
}

/// Fehler beim Dateizugriff.
#[derive(Debug)]
pub enum SftpError {
    /// Kanal/Subsystem ließ sich nicht öffnen.
    Transport(String),
    /// Der Server lehnte den Zugriff ab (fehlende Rechte, Pfad existiert nicht …).
    Remote(String),
    /// Datei überschreitet die zugestandene Größe.
    TooLarge { size: u64, limit: u64 },
}

impl std::fmt::Display for SftpError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            SftpError::Transport(m) => write!(f, "SFTP-Verbindung fehlgeschlagen: {m}"),
            SftpError::Remote(m) => write!(f, "Zugriff fehlgeschlagen: {m}"),
            SftpError::TooLarge { size, limit } => write!(
                f,
                "Datei ist {size} Bytes groß, erlaubt sind {limit} — bitte herunterladen"
            ),
        }
    }
}

/// Öffnet eine SFTP-Sitzung auf einer bestehenden SSH-Verbindung.
///
/// Bewusst **pro Vorgang** geöffnet und danach fallengelassen, statt eine dauerhaft offen zu
/// halten: die Ablage wird selten benutzt, und ein Kanal, der über Stunden mitläuft, müsste beim
/// Reconnect eigens wieder aufgebaut werden. Ein zusätzlicher Kanalaufbau kostet einen
/// Roundtrip — gegenüber dem Nutzen vernachlässigbar.
pub(crate) async fn open_session(
    handle: &russh::client::Handle<crate::ssh::connection::ClientHandler>,
) -> Result<SftpSession, SftpError> {
    let channel = handle
        .channel_open_session()
        .await
        .map_err(|e| SftpError::Transport(e.to_string()))?;
    channel
        .request_subsystem(true, "sftp")
        .await
        .map_err(|e| SftpError::Transport(e.to_string()))?;
    SftpSession::new(channel.into_stream())
        .await
        .map_err(|e| SftpError::Transport(e.to_string()))
}

/// Listet ein Verzeichnis. Ordner zuerst, darin nach Änderungszeit absteigend.
///
/// Die Reihenfolge ist der eigentliche Zweck der Ablage: was Claude gerade erzeugt hat, soll
/// oben stehen, ohne dass man erst sortieren muss.
pub async fn list_dir(session: &SftpSession, path: &str) -> Result<Vec<RemoteEntry>, SftpError> {
    let entries = session
        .read_dir(path)
        .await
        .map_err(|e| SftpError::Remote(e.to_string()))?;

    let mut out: Vec<RemoteEntry> = entries
        .map(|entry| {
            let meta = entry.metadata();
            RemoteEntry {
                name: entry.file_name(),
                path: entry.path(),
                is_dir: entry.file_type().is_dir(),
                size: meta.size.unwrap_or(0),
                modified: meta.mtime.unwrap_or(0) as i64,
            }
        })
        .collect();

    out.sort_by(|a, b| {
        b.is_dir
            .cmp(&a.is_dir)
            .then(b.modified.cmp(&a.modified))
            .then_with(|| a.name.to_lowercase().cmp(&b.name.to_lowercase()))
    });
    Ok(out)
}

/// Liest eine Datei vollständig — aber nur bis `limit` Bytes.
///
/// Die Größe wird **vorher** über `metadata` geprüft statt hinterher am Ergebnis: sonst wäre die
/// Datei bereits vollständig durch die Leitung und im Speicher, bevor die Grenze greift.
pub async fn read_file(
    session: &SftpSession,
    path: &str,
    limit: u64,
) -> Result<Vec<u8>, SftpError> {
    let meta = session
        .metadata(path)
        .await
        .map_err(|e| SftpError::Remote(e.to_string()))?;
    let size = meta.size.unwrap_or(0);
    if size > limit {
        return Err(SftpError::TooLarge { size, limit });
    }

    session
        .read(path)
        .await
        .map_err(|e| SftpError::Remote(e.to_string()))
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Ordner vor Dateien, darin neueste zuerst — und bei gleicher Zeit nach Namen, damit die
    /// Reihenfolge nicht von der Serverantwort abhängt.
    #[test]
    fn sortierung_stellt_ordner_und_neues_nach_vorn() {
        let mut list = [
            RemoteEntry {
                name: "alt.txt".into(),
                path: "/p/alt.txt".into(),
                is_dir: false,
                size: 1,
                modified: 100,
            },
            RemoteEntry {
                name: "neu.png".into(),
                path: "/p/neu.png".into(),
                is_dir: false,
                size: 2,
                modified: 900,
            },
            RemoteEntry {
                name: "zzz".into(),
                path: "/p/zzz".into(),
                is_dir: true,
                size: 0,
                modified: 50,
            },
            RemoteEntry {
                name: "Aaa".into(),
                path: "/p/Aaa".into(),
                is_dir: true,
                size: 0,
                modified: 50,
            },
        ];
        #[allow(clippy::redundant_closure_for_method_calls)]
        list.sort_by(|a, b| {
            b.is_dir
                .cmp(&a.is_dir)
                .then(b.modified.cmp(&a.modified))
                .then_with(|| a.name.to_lowercase().cmp(&b.name.to_lowercase()))
        });

        let names: Vec<_> = list.iter().map(|e| e.name.as_str()).collect();
        assert_eq!(names, ["Aaa", "zzz", "neu.png", "alt.txt"]);
    }

    #[test]
    fn zu_grosse_datei_meldet_groesse_und_grenze() {
        let err = SftpError::TooLarge {
            size: 9_000_000,
            limit: 8_388_608,
        };
        let text = err.to_string();
        assert!(text.contains("9000000"), "{text}");
        assert!(text.contains("herunterladen"), "{text}");
    }
}
