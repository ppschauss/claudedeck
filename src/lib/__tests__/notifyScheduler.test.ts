import { describe, expect, it } from "vitest";
import { decideSchedule } from "../notifyScheduler";
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
