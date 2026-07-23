import { describe, expect, it } from "vitest";
import { onOutput, shouldNotify, type Activity } from "../badges";

const initial: Activity = { badge: 0, lastOutputAt: null, notified: false };

describe("onOutput", () => {
  it("erhöht den Badge, wenn die Session inaktiv ist", () => {
    const a = onOutput(initial, 1000, false);
    expect(a.badge).toBe(1);
    const b = onOutput(a, 2000, false);
    expect(b.badge).toBe(2);
  });

  it("hält den Badge bei 0, solange die Session aktiv ist", () => {
    const a = onOutput(initial, 1000, true);
    expect(a.badge).toBe(0);
    const b = onOutput(a, 2000, true);
    expect(b.badge).toBe(0);
  });

  it("setzt lastOutputAt auf now, aktiv wie inaktiv", () => {
    expect(onOutput(initial, 1234, false).lastOutputAt).toBe(1234);
    expect(onOutput(initial, 5678, true).lastOutputAt).toBe(5678);
  });

  it("setzt notified bei neuem Output immer auf false (auch nach vorherigem Notify)", () => {
    const notified: Activity = { badge: 3, lastOutputAt: 1000, notified: true };
    const a = onOutput(notified, 2000, false);
    expect(a.notified).toBe(false);
  });
});

describe("shouldNotify", () => {
  it("liefert false unterhalb der Schwelle (1,9s)", () => {
    const a: Activity = { badge: 1, lastOutputAt: 1000, notified: false };
    expect(shouldNotify(a, 1000 + 1900, true)).toBe(false);
  });

  it("liefert true ab der Schwelle (2,1s)", () => {
    const a: Activity = { badge: 1, lastOutputAt: 1000, notified: false };
    expect(shouldNotify(a, 1000 + 2100, true)).toBe(true);
  });

  it("liefert true exakt an der Schwelle (2,0s, >=)", () => {
    const a: Activity = { badge: 1, lastOutputAt: 1000, notified: false };
    expect(shouldNotify(a, 1000 + 2000, true)).toBe(true);
  });

  it("kein zweites Mal, wenn bereits notified", () => {
    const a: Activity = { badge: 1, lastOutputAt: 1000, notified: true };
    expect(shouldNotify(a, 1000 + 5000, true)).toBe(false);
  });

  it("nie, wenn Notifications deaktiviert sind", () => {
    const a: Activity = { badge: 1, lastOutputAt: 1000, notified: false };
    expect(shouldNotify(a, 1000 + 5000, false)).toBe(false);
  });

  it("nie, wenn noch kein Output stattfand (lastOutputAt === null)", () => {
    const a: Activity = { badge: 0, lastOutputAt: null, notified: false };
    expect(shouldNotify(a, 999999, true)).toBe(false);
  });

  it("respektiert einen abweichenden thresholdMs-Parameter", () => {
    const a: Activity = { badge: 1, lastOutputAt: 1000, notified: false };
    expect(shouldNotify(a, 1000 + 500, true, 400)).toBe(true);
    expect(shouldNotify(a, 1000 + 300, true, 400)).toBe(false);
  });

  it("aktive Session: Badge bleibt 0 über onOutput, unabhängig von shouldNotify-Aufrufen", () => {
    // Die "aktive Session nie benachrichtigen"-Regel wird von den Aufrufern durchgesetzt
    // (Task 6 ruft shouldNotify nur für Hintergrund-Sessions auf) — hier wird nur
    // sichergestellt, dass onOutput für aktive Sessions den Badge nicht hochzählt.
    const a = onOutput(initial, 1000, true);
    expect(a.badge).toBe(0);
    expect(shouldNotify(a, 1000 + 5000, true)).toBe(true);
  });
});
