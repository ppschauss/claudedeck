//! IPC für die Ablage: Verzeichnis auflisten, Bild vorschauen, Datei herunterladen.
//!
//! Eigene Datei wie `catalog.rs` — `sessions.rs` ist mit ~690 Zeilen groß genug.
//!
//! **Nur lesend.** Kein Upload, kein Löschen, kein Umbenennen: der Server läuft unter `root`,
//! und ein schreibender Dateimanager braucht eine Sicherheitsbetrachtung, die dieser Auftrag
//! nicht führt.

use data_encoding::BASE64;
use serde::Serialize;
use tauri::{AppHandle, State};

use claudedeck_core::sftp::{RemoteEntry, SftpError};

use crate::error::ApiError;
use crate::state::AppState;

use super::sessions::{note_ssh_failure, require_conn};

/// Obergrenze für die Bildvorschau. Der Inhalt wandert base64-kodiert (also rund 4/3 der Größe)
/// durch die IPC-Brücke und landet als Data-URL im DOM — bei einem versehentlich angeklickten
/// Riesenbild wäre das sonst ein Speicherproblem statt einer Vorschau.
const PREVIEW_LIMIT: u64 = 8 * 1024 * 1024;

/// Obergrenze für Downloads. Großzügig, aber nicht unbegrenzt: die Datei wird vollständig im
/// Speicher gehalten, bevor sie geschrieben wird.
const DOWNLOAD_LIMIT: u64 = 512 * 1024 * 1024;

#[derive(Serialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct RemoteEntryDto {
    pub name: String,
    pub path: String,
    pub is_dir: bool,
    pub size: u64,
    pub modified: i64,
}

impl From<RemoteEntry> for RemoteEntryDto {
    fn from(e: RemoteEntry) -> Self {
        RemoteEntryDto {
            name: e.name,
            path: e.path,
            is_dir: e.is_dir,
            size: e.size,
            modified: e.modified,
        }
    }
}

#[derive(Serialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct FilePreview {
    pub mime: String,
    pub data_b64: String,
}

/// `SftpError` trägt die Ursache schon als Text; „zu groß" ist ein Bedienfall und keine
/// Verbindungsstörung, deshalb `Io` statt `Ssh`.
fn to_api_error(err: SftpError) -> ApiError {
    let message = err.to_string();
    match err {
        SftpError::Transport(_) => ApiError::Ssh { message },
        SftpError::Remote(_) | SftpError::TooLarge { .. } => ApiError::Io { message },
    }
}

#[tauri::command]
pub async fn list_directory(
    app: AppHandle,
    state: State<'_, AppState>,
    path: String,
) -> Result<Vec<RemoteEntryDto>, ApiError> {
    let conn = {
        let inner = state.lock().await;
        require_conn(&inner)?
    };

    let entries = conn.sftp_list(&path).await.map_err(|e| match e {
        // Ein Transportfehler heißt: die Verbindung taugt nichts mehr — das soll der
        // Reconnect-Supervisor erfahren, wie bei den übrigen Kommandos auch.
        SftpError::Transport(ref m) => note_ssh_failure(&app, m.clone()),
        other => to_api_error(other),
    })?;

    Ok(entries.into_iter().map(RemoteEntryDto::from).collect())
}

/// Liefert eine Datei als Data-URL-tauglichen Base64-Block.
///
/// `mime` wird aus der Endung abgeleitet und nicht aus dem Inhalt: für die Vorschau genügt das,
/// und den Inhalt zu schnüffeln würde bedeuten, die Datei schon geladen zu haben.
#[tauri::command]
pub async fn preview_file(
    app: AppHandle,
    state: State<'_, AppState>,
    path: String,
) -> Result<FilePreview, ApiError> {
    let conn = {
        let inner = state.lock().await;
        require_conn(&inner)?
    };

    let bytes = conn
        .sftp_read(&path, PREVIEW_LIMIT)
        .await
        .map_err(|e| match e {
            SftpError::Transport(ref m) => note_ssh_failure(&app, m.clone()),
            other => to_api_error(other),
        })?;

    Ok(FilePreview {
        mime: mime_for(&path).to_string(),
        data_b64: BASE64.encode(&bytes),
    })
}

/// Lädt eine Datei in den Downloads-Ordner und liefert den lokalen Pfad zurück.
///
/// Bewusst ohne „Speichern unter"-Dialog: der bräuchte `tauri-plugin-dialog` plus eine
/// Erweiterung der Capabilities. Der Downloads-Ordner ist das erwartete Ziel, und der
/// zurückgegebene Pfad wird dem Nutzer angezeigt.
#[tauri::command]
pub async fn download_file(
    app: AppHandle,
    state: State<'_, AppState>,
    path: String,
) -> Result<String, ApiError> {
    let conn = {
        let inner = state.lock().await;
        require_conn(&inner)?
    };

    let bytes = conn
        .sftp_read(&path, DOWNLOAD_LIMIT)
        .await
        .map_err(|e| match e {
            SftpError::Transport(ref m) => note_ssh_failure(&app, m.clone()),
            other => to_api_error(other),
        })?;

    let dir = dirs::download_dir()
        .or_else(dirs::home_dir)
        .ok_or_else(|| ApiError::Io {
            message: "Kein Downloads-Ordner gefunden".to_string(),
        })?;
    std::fs::create_dir_all(&dir)?;

    let name = path
        .rsplit('/')
        .next()
        .filter(|n| !n.is_empty())
        .unwrap_or("download");
    let target = unique_path(&dir, name);
    std::fs::write(&target, &bytes)?;

    Ok(target.to_string_lossy().into_owned())
}

/// Findet einen freien Dateinamen: `bericht.pdf`, dann `bericht (2).pdf`, `bericht (3).pdf` …
///
/// Zweimal dieselbe Datei zu laden darf die erste nicht überschreiben — das wäre stiller
/// Datenverlust, falls man die alte noch brauchte.
fn unique_path(dir: &std::path::Path, name: &str) -> std::path::PathBuf {
    let candidate = dir.join(name);
    if !candidate.exists() {
        return candidate;
    }

    let (stem, ext) = match name.rsplit_once('.') {
        // Ein führender Punkt gehört zum Namen (".bashrc"), nicht zur Endung.
        Some((s, e)) if !s.is_empty() => (s, format!(".{e}")),
        _ => (name, String::new()),
    };

    for n in 2..1000 {
        let candidate = dir.join(format!("{stem} ({n}){ext}"));
        if !candidate.exists() {
            return candidate;
        }
    }
    dir.join(name)
}

/// MIME-Typ anhand der Endung — nur für die Bildvorschau gebraucht.
fn mime_for(path: &str) -> &'static str {
    let ext = path.rsplit('.').next().unwrap_or("").to_lowercase();
    match ext.as_str() {
        "png" => "image/png",
        "jpg" | "jpeg" => "image/jpeg",
        "gif" => "image/gif",
        "webp" => "image/webp",
        "bmp" => "image/bmp",
        "svg" => "image/svg+xml",
        "avif" => "image/avif",
        _ => "application/octet-stream",
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn mime_kennt_die_gaengigen_bildformate() {
        assert_eq!(mime_for("/a/b.png"), "image/png");
        assert_eq!(mime_for("/a/B.JPG"), "image/jpeg");
        assert_eq!(mime_for("/a/b.jpeg"), "image/jpeg");
        assert_eq!(mime_for("/a/b.svg"), "image/svg+xml");
    }

    #[test]
    fn mime_faellt_bei_unbekanntem_auf_octet_stream() {
        assert_eq!(mime_for("/a/b.xyz"), "application/octet-stream");
        assert_eq!(mime_for("/a/ohne-endung"), "application/octet-stream");
    }

    #[test]
    fn unique_path_nutzt_den_namen_wenn_frei() {
        let tmp = tempfile::TempDir::new().unwrap();
        assert_eq!(unique_path(tmp.path(), "a.png"), tmp.path().join("a.png"));
    }

    /// Der eigentliche Zweck: eine vorhandene Datei darf nicht überschrieben werden.
    #[test]
    fn unique_path_zaehlt_hoch_statt_zu_ueberschreiben() {
        let tmp = tempfile::TempDir::new().unwrap();
        std::fs::write(tmp.path().join("a.png"), b"alt").unwrap();
        assert_eq!(
            unique_path(tmp.path(), "a.png"),
            tmp.path().join("a (2).png")
        );

        std::fs::write(tmp.path().join("a (2).png"), b"alt").unwrap();
        assert_eq!(
            unique_path(tmp.path(), "a.png"),
            tmp.path().join("a (3).png")
        );
    }

    #[test]
    fn unique_path_behaelt_namen_ohne_endung_bei() {
        let tmp = tempfile::TempDir::new().unwrap();
        std::fs::write(tmp.path().join("README"), b"alt").unwrap();
        assert_eq!(
            unique_path(tmp.path(), "README"),
            tmp.path().join("README (2)")
        );
    }

    /// `.bashrc` ist ein Name ohne Endung, keine Endung ohne Namen.
    #[test]
    fn unique_path_behandelt_fuehrenden_punkt_als_teil_des_namens() {
        let tmp = tempfile::TempDir::new().unwrap();
        std::fs::write(tmp.path().join(".bashrc"), b"alt").unwrap();
        assert_eq!(
            unique_path(tmp.path(), ".bashrc"),
            tmp.path().join(".bashrc (2)")
        );
    }
}
