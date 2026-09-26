import { describe, expect, it } from "vitest";
import {
  normalizeProvenance,
  parseOwnerValue,
  PROVENANCE_KEY,
  STATE_KEY,
  stateOn,
  toBackendFilters,
  toggleState,
  withOwner,
} from "./filters";
import type { FilterMap } from "$lib/library/filters";

describe("game folder filters (DOSSIER§6)", () => {
  it("sends included and excluded states apart, and ignores unknown values", () => {
    const map: FilterMap = {
      [STATE_KEY]: {
        type: "val",
        op: "or",
        values: [
          { value: "drift", sign: 1 },
          { value: "waiting", sign: 1 },
          { value: "nobody", sign: -1 },
          { value: "gone", sign: 1 },
        ],
      },
    };
    expect(toBackendFilters(map)).toEqual({ states: ["drift", "waiting"], excluded: ["nobody"], owner: null });
  });

  it("keeps a single provenance: the last one posed with a plus", () => {
    const map: FilterMap = {
      [PROVENANCE_KEY]: {
        type: "val",
        op: "or",
        values: [
          { value: "car:rss_lanzo", sign: 1 },
          { value: "app:helicorsa", sign: -1 },
          { value: "car:rss_forza", sign: 1 },
        ],
      },
    };
    expect(toBackendFilters(map).owner).toEqual({ kind: "car", id: "rss_forza" });
    const normal = normalizeProvenance(map);
    expect(normal[PROVENANCE_KEY]).toEqual({ type: "val", op: "or", values: [{ value: "car:rss_forza", sign: 1 }] });
    expect(normalizeProvenance(normal)).toBe(normal);
  });

  it("drops a provenance left with only a minus", () => {
    const map: FilterMap = {
      [PROVENANCE_KEY]: { type: "val", op: "or", values: [{ value: "car:x", sign: -1 }] },
    };
    expect(PROVENANCE_KEY in normalizeProvenance(map)).toBe(false);
  });

  it("toggles a state from the summary band, and removes the chip when empty", () => {
    const once = toggleState({}, "drift");
    expect(stateOn(once, "drift")).toBe(true);
    const twice = toggleState(once, "drift");
    expect(STATE_KEY in twice).toBe(false);
  });

  it("reads back an owner value whose id holds a colon", () => {
    expect(parseOwnerValue("other:mods:weird")).toEqual({ kind: "other", id: "mods:weird" });
    expect(parseOwnerValue("nonsense")).toBeNull();
    expect(parseOwnerValue("planet:x")).toBeNull();
  });

  it("poses a provenance in place of the previous one", () => {
    const map = withOwner(withOwner({}, { kind: "car", id: "a" }), { kind: "track", id: "b" });
    expect(toBackendFilters(map).owner).toEqual({ kind: "track", id: "b" });
  });
});
