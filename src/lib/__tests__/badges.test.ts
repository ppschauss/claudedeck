import { describe, expect, it } from "vitest";
import { activityState, onOutput, shouldNotify, type Activity } from "../badges";

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
    expect(shouldNotify(a, 1000 + 1900, true, false)).toBe(false);
  });

  it("liefert true ab der Schwelle (2,1s)", () => {
    const a: Activity = { badge: 1, lastOutputAt: 1000, notified: false };
    expect(shouldNotify(a, 1000 + 2100, true, false)).toBe(true);
  });

  it("liefert true exakt an der Schwelle (2,0s, >=)", () => {
    const a: Activity = { badge: 1, lastOutputAt: 1000, notified: false };
    expect(shouldNotify(a, 1000 + 2000, true, false)).toBe(true);
  });

  it("kein zweites Mal, wenn bereits notified", () => {
    const a: Activity = { badge: 1, lastOutputAt: 1000, notified: true };
    expect(shouldNotify(a, 1000 + 5000, true, false)).toBe(false);
  });

  it("nie, wenn Notifications deaktiviert sind", () => {
    const a: Activity = { badge: 1, lastOutputAt: 1000, notified: false };
    expect(shouldNotify(a, 1000 + 5000, false, false)).toBe(false);
  });

  it("nie, wenn noch kein Output stattfand (lastOutputAt === null)", () => {
    const a: Activity = { badge: 0, lastOutputAt: null, notified: false };
    expect(shouldNotify(a, 999999, true, false)).toBe(false);
  });

  it("nie, wenn die Session lost ist (Fix Minor, Task 6) — auch sonst über der Schwelle", () => {
    const a: Activity = { badge: 1, lastOutputAt: 1000, notified: false };
    expect(shouldNotify(a, 1000 + 5000, true, true)).toBe(false);
  });

  it("respektiert einen abweichenden thresholdMs-Parameter", () => {
    const a: Activity = { badge: 1, lastOutputAt: 1000, notified: false };
    expect(shouldNotify(a, 1000 + 500, true, false, 400)).toBe(true);
    expect(shouldNotify(a, 1000 + 300, true, false, 400)).toBe(false);
  });

  it("aktive Session: Badge bleibt 0 über onOutput, unabhängig von shouldNotify-Aufrufen", () => {
    // Die "aktive Session nie benachrichtigen"-Regel wird von den Aufrufern durchgesetzt
    // (Task 6 ruft shouldNotify nur für Hintergrund-Sessions auf) — hier wird nur
    // sichergestellt, dass onOutput für aktive Sessions den Badge nicht hochzählt.
    const a = onOutput(initial, 1000, true);
    expect(a.badge).toBe(0);
    expect(shouldNotify(a, 1000 + 5000, true, false)).toBe(true);
  });
});

describe("activityState", () => {
  const busy: Activity = { badge: 0, lastOutputAt: 10_000, notified: false };

  it("meldet 'idle', solange es nie Output gab", () => {
    expect(activityState(initial, 50_000, false)).toBe("idle");
  });

  it("meldet 'working', solange der letzte Output noch keine Sekunde her ist", () => {
    expect(activityState(busy, 10_500, false)).toBe("working");
  });

  // Derselbe Schwellenwert, aus dem heute die Benachrichtigung entsteht — beides bedeutet
  // "wartet vermutlich auf Eingabe", also darf es nicht auseinanderlaufen.
  it("meldet 'waiting', sobald der Schwellenwert erreicht ist", () => {
    expect(activityState(busy, 12_000, false)).toBe("waiting");
    expect(activityState(busy, 99_000, false)).toBe("waiting");
  });

  it("nutzt denselben Schwellenwert wie shouldNotify", () => {
    const now = 10_000 + 2000;
    expect(shouldNotify(busy, now, true, false)).toBe(true);
    expect(activityState(busy, now, false)).toBe("waiting");
  });

  it("respektiert einen abweichenden Schwellenwert", () => {
    expect(activityState(busy, 13_000, false, 5000)).toBe("working");
    expect(activityState(busy, 16_000, false, 5000)).toBe("waiting");
  });

  // Eine Session, die auf Reconnect wartet, hat keinen laufenden Prozess — sie ist weder
  // beschäftigt noch fertig, und ein Haken wäre dort schlicht gelogen.
  it("meldet 'lost' unabhängig von der Output-Zeit", () => {
    expect(activityState(busy, 10_500, true)).toBe("lost");
    expect(activityState(busy, 99_000, true)).toBe("lost");
    expect(activityState(initial, 99_000, true)).toBe("lost");
  });

  it("behandelt einen Zeitsprung rückwärts nicht als 'waiting'", () => {
    expect(activityState(busy, 9_000, false)).toBe("working");
  });
});
