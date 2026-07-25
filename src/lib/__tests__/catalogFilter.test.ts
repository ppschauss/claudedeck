import { describe, expect, it } from "vitest";
import { filterCatalog, groupByKind } from "../catalogFilter";
import type { CommandEntry } from "../ipc";

function entry(
  name: string,
  description = "",
  kind: CommandEntry["kind"] = "skill",
  scope: CommandEntry["scope"] = "global",
): CommandEntry {
  return { name, description, kind, scope };
}

describe("filterCatalog", () => {
  const entries = [
    entry("homelab-service", "Scaffold a self-hosted container"),
    entry("wp-theme-dev", "WordPress Block-Theme"),
  ];

  it("liefert alles bei leerer Query", () => {
    expect(filterCatalog(entries, "")).toHaveLength(2);
  });

  it("matcht den Namen", () => {
    expect(filterCatalog(entries, "homelab").map((e) => e.name)).toEqual(["homelab-service"]);
  });

  // Man erinnert sich oft an das, was ein Skill tut, nicht an seinen Namen.
  it("matcht auch die Beschreibung", () => {
    expect(filterCatalog(entries, "wordpress").map((e) => e.name)).toEqual(["wp-theme-dev"]);
  });

  it("liefert eine leere Liste, wenn nichts passt", () => {
    expect(filterCatalog(entries, "kubernetes")).toEqual([]);
  });

  it("lässt die Eingabeliste unverändert", () => {
    filterCatalog(entries, "homelab");
    expect(entries).toHaveLength(2);
  });
});

describe("groupByKind", () => {
  it("verteilt Einträge auf die drei Gruppen", () => {
    const entries = [
      entry("a-skill", "", "skill"),
      entry("Explore", "", "agent"),
      entry("deploy", "", "command"),
      entry("b-skill", "", "skill"),
    ];
    const grouped = groupByKind(entries);
    expect(grouped.skill.map((e) => e.name)).toEqual(["a-skill", "b-skill"]);
    expect(grouped.agent.map((e) => e.name)).toEqual(["Explore"]);
    expect(grouped.command.map((e) => e.name)).toEqual(["deploy"]);
  });

  it("liefert für jede Gruppe ein Array, auch wenn sie leer ist", () => {
    const grouped = groupByKind([]);
    expect(grouped.skill).toEqual([]);
    expect(grouped.agent).toEqual([]);
    expect(grouped.command).toEqual([]);
  });

  // Projektlokale Einträge sollen vor den globalen stehen: sie sind der speziellere Fall und
  // gehen in einer langen globalen Liste sonst unter.
  it("stellt projektlokale Einträge vor die globalen", () => {
    const entries = [
      entry("global-b", "", "skill", "global"),
      entry("projekt", "", "skill", "project"),
      entry("global-a", "", "skill", "global"),
    ];
    expect(groupByKind(entries).skill.map((e) => e.name)).toEqual([
      "projekt",
      "global-a",
      "global-b",
    ]);
  });
});
