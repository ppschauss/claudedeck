/**
 * Such- und Gruppierlogik für das Befehls-Panel — pure Funktionen wie `sessionFilter.ts`, damit
 * `CommandPanel.tsx` nur noch rendert.
 */
import type { CommandEntry, CommandKind } from "./ipc";
import { matchesQuery } from "./sessionFilter";

/** Nach Name **und** Beschreibung suchbare Teilmenge — hält die Funktion für Tests handlich. */
interface Searchable {
  name: string;
  description: string;
}

/**
 * Filtert nach Name oder Beschreibung. Die Beschreibung mitzudurchsuchen ist der eigentliche
 * Nutzen: man erinnert sich meist daran, *was* ein Skill tut, nicht wie er heißt.
 */
export function filterCatalog<T extends Searchable>(entries: readonly T[], query: string): T[] {
  return entries.filter(
    (entry) => matchesQuery(entry.name, query) || matchesQuery(entry.description, query),
  );
}

/** Ergebnis von [`groupByKind`] — jede Gruppe existiert immer, notfalls leer. */
export type GroupedCommands = Record<CommandKind, CommandEntry[]>;

/**
 * Teilt die Einträge in die Akkordeon-Gruppen auf. Innerhalb einer Gruppe stehen projektlokale
 * Einträge vorn — sie sind der speziellere Fall und gingen in einer langen globalen Liste sonst
 * unter. Bei gleichem Scope entscheidet der Name.
 */
export function groupByKind(entries: readonly CommandEntry[]): GroupedCommands {
  const grouped: GroupedCommands = { skill: [], agent: [], command: [] };
  for (const entry of entries) {
    grouped[entry.kind].push(entry);
  }

  for (const kind of Object.keys(grouped) as CommandKind[]) {
    grouped[kind].sort((a, b) => {
      if (a.scope !== b.scope) return a.scope === "project" ? -1 : 1;
      return a.name.localeCompare(b.name, "de", { sensitivity: "base" });
    });
  }

  return grouped;
}
