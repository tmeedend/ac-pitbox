import { describe, expect, it } from "vitest";
import { sessionFallback } from "./sessionFallback";

// Rule (SESSION§1, §6.3bis): the session never names a mod that is gone, and
// never jumps because of a filter or of a card that was merely clicked.

const existing = ["a", "b", "c"];
const playable = ["a", "b", "c"];

describe("sessionFallback", () => {
  it("replaces a deleted session track even when the last card clicked still exists", () => {
    // Real bug: two tracks deleted from a multiple selection, the last card
    // clicked still there, the session column still naming a deleted track.
    expect(sessionFallback({ existing, playable, session: "gone", selected: "b" })).toEqual({
      clearSelected: false,
      pick: "a",
    });
  });

  it("forgets a deleted clicked card without moving a session that still exists", () => {
    expect(sessionFallback({ existing, playable, session: "b", selected: "gone" })).toEqual({
      clearSelected: true,
      pick: null,
    });
  });

  it("takes the first card that can be played, never one in the showcase", () => {
    // A showcase card cannot enter a session: picking it left the old choice.
    expect(sessionFallback({ existing, playable: ["c"], session: "gone", selected: null }).pick).toBe("c");
  });

  it("keeps a session mod in the showcase: it is still there, its column says so", () => {
    expect(sessionFallback({ existing, playable: ["a"], session: "b", selected: null }).pick).toBeNull();
  });

  it("picks one on a first start, and nothing once something is chosen", () => {
    expect(sessionFallback({ existing, playable, session: null, selected: null }).pick).toBe("a");
    expect(sessionFallback({ existing, playable, session: null, selected: "c" }).pick).toBeNull();
    expect(sessionFallback({ existing, playable, session: "c", selected: null }).pick).toBeNull();
  });

  it("does nothing when no card can be played", () => {
    expect(sessionFallback({ existing, playable: [], session: "gone", selected: null }).pick).toBeNull();
  });
});
