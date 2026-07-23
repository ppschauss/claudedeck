import { describe, expect, it } from "vitest";
import { describeApiError, isApiError } from "../apiError";

describe("isApiError", () => {
  it("erkennt ein gültiges ApiError-Objekt", () => {
    expect(isApiError({ kind: "authFailed", message: "nein" })).toBe(true);
  });

  it("lehnt null/undefined/primitive Werte ab", () => {
    expect(isApiError(null)).toBe(false);
    expect(isApiError(undefined)).toBe(false);
    expect(isApiError("fehler")).toBe(false);
    expect(isApiError(42)).toBe(false);
  });

  it("lehnt Objekte ohne kind oder message ab", () => {
    expect(isApiError({ message: "nur message" })).toBe(false);
    expect(isApiError({ kind: "authFailed" })).toBe(false);
    expect(isApiError({})).toBe(false);
  });
});

describe("describeApiError", () => {
  it("liefert die message eines ApiError", () => {
    expect(describeApiError({ kind: "hostkeyUnknown", message: "Host-Key unbekannt" })).toBe(
      "Host-Key unbekannt",
    );
  });

  it("liefert die message einer normalen Error-Instanz", () => {
    expect(describeApiError(new Error("kaputt"))).toBe("kaputt");
  });

  it("fällt bei unbekannten Fehlertypen auf einen generischen Text zurück", () => {
    expect(describeApiError("irgendwas")).toBe("Unbekannter Fehler");
    expect(describeApiError(null)).toBe("Unbekannter Fehler");
    expect(describeApiError(undefined)).toBe("Unbekannter Fehler");
  });
});
