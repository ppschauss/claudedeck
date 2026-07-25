/**
 * Darstellungslogik für die Ablage — Dateiart, Symbol, Größe, Alter. Pure Funktionen wie
 * `sessionFilter.ts`, damit `FilePanel.tsx` nur noch rendert.
 */

export type FileKind = "image" | "text" | "archive" | "other";

/** Nur diese Arten bekommen eine Vorschau; alles andere kann man ausschließlich herunterladen. */
const IMAGE = ["png", "jpg", "jpeg", "gif", "webp", "svg", "avif", "bmp"];
const TEXT = [
  "md", "txt", "json", "toml", "yaml", "yml", "csv", "log",
  "ts", "tsx", "js", "jsx", "rs", "py", "sh", "css", "html", "xml",
];
const ARCHIVE = ["zip", "tar", "gz", "tgz", "xz", "7z", "rar", "msi", "deb", "rpm"];

/**
 * Endung eines Dateinamens in Kleinschreibung — leer, wenn es keine gibt.
 *
 * Ein **führender** Punkt gehört zum Namen, nicht zur Endung: `.bashrc` ist eine Punktdatei,
 * keine Datei mit der Endung „bashrc". Ohne diese Unterscheidung gälte `.png` als Bild.
 */
function extensionOf(name: string): string {
  const dot = name.lastIndexOf(".");
  if (dot <= 0) return "";
  return name.slice(dot + 1).toLowerCase();
}

export function fileKind(name: string): FileKind {
  const ext = extensionOf(name);
  if (IMAGE.includes(ext)) return "image";
  if (TEXT.includes(ext)) return "text";
  if (ARCHIVE.includes(ext)) return "archive";
  return "other";
}

const ICONS: Record<FileKind, string> = {
  image: "🖼",
  text: "📄",
  archive: "🗜",
  other: "▪",
};

export function fileIcon(kind: FileKind, isDir: boolean): string {
  return isDir ? "📁" : ICONS[kind];
}

/** Größe in der jeweils passenden Einheit, deutsches Dezimalkomma. */
export function formatSize(bytes: number): string {
  if (bytes < 1024) return `${bytes} B`;
  const units = ["KB", "MB", "GB", "TB"];
  let value = bytes / 1024;
  let unit = 0;
  while (value >= 1024 && unit < units.length - 1) {
    value /= 1024;
    unit += 1;
  }
  return `${value.toFixed(1).replace(".", ",")} ${units[unit]}`;
}

/**
 * Alter in grober, lesbarer Form („vor 2 Min").
 *
 * `modifiedSeconds === 0` heißt „der Server hat keine Zeit gemeldet" und ergibt einen leeren
 * Text — sonst stünde dort „vor 55 Jahren", was schlicht falsch wäre. Ein Zeitstempel knapp in
 * der Zukunft (die Uhren von Server und Client laufen selten exakt gleich) gilt als „gerade
 * eben" statt als negative Angabe.
 */
export function formatAge(modifiedSeconds: number, nowMs: number): string {
  if (!modifiedSeconds) return "";

  const seconds = Math.round(nowMs / 1000 - modifiedSeconds);
  if (seconds < 60) return "gerade eben";

  const minutes = Math.floor(seconds / 60);
  if (minutes < 60) return `vor ${minutes} Min`;

  const hours = Math.floor(minutes / 60);
  if (hours < 24) return `vor ${hours} Std`;

  const days = Math.floor(hours / 24);
  return days === 1 ? "vor 1 Tag" : `vor ${days} Tagen`;
}
