import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { debounceTrailing } from "../debounce";

describe("debounceTrailing", () => {
  beforeEach(() => {
    vi.useFakeTimers();
  });

  afterEach(() => {
    vi.useRealTimers();
  });

  it("ruft die Funktion nach mehreren Aufrufen innerhalb des Zeitfensters genau einmal auf, mit den letzten Argumenten", () => {
    const fn = vi.fn();
    const debounced = debounceTrailing(fn, 100);

    debounced(1, "a");
    vi.advanceTimersByTime(30);
    debounced(2, "b");
    vi.advanceTimersByTime(30);
    debounced(3, "c");

    expect(fn).not.toHaveBeenCalled();

    vi.advanceTimersByTime(100);

    expect(fn).toHaveBeenCalledTimes(1);
    expect(fn).toHaveBeenCalledWith(3, "c");
  });

  it("ruft die Funktion erneut auf, wenn zwischen zwei Aufrufen mehr als `ms` vergangen ist", () => {
    const fn = vi.fn();
    const debounced = debounceTrailing(fn, 100);

    debounced("erst");
    vi.advanceTimersByTime(100);
    expect(fn).toHaveBeenCalledTimes(1);
    expect(fn).toHaveBeenLastCalledWith("erst");

    debounced("zweit");
    vi.advanceTimersByTime(100);
    expect(fn).toHaveBeenCalledTimes(2);
    expect(fn).toHaveBeenLastCalledWith("zweit");
  });

  it("löst noch nicht aus, bevor die volle Zeitspanne vergangen ist", () => {
    const fn = vi.fn();
    const debounced = debounceTrailing(fn, 100);

    debounced("x");
    vi.advanceTimersByTime(99);
    expect(fn).not.toHaveBeenCalled();
  });
});
