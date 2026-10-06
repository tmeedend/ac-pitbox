import { describe, expect, it, vi } from "vitest";
import { readOnce } from "./readOnce";

/** A fetch the test answers by hand, and counts. */
function controlled<T>() {
  const calls: Array<{ resolve: (v: T) => void; reject: (e: unknown) => void }> = [];
  const fetch = () => new Promise<T>((resolve, reject) => calls.push({ resolve, reject }));
  return { fetch, calls };
}

const tick = () => new Promise((r) => setTimeout(r, 0));

describe("readOnce", () => {
  it("reads once, and never again after a usable answer", async () => {
    const { fetch, calls } = controlled<string[]>();
    const applied: string[][] = [];
    const load = readOnce(fetch, (v) => applied.push(v), { label: "t" });
    const first = load();
    void load();
    calls[0].resolve(["France"]);
    await first;
    await load();
    expect(calls.length, "one read for every caller").toBe(1);
    expect(applied, "the answer applied once").toEqual([["France"]]);
  });

  /** The bug: a fallback taken on a slow backend was remembered for the whole
   * session, and the country index never got its flags back. */
  it("lets a slow read finish after its caller was let go", async () => {
    vi.useFakeTimers();
    try {
      const { fetch, calls } = controlled<string[]>();
      const applied: string[][] = [];
      const load = readOnce(fetch, (v) => applied.push(v), { label: "t", waitMs: 50 });
      const waiting = load();
      await vi.advanceTimersByTimeAsync(50);
      await waiting;
      expect(applied, "nothing yet when the caller is let go").toEqual([]);
      calls[0].resolve(["Italy"]);
      await vi.advanceTimersByTimeAsync(0);
      expect(applied, "the late answer is applied all the same").toEqual([["Italy"]]);
      await load();
      expect(calls.length, "and not asked again").toBe(1);
    } finally {
      vi.useRealTimers();
    }
  });

  it("asks again after an error", async () => {
    const { fetch, calls } = controlled<string[]>();
    const warn = vi.spyOn(console, "warn").mockImplementation(() => {});
    const load = readOnce(fetch, () => {}, { label: "t" });
    const first = load();
    calls[0].reject(new Error("boom"));
    await first;
    await tick();
    void load();
    expect(calls.length, "a failed read is not remembered").toBe(2);
    warn.mockRestore();
  });

  /** An empty country table: Assetto Corsa not configured yet, or its file
   * unreadable at that moment — nothing to keep for the session. */
  it("asks again after an answer that is not usable", async () => {
    const { fetch, calls } = controlled<string[]>();
    const applied: string[][] = [];
    const load = readOnce(fetch, (v) => applied.push(v), { label: "t", usable: (v) => v.length > 0 });
    const first = load();
    calls[0].resolve([]);
    await first;
    void load();
    expect(calls.length, "an unusable answer is asked again").toBe(2);
    expect(applied, "and never applied").toEqual([]);
  });
});
