import { describe, expect, it } from "vitest";
import { basenameRemote, joinRemote, parentRemote } from "../paths";

describe("remote paths", () => {
  it("joint Verzeichnis und Name", () => {
    expect(joinRemote("/mnt/cache/appdata", "otakupulse")).toBe("/mnt/cache/appdata/otakupulse");
  });
  it("joint an der Wurzel ohne Doppel-Slash", () => {
    expect(joinRemote("/", "etc")).toBe("/etc");
  });
  it("ignoriert trailing Slashes beim Join", () => {
    expect(joinRemote("/tmp/", "x")).toBe("/tmp/x");
  });
  it("liefert das Parent-Verzeichnis", () => {
    expect(parentRemote("/mnt/cache/appdata")).toBe("/mnt/cache");
  });
  it("Parent der Wurzel bleibt die Wurzel", () => {
    expect(parentRemote("/")).toBe("/");
    expect(parentRemote("/etc")).toBe("/");
  });
  it("liefert den Basename", () => {
    expect(basenameRemote("/a/b/c.txt")).toBe("c.txt");
    expect(basenameRemote("/")).toBe("/");
  });
});
