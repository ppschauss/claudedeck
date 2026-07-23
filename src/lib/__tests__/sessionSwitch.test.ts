import { describe, expect, it } from "vitest";
import { nextActiveSessionId } from "../sessionSwitch";

describe("nextActiveSessionId", () => {
  it("wählt die Session, die an die Stelle der mittleren geschlossenen nachrückt", () => {
    expect(nextActiveSessionId(["a", "b", "c"], "b")).toBe("c");
  });

  it("wählt die vorherige Session, wenn die letzte geschlossen wird", () => {
    expect(nextActiveSessionId(["a", "b", "c"], "c")).toBe("b");
  });

  it("wählt die nachrückende Session, wenn die erste geschlossen wird", () => {
    expect(nextActiveSessionId(["a", "b", "c"], "a")).toBe("b");
  });

  it("liefert null, wenn die einzige offene Session geschlossen wird", () => {
    expect(nextActiveSessionId(["a"], "a")).toBeNull();
  });

  it("liefert null bei leerer Liste", () => {
    expect(nextActiveSessionId([], "a")).toBeNull();
  });

  it("fällt auf die erste verbleibende Session zurück, wenn closingId unbekannt war", () => {
    expect(nextActiveSessionId(["a", "b"], "x")).toBe("a");
  });

  it("liefert null, wenn closingId unbekannt war und keine andere Session existiert", () => {
    expect(nextActiveSessionId([], "x")).toBeNull();
  });
});
