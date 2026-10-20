import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";

// `errorText` reaches i18n to translate an error key; the stub keeps this a
// pure-logic test (see `vitest.config.ts`) and makes a key read as itself.
vi.mock("$lib/i18n/index.svelte", () => ({ t: (key: string) => key }));

// Imported after the stub is registered, `vi.mock` being hoisted above a plain
// top-level import.
const { writeQueue } = await import("./writeQueue");

/** A write that waits until the test lets it go. */
function gate(): { promise: Promise<void>; open: () => void } {
  let open!: () => void;
  const promise = new Promise<void>((resolve) => (open = resolve));
  return { promise, open };
}

describe("writeQueue", () => {
  beforeEach(() => {
    vi.spyOn(console, "error").mockImplementation(() => {});
  });
  afterEach(() => {
    vi.restoreAllMocks();
  });

  // Rule: one write at a time, in the order of the gestures - the second one
  // builds on what the first one stored.
  it("runs the writes one after the other, in order", async () => {
    const log: string[] = [];
    const enqueue = writeQueue({ error: () => {} });
    const first = gate();
    enqueue("a", async () => {
      log.push("a starts");
      await first.promise;
      log.push("a ends");
    });
    const done = enqueue("b", async () => {
      log.push("b starts");
    });
    await Promise.resolve();
    expect(log, "b waits for a").toEqual(["a starts"]);
    first.open();
    await done;
    expect(log).toEqual(["a starts", "a ends", "b starts"]);
  });

  // Rule (golden rule n°6): a failed write is shown and logged, and does not
  // freeze the writes that follow it.
  it("reports a failure, then lets the next write through and clears it", async () => {
    const errors: string[] = [];
    const enqueue = writeQueue({ error: (text) => errors.push(text) });
    const failed = enqueue("save_brand_overlay", async () => {
      throw new Error("disk full");
    });
    await expect(failed, "the queue never rejects").resolves.toBeUndefined();
    expect(errors, "the failure reaches the tab").toEqual(["disk full"]);
    expect(console.error).toHaveBeenCalledWith("save_brand_overlay", expect.any(Error));

    let ran = false;
    await enqueue("next", async () => {
      ran = true;
    });
    expect(ran, "the next write still runs").toBe(true);
    expect(errors.at(-1), "and its success clears the message").toBe("");
  });

  // Rule: the busy flag covers each write, failed ones included.
  it("raises busy during each write and lowers it whatever the outcome", async () => {
    const busy: boolean[] = [];
    const enqueue = writeQueue({ busy: (b) => busy.push(b), error: () => {} });
    await enqueue("ok", async () => {});
    await enqueue("ko", async () => {
      throw "errors.libraryNotConfigured";
    });
    expect(busy).toEqual([true, false, true, false]);
  });
});
