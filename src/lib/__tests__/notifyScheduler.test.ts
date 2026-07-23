import { describe, expect, it } from "vitest";
import { decideFire, decideSchedule } from "../notifyScheduler";
import type { OpenSession } from "../../stores/sessionStore";

function session(overrides: Partial<OpenSession> = {}): OpenSession {
  return {
    name: "cc-a",
    activity: { badge: 1, lastOutputAt: 1000, notified: false },
    notifyEnabled: true,
    lost: false,
    ...overrides,
  };
}

describe("decideSchedule (pure)", () => {
  it("plant einen Timer für eine eligible Hintergrund-Session ohne laufenden Timer", () => {
    const openSessions = new Map([["s1", session()]]);
    const decision = decideSchedule(openSessions, "s2", new Set());
    expect(decision.toSchedule).toEqual(["s1"]);
    expect(decision.toCancel).toEqual([]);
  });

  it("plant KEINEN Timer für die aktive Session", () => {
    const openSessions = new Map([["s1", session()]]);
    const decision = decideSchedule(openSessions, "s1", new Set());
    expect(decision.toSchedule).toEqual([]);
  });

  it("plant KEINEN Timer, wenn Notifications deaktiviert sind", () => {
    const openSessions = new Map([["s1", session({ notifyEnabled: false })]]);
    const decision = decideSchedule(openSessions, "s2", new Set());
    expect(decision.toSchedule).toEqual([]);
  });

  it("plant KEINEN Timer, wenn bereits benachrichtigt wurde", () => {
    const openSessions = new Map([
      ["s1", session({ activity: { badge: 1, lastOutputAt: 1000, notified: true } })],
    ]);
    const decision = decideSchedule(openSessions, "s2", new Set());
    expect(decision.toSchedule).toEqual([]);
  });

  it("plant KEINEN Timer ohne bisherigen Output (lastOutputAt null)", () => {
    const openSessions = new Map([
      ["s1", session({ activity: { badge: 0, lastOutputAt: null, notified: false } })],
    ]);
    const decision = decideSchedule(openSessions, "s2", new Set());
    expect(decision.toSchedule).toEqual([]);
  });

  it("plant KEINEN Timer für eine lost-Session (Fix Minor, Task 6)", () => {
    const openSessions = new Map([["s1", session({ lost: true })]]);
    const decision = decideSchedule(openSessions, "s2", new Set());
    expect(decision.toSchedule).toEqual([]);
    expect(decision.toCancel).toEqual([]);
  });

  it("cancelt einen laufenden Timer, wenn die Session inzwischen lost wurde (Fix Minor, Task 6)", () => {
    const openSessions = new Map([["s1", session({ lost: true })]]);
    const decision = decideSchedule(openSessions, "s2", new Set(["s1"]));
    expect(decision.toCancel).toEqual(["s1"]);
  });

  it("plant KEINEN zweiten Timer für eine bereits geplante Session", () => {
    const openSessions = new Map([["s1", session()]]);
    const decision = decideSchedule(openSessions, "s2", new Set(["s1"]));
    expect(decision.toSchedule).toEqual([]);
    expect(decision.toCancel).toEqual([]);
  });

  it("cancelt den Timer, wenn die Session inzwischen aktiv wurde", () => {
    const openSessions = new Map([["s1", session()]]);
    const decision = decideSchedule(openSessions, "s1", new Set(["s1"]));
    expect(decision.toCancel).toEqual(["s1"]);
  });

  it("cancelt den Timer, wenn notifyEnabled inzwischen aus ist", () => {
    const openSessions = new Map([["s1", session({ notifyEnabled: false })]]);
    const decision = decideSchedule(openSessions, "s2", new Set(["s1"]));
    expect(decision.toCancel).toEqual(["s1"]);
  });

  it("cancelt den Timer für eine sessionId, die nicht mehr offen ist (Leak-Schutz)", () => {
    const openSessions = new Map<string, OpenSession>();
    const decision = decideSchedule(openSessions, null, new Set(["gone"]));
    expect(decision.toCancel).toEqual(["gone"]);
  });

  it("mehrere Sessions gemischt: nur die eligible ohne Timer landen in toSchedule", () => {
    const openSessions = new Map([
      ["s1", session()], // eligible, kein Timer -> schedule
      ["s2", session()], // eligible, schon geplant -> weder noch
      ["s3", session({ notifyEnabled: false })], // nicht eligible, kein Timer -> weder noch
    ]);
    const decision = decideSchedule(openSessions, "active", new Set(["s2"]));
    expect(decision.toSchedule).toEqual(["s1"]);
    expect(decision.toCancel).toEqual([]);
  });
});

describe("decideFire (pure, Fix I-1)", () => {
  it("benachrichtigt, wenn der Schwellenwert seit lastOutputAt erreicht ist", () => {
    const entry = session({ activity: { badge: 1, lastOutputAt: 1000, notified: false } });
    const decision = decideFire(entry, "s2", "s1", 3000);
    expect(decision).toEqual({ action: "notify" });
  });

  it("cancelt, wenn die Session inzwischen nicht mehr existiert (entry undefined)", () => {
    const decision = decideFire(undefined, "s2", "s1", 3000);
    expect(decision).toEqual({ action: "cancel" });
  });

  it("cancelt statt neu zu planen, wenn die Session beim Ablehnen inzwischen aktiv wurde", () => {
    // Schwellenwert noch nicht erreicht (now - lastOutputAt = 500 < 2000) -> shouldNotify lehnt
    // ab, decideFire prüft dann Eligibility neu: id === activeSessionId -> nicht eligible.
    const entry = session({ activity: { badge: 0, lastOutputAt: 2500, notified: false } });
    const decision = decideFire(entry, "s1", "s1", 3000);
    expect(decision).toEqual({ action: "cancel" });
  });

  it("cancelt, wenn bereits benachrichtigt wurde", () => {
    const entry = session({ activity: { badge: 1, lastOutputAt: 500, notified: true } });
    const decision = decideFire(entry, "s2", "s1", 3000);
    expect(decision).toEqual({ action: "cancel" });
  });

  it(
    "plant NEU relativ zu lastOutputAt statt die Notification zu verlieren, wenn seit dem " +
      "Planen des Timers neuer Output kam (Fix I-1, Review-Fund Task 7)",
    () => {
      const entry = session({ activity: { badge: 2, lastOutputAt: 1500, notified: false } });
      // Timer wurde bei t=0 für t=2000 geplant, feuert jetzt bei t=2000 — seit t=1500 kam aber
      // neuer Output rein, der Schwellenwert (1500 + 2000 = 3500) ist noch nicht erreicht.
      const decision = decideFire(entry, "s2", "s1", 2000);
      expect(decision).toEqual({ action: "reschedule", delayMs: 1500 });
    },
  );

  it("feuert beim neu geplanten Timer dann tatsächlich (Fix I-1: t=3500, nicht bei t=2000 verworfen)", () => {
    const entry = session({ activity: { badge: 2, lastOutputAt: 1500, notified: false } });
    // Der bei t=2000 neu geplante Timer (delayMs=1500) feuert bei t=3500.
    const decision = decideFire(entry, "s2", "s1", 3500);
    expect(decision).toEqual({ action: "notify" });
  });

  it("cancelt statt neu zu planen, wenn die Session beim Ablehnen nicht mehr eligible ist (z.B. notifyEnabled aus)", () => {
    const entry = session({
      notifyEnabled: false,
      activity: { badge: 2, lastOutputAt: 1500, notified: false },
    });
    const decision = decideFire(entry, "s2", "s1", 2000);
    expect(decision).toEqual({ action: "cancel" });
  });
});
