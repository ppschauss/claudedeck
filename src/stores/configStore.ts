/**
 * Die geladene `config.json` als gemeinsamer Zustand — bisher holte sich jede Komponente ihre
 * eigene Kopie per `getConfig()`, wodurch eine Änderung (Theme, Zoom, Model) anderswo nicht
 * ankam.
 *
 * Wie die übrigen Stores rein: das Schreiben (`setConfig`) bleibt beim Aufrufer.
 */
import { create } from "zustand";
import type { Config } from "../lib/ipc";

export interface ConfigState {
  config: Config | null;
  /** Frisch geladen oder ersetzt. */
  loaded: (config: Config) => void;
  /**
   * Teiländerung. No-Op, solange nichts geladen ist — ohne Grundlage ließe sich sonst eine
   * unvollständige Config zusammensetzen und beim Speichern alles Übrige überschreiben.
   */
  patched: (patch: Partial<Config>) => void;
}

export const useConfigStore = create<ConfigState>((set) => ({
  config: null,

  loaded: (config) => set({ config }),

  patched: (patch) =>
    set((state) => (state.config ? { config: { ...state.config, ...patch } } : {})),
}));
