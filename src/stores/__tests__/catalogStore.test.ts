import { beforeEach, describe, expect, it } from "vitest";
import type { Catalog } from "../../lib/ipc";
import { needsReload, useCatalogStore } from "../catalogStore";

const catalog: Catalog = {
  entries: [{ kind: "skill", name: "homelab-service", description: "Scaffold", scope: "global" }],
  connectors: [{ name: "Semrush", url: "https://x", status: "Connected", connected: true }],
};

function reset() {
  useCatalogStore.setState({
    entries: [],
    connectors: [],
    loading: false,
    error: null,
    query: "",
    loadedFor: null,
    hasLoaded: false,
    open: false,
  });
}

beforeEach(reset);

describe("needsReload", () => {
  it("lädt beim ersten Öffnen", () => {
    expect(needsReload(useCatalogStore.getState(), null)).toBe(true);
  });

  it("lädt nicht erneut für dasselbe Projekt", () => {
    useCatalogStore.getState().loaded(catalog, "/mnt/a");
    expect(needsReload(useCatalogStore.getState(), "/mnt/a")).toBe(false);
  });

  // Beim Sessionwechsel ändern sich die projektlokalen Einträge — der Katalog muss neu.
  it("lädt neu, wenn sich das Projekt ändert", () => {
    useCatalogStore.getState().loaded(catalog, "/mnt/a");
    expect(needsReload(useCatalogStore.getState(), "/mnt/b")).toBe(true);
  });

  // `null` als geladenes Projekt ist ein gültiger Zustand (keine Session offen) und darf nicht
  // mit „noch nie geladen" verwechselt werden.
  it("unterscheidet 'ohne Projekt geladen' von 'nie geladen'", () => {
    useCatalogStore.getState().loaded(catalog, null);
    expect(needsReload(useCatalogStore.getState(), null)).toBe(false);
    expect(needsReload(useCatalogStore.getState(), "/mnt/a")).toBe(true);
  });

  it("lädt nicht erneut, während ein Ladevorgang läuft", () => {
    useCatalogStore.getState().loadStarted();
    expect(needsReload(useCatalogStore.getState(), "/mnt/a")).toBe(false);
  });
});

describe("Ladezustände", () => {
  it("setzt loading und löscht einen alten Fehler beim Start", () => {
    useCatalogStore.getState().failed("kaputt");
    useCatalogStore.getState().loadStarted();
    const state = useCatalogStore.getState();
    expect(state.loading).toBe(true);
    expect(state.error).toBeNull();
  });

  it("übernimmt Einträge und Connectors und merkt sich das Projekt", () => {
    useCatalogStore.getState().loaded(catalog, "/mnt/a");
    const state = useCatalogStore.getState();
    expect(state.entries).toHaveLength(1);
    expect(state.connectors).toHaveLength(1);
    expect(state.loadedFor).toBe("/mnt/a");
    expect(state.hasLoaded).toBe(true);
    expect(state.loading).toBe(false);
  });

  // Nach einem Fehler muss ein erneuter Versuch möglich sein, sonst bleibt das Panel tot.
  it("beendet den Ladevorgang bei einem Fehler und erlaubt einen neuen Versuch", () => {
    useCatalogStore.getState().loadStarted();
    useCatalogStore.getState().failed("SSH weg");
    const state = useCatalogStore.getState();
    expect(state.loading).toBe(false);
    expect(state.error).toBe("SSH weg");
    expect(needsReload(state, null)).toBe(true);
  });
});

describe("toggled", () => {
  it("klappt das Panel auf und wieder zu", () => {
    expect(useCatalogStore.getState().open).toBe(false);
    useCatalogStore.getState().toggled();
    expect(useCatalogStore.getState().open).toBe(true);
    useCatalogStore.getState().toggled();
    expect(useCatalogStore.getState().open).toBe(false);
  });
});
