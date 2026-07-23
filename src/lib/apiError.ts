/**
 * Kleine, pure Helfer, um Fehler aus fehlgeschlagenen `invoke()`-Aufrufen (siehe `ipc.ts`) in
 * der UI anzuzeigen. `invoke()` lehnt bei einem `Err` aus Rust mit dem serialisierten
 * `ApiError`-Objekt selbst ab (kein `Error`-Wrapper) — `isApiError` unterscheidet das von
 * anderen möglichen Ablehnungsgründen (z.B. Tauri-interne Fehler), `describeApiError` liefert
 * in jedem Fall einen anzeigbaren deutschen Text statt z.B. "[object Object]".
 */
import type { ApiError } from "./ipc";

export function isApiError(err: unknown): err is ApiError {
  if (typeof err !== "object" || err === null) return false;
  const candidate = err as Record<string, unknown>;
  return typeof candidate.kind === "string" && typeof candidate.message === "string";
}

export function describeApiError(err: unknown): string {
  if (isApiError(err)) return err.message;
  if (err instanceof Error) return err.message;
  return "Unbekannter Fehler";
}
