/**
 * Inhalt des Reiters „Ablage": Dateibrowser über das Arbeitsverzeichnis der aktiven Session,
 * mit Bildvorschau und Download. Den Rahmen stellt `RightPanel.tsx`.
 *
 * Die Reihenfolge kommt vom Backend (Ordner zuerst, darin neueste zuerst) — das ist der
 * eigentliche Zweck: was Claude gerade erzeugt hat, steht oben, ohne dass man erst sortiert.
 *
 * **Nur lesend.** Kein Upload, kein Löschen, kein Umbenennen — siehe `commands/files.rs`.
 */
import { useCallback, useEffect, useState } from "react";
import { describeApiError } from "../lib/apiError";
import { fileIcon, fileKind, formatAge, formatSize } from "../lib/fileKind";
import { downloadFile, listDirectory, previewFile, type RemoteEntry } from "../lib/ipc";
import { useToastStore } from "../stores/toastStore";

interface Preview {
  path: string;
  src: string;
}

export function FilePanel({ projectDir }: { projectDir: string | null }) {
  const [path, setPath] = useState<string | null>(null);
  const [entries, setEntries] = useState<RemoteEntry[]>([]);
  const [loading, setLoading] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [preview, setPreview] = useState<Preview | null>(null);
  const [busyPath, setBusyPath] = useState<string | null>(null);
  const now = Date.now();

  const load = useCallback(async (target: string) => {
    setLoading(true);
    setError(null);
    try {
      setEntries(await listDirectory(target));
      setPath(target);
    } catch (err) {
      setError(describeApiError(err));
    } finally {
      setLoading(false);
    }
  }, []);

  // Beim Sessionwechsel in das neue Projekt springen. Die Vorschau gehört zum alten Ordner und
  // wird dabei verworfen.
  useEffect(() => {
    setPreview(null);
    if (projectDir) {
      void load(projectDir);
    } else {
      setPath(null);
      setEntries([]);
    }
  }, [projectDir, load]);

  /** Elternverzeichnis; `null`, wenn schon an der Wurzel. */
  function parentOf(current: string): string | null {
    const trimmed = current.replace(/\/+$/, "");
    const cut = trimmed.lastIndexOf("/");
    if (cut < 0) return null;
    return cut === 0 ? "/" : trimmed.slice(0, cut);
  }

  async function openPreview(entry: RemoteEntry) {
    setBusyPath(entry.path);
    try {
      const { mime, dataB64 } = await previewFile(entry.path);
      setPreview({ path: entry.path, src: `data:${mime};base64,${dataB64}` });
    } catch (err) {
      // Zu groß oder nicht lesbar — kein Grund für einen Fehlerzustand im Panel, der Download
      // steht ja weiterhin bereit.
      useToastStore.getState().push(describeApiError(err));
    } finally {
      setBusyPath(null);
    }
  }

  async function download(entry: RemoteEntry) {
    setBusyPath(entry.path);
    try {
      const local = await downloadFile(entry.path);
      useToastStore.getState().push(`Gespeichert: ${local}`);
    } catch (err) {
      useToastStore.getState().push(describeApiError(err));
    } finally {
      setBusyPath(null);
    }
  }

  if (!projectDir) {
    return (
      <p className="sidebar-empty file-empty">
        Keine Session aktiv — die Ablage zeigt den Ordner der laufenden Session.
      </p>
    );
  }

  const parent = path ? parentOf(path) : null;

  return (
    <>
      <div className="command-panel-head">
        <div className="file-path-row">
          <button
            type="button"
            className="file-up"
            aria-label="Eine Ebene höher"
            title="Eine Ebene höher"
            disabled={!parent || loading}
            onClick={() => parent && void load(parent)}
          >
            ↑
          </button>
          {/* `dir="rtl"` kürzt lange Pfade vorne statt hinten — der Ordnername am Ende ist das,
              was man lesen will. */}
          <span className="file-path" dir="rtl" title={path ?? ""}>
            {path ?? ""}
          </span>
          <button
            type="button"
            className="command-refresh"
            aria-label="Neu laden"
            title="Neu laden"
            disabled={loading || !path}
            onClick={() => path && void load(path)}
          >
            ⟳
          </button>
        </div>
      </div>

      {error && <p className="error-text sidebar-error">{error}</p>}
      {loading && <p className="sidebar-empty">Lädt …</p>}
      {!loading && !error && entries.length === 0 && (
        <p className="sidebar-empty">Dieser Ordner ist leer.</p>
      )}

      <ul className="file-list">
        {entries.map((entry) => {
          const kind = fileKind(entry.name);
          const age = formatAge(entry.modified, now);
          const busy = busyPath === entry.path;
          return (
            <li key={entry.path} className="file-row">
              <button
                type="button"
                className="file-item"
                disabled={busy}
                title={entry.name}
                onClick={() =>
                  entry.isDir
                    ? void load(entry.path)
                    : kind === "image"
                      ? void openPreview(entry)
                      : void download(entry)
                }
              >
                <span className="file-icon" aria-hidden="true">
                  {fileIcon(kind, entry.isDir)}
                </span>
                <span className="file-name">{entry.name}</span>
                <span className="file-meta">
                  {entry.isDir ? age : [formatSize(entry.size), age].filter(Boolean).join(" · ")}
                </span>
              </button>
              {!entry.isDir && (
                <button
                  type="button"
                  className="session-menu-trigger file-download"
                  aria-label={`${entry.name} herunterladen`}
                  title="Herunterladen"
                  disabled={busy}
                  onClick={() => void download(entry)}
                >
                  ↓
                </button>
              )}
            </li>
          );
        })}
      </ul>

      {preview && (
        <div className="file-preview">
          <div className="file-preview-head">
            <span className="file-name">{preview.path.split("/").pop()}</span>
            <button
              type="button"
              className="session-menu-trigger"
              aria-label="Vorschau schließen"
              onClick={() => setPreview(null)}
            >
              ×
            </button>
          </div>
          <img src={preview.src} alt={preview.path} />
        </div>
      )}
    </>
  );
}
