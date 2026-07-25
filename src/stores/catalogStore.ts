/**
 * Zustand des Befehls-Panels. Wie `sessionStore` reine Zustandsübergänge — den `list_commands`-
 * Aufruf macht `CommandPanel.tsx`.
 */
import { create } from "zustand";
import type { Catalog, CommandEntry, Connector } from "../lib/ipc";

export interface CatalogState {
  entries: CommandEntry[];
  connectors: Connector[];
  loading: boolean;
  error: string | null;
  /** Suchtext des Panels (unabhängig von der Session-Suche in der Sidebar). */
  query: string;
  /** Projektpfad, für den der aktuelle Katalog gilt — `null` heißt „ohne offene Session". */
  loadedFor: string | null;
  /** Ob überhaupt schon einmal geladen wurde. Trennt „nie geladen" von „ohne Projekt geladen". */
  hasLoaded: boolean;
  /** Ob das rechte Panel ausgeklappt ist. */
  open: boolean;

  queryChanged: (query: string) => void;
  toggled: () => void;
  loadStarted: () => void;
  loaded: (catalog: Catalog, projectDir: string | null) => void;
  failed: (message: string) => void;
}

/**
 * Ob für `projectDir` neu geladen werden muss.
 *
 * Läuft gerade ein Ladevorgang, wird nicht erneut angestoßen — sonst würde jeder Re-Render
 * während des Ladens einen weiteren Exec auslösen.
 */
export function needsReload(
  state: Pick<CatalogState, "loading" | "hasLoaded" | "loadedFor">,
  projectDir: string | null,
): boolean {
  if (state.loading) return false;
  if (!state.hasLoaded) return true;
  return state.loadedFor !== projectDir;
}

export const useCatalogStore = create<CatalogState>((set) => ({
  entries: [],
  connectors: [],
  loading: false,
  error: null,
  query: "",
  loadedFor: null,
  hasLoaded: false,
  open: false,

  queryChanged: (query) => set({ query }),

  toggled: () => set((state) => ({ open: !state.open })),

  // Alten Fehler löschen: sonst bliebe die Meldung des vorherigen Versuchs während des neuen
  // Ladens stehen.
  loadStarted: () => set({ loading: true, error: null }),

  loaded: (catalog, projectDir) =>
    set({
      entries: catalog.entries,
      connectors: catalog.connectors,
      loading: false,
      error: null,
      loadedFor: projectDir,
      hasLoaded: true,
    }),

  // `hasLoaded` bleibt unverändert, damit `needsReload` einen erneuten Versuch zulässt.
  failed: (message) => set({ loading: false, error: message }),
}));
