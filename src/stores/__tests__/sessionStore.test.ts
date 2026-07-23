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
const startable: Project = { path: "/mnt/cache/appdata/habitbot", name: "habitbot" };

function reset() {
  useSessionStore.setState({
    running: [],
    startable: [],
    openSessions: new Map(),
    activeSessionId: null,
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
