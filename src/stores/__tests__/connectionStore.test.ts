import { beforeEach, describe, expect, it } from "vitest";
import type { ConnectionStateEvent } from "../../lib/ipc";
import {
  initialConnectionState,
  reduceConnectionState,
  tickRetryCountdown,
  useConnectionStore,
} from "../connectionStore";

describe("reduceConnectionState (pure)", () => {
  it("übernimmt state + attempt + nextRetryInS aus dem Event", () => {
    const event: ConnectionStateEvent = { state: "reconnecting", attempt: 2, nextRetryInS: 6 };
    const next = reduceConnectionState(initialConnectionState, event);
    expect(next).toEqual({ state: "reconnecting", attempt: 2, nextRetryInS: 6 });
  });

  it("setzt attempt/nextRetryInS auf null zurück, wenn das Event sie nicht mitliefert", () => {
    const prev = { state: "reconnecting" as const, attempt: 2, nextRetryInS: 6 };
    const next = reduceConnectionState(prev, { state: "connected" });
    expect(next).toEqual({ state: "connected", attempt: null, nextRetryInS: null });
  });

  it("disconnected → connecting → connected: kein Leck von attempt/nextRetryInS", () => {
    let s = initialConnectionState;
    s = reduceConnectionState(s, { state: "connecting" });
    expect(s).toEqual({ state: "connecting", attempt: null, nextRetryInS: null });
    s = reduceConnectionState(s, { state: "connected" });
    expect(s).toEqual({ state: "connected", attempt: null, nextRetryInS: null });
  });
});

describe("tickRetryCountdown (pure)", () => {
  it("zählt nextRetryInS um 1 herunter", () => {
    const s = { state: "reconnecting" as const, attempt: 1, nextRetryInS: 3 };
    expect(tickRetryCountdown(s).nextRetryInS).toBe(2);
  });

  it("bleibt bei 0 stehen (kein Negativwert)", () => {
    const s = { state: "reconnecting" as const, attempt: 1, nextRetryInS: 0 };
    expect(tickRetryCountdown(s).nextRetryInS).toBe(0);
  });

  it("ist ein No-Op, wenn kein Countdown läuft (nextRetryInS === null)", () => {
    const s = { state: "connected" as const, attempt: null, nextRetryInS: null };
    expect(tickRetryCountdown(s)).toEqual(s);
  });
});

describe("useConnectionStore", () => {
  beforeEach(() => {
    useConnectionStore.setState({ connectionState: initialConnectionState });
  });

  it("eventReceived wendet den Reducer auf den State an", () => {
    useConnectionStore.getState().eventReceived({ state: "failed" });
    expect(useConnectionStore.getState().connectionState.state).toBe("failed");
  });

  it("tick dekrementiert den Countdown im Store", () => {
    useConnectionStore.getState().eventReceived({ state: "reconnecting", attempt: 1, nextRetryInS: 3 });
    useConnectionStore.getState().tick();
    expect(useConnectionStore.getState().connectionState.nextRetryInS).toBe(2);
  });
});
