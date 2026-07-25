import { describe, expect, it } from "vitest";
import { fileIcon, fileKind, formatAge, formatSize } from "../fileKind";

describe("fileKind", () => {
  it("erkennt Bilder an der Endung", () => {
    for (const n of ["a.png", "b.jpg", "c.jpeg", "d.gif", "e.webp", "f.svg", "g.avif"]) {
      expect(fileKind(n), n).toBe("image");
    }
  });

  it("erkennt Endungen unabhängig von der Schreibweise", () => {
    expect(fileKind("DIAGRAMM.PNG")).toBe("image");
    expect(fileKind("Report.Md")).toBe("text");
  });

  it("erkennt Text und Archive", () => {
    expect(fileKind("notiz.md")).toBe("text");
    expect(fileKind("main.rs")).toBe("text");
    expect(fileKind("paket.zip")).toBe("archive");
    expect(fileKind("setup.msi")).toBe("archive");
  });

  it("liefert 'other' für Unbekanntes und für Dateien ohne Endung", () => {
    expect(fileKind("programm.exe")).toBe("other");
    expect(fileKind("README")).toBe("other");
    expect(fileKind("")).toBe("other");
  });

  // Ein führender Punkt gehört zum Namen, nicht zur Endung — sonst gälte ".png" als Bild.
  it("behandelt Punktdateien nicht als Endung", () => {
    expect(fileKind(".bashrc")).toBe("other");
    expect(fileKind(".png")).toBe("other");
  });
});

describe("fileIcon", () => {
  it("nimmt für Ordner immer das Ordnersymbol", () => {
    expect(fileIcon("image", true)).toBe(fileIcon("other", true));
  });

  it("unterscheidet die Dateiarten", () => {
    const icons = new Set([
      fileIcon("image", false),
      fileIcon("text", false),
      fileIcon("archive", false),
      fileIcon("other", false),
    ]);
    expect(icons.size).toBe(4);
  });
});

describe("formatSize", () => {
  it("zeigt Bytes unter einem Kilobyte", () => {
    expect(formatSize(0)).toBe("0 B");
    expect(formatSize(512)).toBe("512 B");
  });

  it("rechnet in KB, MB und GB um", () => {
    expect(formatSize(2048)).toBe("2,0 KB");
    expect(formatSize(5 * 1024 * 1024)).toBe("5,0 MB");
    expect(formatSize(3 * 1024 * 1024 * 1024)).toBe("3,0 GB");
  });
});

describe("formatAge", () => {
  const now = 1_800_000_000_000; // ms

  it("zeigt frische Dateien als 'gerade eben'", () => {
    expect(formatAge(now / 1000 - 5, now)).toBe("gerade eben");
  });

  it("zeigt Minuten, Stunden und Tage", () => {
    expect(formatAge(now / 1000 - 120, now)).toBe("vor 2 Min");
    expect(formatAge(now / 1000 - 7200, now)).toBe("vor 2 Std");
    expect(formatAge(now / 1000 - 3 * 86400, now)).toBe("vor 3 Tagen");
  });

  it("nutzt Einzahl bei genau einem Tag", () => {
    expect(formatAge(now / 1000 - 86400, now)).toBe("vor 1 Tag");
  });

  // Ein Server ohne Zeitangabe liefert 0 — das darf nicht als „1970" erscheinen.
  it("liefert nichts, wenn kein Zeitstempel vorliegt", () => {
    expect(formatAge(0, now)).toBe("");
  });

  // Uhren zwischen Server und Client laufen selten exakt gleich.
  it("behandelt eine Zeit knapp in der Zukunft als gerade eben", () => {
    expect(formatAge(now / 1000 + 30, now)).toBe("gerade eben");
  });
});
