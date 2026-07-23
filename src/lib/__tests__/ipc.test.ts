import { describe, expect, it } from "vitest";
import { b64ToBytes, bytesToB64 } from "../ipc";

describe("bytesToB64 / b64ToBytes", () => {
  it("roundtrip für reine ASCII-Bytes", () => {
    const bytes = new TextEncoder().encode("hello world");
    expect(b64ToBytes(bytesToB64(bytes))).toEqual(bytes);
  });

  it("roundtrip für Nicht-ASCII-Bytes (0x00, 0xFF, mittlere Werte)", () => {
    const bytes = new Uint8Array([0x00, 0xff, 0x01, 0x7f, 0x80, 0xde, 0xad, 0xbe, 0xef]);
    expect(b64ToBytes(bytesToB64(bytes))).toEqual(bytes);
  });

  it("roundtrip für UTF-8-kodierte Umlaute (mehrere Bytes pro Zeichen)", () => {
    const bytes = new TextEncoder().encode("äöü€ß — Prüfung");
    const roundtripped = b64ToBytes(bytesToB64(bytes));
    expect(roundtripped).toEqual(bytes);
    expect(new TextDecoder().decode(roundtripped)).toBe("äöü€ß — Prüfung");
  });

  it("erzeugt Standard-Base64 mit Padding, kompatibel zu data-encoding::BASE64 (Rust)", () => {
    // "Ma" -> 2 Bytes -> ein "=" Padding; bekannter Vektor gegen RFC 4648 §4 geprüft.
    expect(bytesToB64(new TextEncoder().encode("Ma"))).toBe("TWE=");
    expect(bytesToB64(new TextEncoder().encode("Man"))).toBe("TWFu");
  });

  it("leeres Array roundtrip", () => {
    const bytes = new Uint8Array([]);
    expect(bytesToB64(bytes)).toBe("");
    expect(b64ToBytes("")).toEqual(bytes);
  });

  it("größere zufällige Byte-Folge (Chunking-Pfad, >0x8000 falls implementiert) bleibt stabil", () => {
    const bytes = new Uint8Array(70000);
    for (let i = 0; i < bytes.length; i++) bytes[i] = i % 256;
    expect(b64ToBytes(bytesToB64(bytes))).toEqual(bytes);
  });
});
