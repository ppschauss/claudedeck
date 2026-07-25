import { beforeEach, describe, expect, it } from "vitest";
import type { Project, SessionInfo, SessionList } from "../../lib/ipc";
import { useSessionStore } from "../sessionStore";

const running: SessionInfo = {
  name: "cc-otakupulse",
  kind: "claude",
  cwd: "/mnt/cache/appdata/otakupulse",
  attached: false,
  created: 1000,
  managed: true,
};
const startable: Project = {
  path: "/mnt/cache/appdata/habitbot",
  name: "habitbot",
  modified: 1_753_400_000,
};

function reset() {
  useSessionStore.setState({
    running: [],
    startable: [],
    openSessions: new Map(),
    activeSessionId: null,
    query: "",
    sortBy: "name",
  });
}

beforeEach(reset);

describe("sessionsLoaded", () => {
  it("übernimmt running und startable 1:1 aus der SessionList", () => {
    const list: SessionList = { running: [running], startable: [startable] };
    useSessionStore.getState().sessionsLoaded(list);
    const state = useSessionStore.getState();
    expect(state.running).toEqual([running]);
    expect(state.startable).toEqual([startable]);
  });
});

describe("opened", () => {
  it("legt eine neue offene Session mit Badge 0 an und aktiviert sie sofort", () => {
    useSessionStore.getState().opened("s1", "cc-otakupulse");
    const state = useSessionStore.getState();
    expect(state.activeSessionId).toBe("s1");
    const entry = state.openSessions.get("s1");
    expect(entry).toBeDefined();
    expect(entry?.name).toBe("cc-otakupulse");
    expect(entry?.activity).toEqual({ badge: 0, lastOutputAt: null, notified: false });
    expect(entry?.notifyEnabled).toBe(true);
    expect(entry?.lost).toBe(false);
  });

  it("mehrere geöffnete Sessions bleiben nebeneinander bestehen (kein Disposen)", () => {
    useSessionStore.getState().opened("s1", "cc-a");
    useSessionStore.getState().opened("s2", "cc-b");
    const state = useSessionStore.getState();
    expect(state.openSessions.size).toBe(2);
    expect(state.activeSessionId).toBe("s2");
  });
});

describe("outputReceived", () => {
  it("erhöht den Badge einer inaktiven Session", () => {
    useSessionStore.getState().opened("s1", "cc-a");
    useSessionStore.getState().opened("s2", "cc-b"); // s2 jetzt aktiv, s1 im Hintergrund
    useSessionStore.getState().outputReceived("s1", 5000);
    const entry = useSessionStore.getState().openSessions.get("s1");
    expect(entry?.activity.badge).toBe(1);
    expect(entry?.activity.lastOutputAt).toBe(5000);
  });

  it("hält den Badge der aktiven Session bei 0", () => {
    useSessionStore.getState().opened("s1", "cc-a");
    useSessionStore.getState().outputReceived("s1", 5000); // s1 ist aktiv
    const entry = useSessionStore.getState().openSessions.get("s1");
    expect(entry?.activity.badge).toBe(0);
    expect(entry?.activity.lastOutputAt).toBe(5000);
  });

  it("ist ein No-Op für eine unbekannte sessionId", () => {
    useSessionStore.getState().opened("s1", "cc-a");
    expect(() => useSessionStore.getState().outputReceived("unbekannt", 1)).not.toThrow();
    expect(useSessionStore.getState().openSessions.size).toBe(1);
  });
});

describe("activated", () => {
  it("wechselt activeSessionId und setzt den Badge der Zielsession auf 0", () => {
    useSessionStore.getState().opened("s1", "cc-a");
    useSessionStore.getState().opened("s2", "cc-b");
    useSessionStore.getState().outputReceived("s1", 1000); // s1 im Hintergrund, Badge hoch
    expect(useSessionStore.getState().openSessions.get("s1")?.activity.badge).toBe(1);

    useSessionStore.getState().activated("s1");
    const state = useSessionStore.getState();
    expect(state.activeSessionId).toBe("s1");
    expect(state.openSessions.get("s1")?.activity.badge).toBe(0);
  });
});

describe("closed", () => {
  it("entfernt die Session aus openSessions", () => {
    useSessionStore.getState().opened("s1", "cc-a");
    useSessionStore.getState().closed("s1");
    expect(useSessionStore.getState().openSessions.has("s1")).toBe(false);
  });

  it("räumt activeSessionId, wenn die aktive Session geschlossen wird", () => {
    useSessionStore.getState().opened("s1", "cc-a");
    useSessionStore.getState().closed("s1");
    expect(useSessionStore.getState().activeSessionId).toBeNull();
  });

  it("lässt activeSessionId unangetastet, wenn eine andere Session geschlossen wird", () => {
    useSessionStore.getState().opened("s1", "cc-a");
    useSessionStore.getState().opened("s2", "cc-b"); // s2 aktiv
    useSessionStore.getState().closed("s1");
    expect(useSessionStore.getState().activeSessionId).toBe("s2");
    expect(useSessionStore.getState().openSessions.has("s2")).toBe(true);
  });
});

describe("markLost", () => {
  it("setzt lost auf true für die betroffene Session, lässt sie aber in openSessions", () => {
    useSessionStore.getState().opened("s1", "cc-a");
    useSessionStore.getState().markLost("s1");
    const state = useSessionStore.getState();
    expect(state.openSessions.get("s1")?.lost).toBe(true);
    expect(state.openSessions.has("s1")).toBe(true);
  });

  it("ist ein No-Op für eine unbekannte sessionId", () => {
    useSessionStore.getState().opened("s1", "cc-a");
    expect(() => useSessionStore.getState().markLost("unbekannt")).not.toThrow();
    expect(useSessionStore.getState().openSessions.get("s1")?.lost).toBe(false);
  });

  it("activated funktioniert weiterhin für eine lost-Session (Re-Attach macht sie wieder aktiv)", () => {
    useSessionStore.getState().opened("s1", "cc-a");
    useSessionStore.getState().opened("s2", "cc-b"); // s2 aktiv, s1 im Hintergrund
    useSessionStore.getState().markLost("s1");
    useSessionStore.getState().activated("s1");
    const state = useSessionStore.getState();
    expect(state.activeSessionId).toBe("s1");
    expect(state.openSessions.get("s1")?.lost).toBe(true);
  });

  it("closed entfernt auch eine lost-Session vollständig", () => {
    useSessionStore.getState().opened("s1", "cc-a");
    useSessionStore.getState().markLost("s1");
    useSessionStore.getState().closed("s1");
    expect(useSessionStore.getState().openSessions.has("s1")).toBe(false);
  });
});

describe("reattached", () => {
  it("setzt lost zurück auf false für eine lost-Session", () => {
    useSessionStore.getState().opened("s1", "cc-a");
    useSessionStore.getState().markLost("s1");
    expect(useSessionStore.getState().openSessions.get("s1")?.lost).toBe(true);
    useSessionStore.getState().reattached("s1");
    expect(useSessionStore.getState().openSessions.get("s1")?.lost).toBe(false);
  });

  it("ist ein No-Op für eine unbekannte sessionId", () => {
    useSessionStore.getState().opened("s1", "cc-a");
    expect(() => useSessionStore.getState().reattached("unbekannt")).not.toThrow();
    expect(useSessionStore.getState().openSessions.size).toBe(1);
  });

  it("lässt eine bereits nicht-lost Session unverändert (idempotent)", () => {
    useSessionStore.getState().opened("s1", "cc-a");
    useSessionStore.getState().reattached("s1");
    expect(useSessionStore.getState().openSessions.get("s1")?.lost).toBe(false);
  });
});

describe("notifiedSent", () => {
  it("setzt activity.notified auf true", () => {
    useSessionStore.getState().opened("s1", "cc-a");
    useSessionStore.getState().opened("s2", "cc-b"); // s1 im Hintergrund
    useSessionStore.getState().outputReceived("s1", 1000);
    expect(useSessionStore.getState().openSessions.get("s1")?.activity.notified).toBe(false);
    useSessionStore.getState().notifiedSent("s1");
    expect(useSessionStore.getState().openSessions.get("s1")?.activity.notified).toBe(true);
  });

  it("ist ein No-Op für eine unbekannte sessionId", () => {
    useSessionStore.getState().opened("s1", "cc-a");
    expect(() => useSessionStore.getState().notifiedSent("unbekannt")).not.toThrow();
    expect(useSessionStore.getState().openSessions.get("s1")?.activity.notified).toBe(false);
  });

  it("neuer Output setzt notified wieder zurück (neuer Benachrichtigungszyklus)", () => {
    useSessionStore.getState().opened("s1", "cc-a");
    useSessionStore.getState().opened("s2", "cc-b");
    useSessionStore.getState().outputReceived("s1", 1000);
    useSessionStore.getState().notifiedSent("s1");
    useSessionStore.getState().outputReceived("s1", 5000);
    expect(useSessionStore.getState().openSessions.get("s1")?.activity.notified).toBe(false);
  });
});

describe("queryChanged", () => {
  it("startet mit leerer Query", () => {
    expect(useSessionStore.getState().query).toBe("");
  });

  it("übernimmt die Query unverändert", () => {
    useSessionStore.getState().queryChanged("  WP ");
    expect(useSessionStore.getState().query).toBe("  WP ");
  });

  // Die Query darf die Datenlage nicht anfassen — sie ist reiner Anzeigefilter.
  it("lässt running und startable unberührt", () => {
    useSessionStore.getState().sessionsLoaded({ running: [running], startable: [startable] });
    useSessionStore.getState().queryChanged("passt-auf-nichts");
    const state = useSessionStore.getState();
    expect(state.running).toHaveLength(1);
    expect(state.startable).toHaveLength(1);
  });
});

describe("sortChanged", () => {
  it("sortiert per Vorgabe nach Namen", () => {
    expect(useSessionStore.getState().sortBy).toBe("name");
  });

  it("übernimmt den gewählten Sortierschlüssel", () => {
    useSessionStore.getState().sortChanged("lastActive");
    expect(useSessionStore.getState().sortBy).toBe("lastActive");
    useSessionStore.getState().sortChanged("created");
    expect(useSessionStore.getState().sortBy).toBe("created");
  });
});

describe("notifyToggled", () => {
  it("kehrt notifyEnabled für die gegebene Session um", () => {
    useSessionStore.getState().opened("s1", "cc-a");
    expect(useSessionStore.getState().openSessions.get("s1")?.notifyEnabled).toBe(true);
    useSessionStore.getState().notifyToggled("s1");
    expect(useSessionStore.getState().openSessions.get("s1")?.notifyEnabled).toBe(false);
    useSessionStore.getState().notifyToggled("s1");
    expect(useSessionStore.getState().openSessions.get("s1")?.notifyEnabled).toBe(true);
  });
});
