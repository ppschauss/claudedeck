import { describe, expect, it } from "vitest";
import { findOpenByName } from "../attachGuard";
import type { OpenSession } from "../../stores/sessionStore";

function entry(name: string): OpenSession {
  return { name, activity: { badge: 0, lastOutputAt: null, notified: false }, notifyEnabled: true, lost: false };
}

describe("findOpenByName", () => {
  it("liefert die sessionId einer bereits offenen Session mit diesem Namen", () => {
    const map = new Map<string, OpenSession>([
      ["s1", entry("cc-a")],
      ["s2", entry("cc-b")],
    ]);
    expect(findOpenByName(map, "cc-b")).toBe("s2");
  });

  it("liefert null, wenn keine offene Session diesen Namen trägt", () => {
    const map = new Map<string, OpenSession>([["s1", entry("cc-a")]]);
    expect(findOpenByName(map, "cc-x")).toBeNull();
  });

  it("liefert null bei leerer Map", () => {
    expect(findOpenByName(new Map(), "cc-a")).toBeNull();
  });
});
