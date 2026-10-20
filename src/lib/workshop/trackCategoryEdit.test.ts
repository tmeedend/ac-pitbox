import { describe, expect, it } from "vitest";
import type { CategoryRow, RuleRow, TagMerge } from "./rules";
import {
  addKeyword,
  goneKeywords,
  isCurated,
  keywordsOf,
  producers,
  removeKeyword,
  restoreKeyword,
  setLook,
} from "./trackCategoryEdit";

const row = (name: string, rank: number | null, on = true): CategoryRow => ({
  name,
  shipped: rank !== null,
  on,
  effect: 0,
  masked: 0,
  shipped_rank: rank,
});

describe("Families › Tracks", () => {
  // The ⚑: a decision of the user's on this category.
  it("flags a category the user named, switched off, made or moved", () => {
    const rows = [row("#rally", 0), row("#drift", 1), row("#touge", 2)];
    expect(rows.map((r) => isCurated(r, rows, {}))).toEqual([false, false, false]);
    expect(isCurated(rows[1], rows, { "#drift": { name: "Glisse" } }), "named").toBe(true);
    expect(isCurated(row("#oval", 3, false), rows, {}), "switched off").toBe(true);
    expect(isCurated(row("#mine", null), rows, {}), "his own").toBe(true);
    const moved = [row("#drift", 1), row("#rally", 0), row("#touge", 2)];
    expect(moved.map((r) => isCurated(r, moved, {})), "the two swapped, not the third").toEqual([true, true, false]);
  });

  it("finds the merge rules that produce a category, bare or with its #", () => {
    const rule = (id: string, to: string[]): RuleRow<TagMerge> => ({
      rule: { id, from: ["x"], to },
      origin: "catalog",
      disabled: false,
      outdated: false,
      effect: 3,
      masked: 0,
    });
    const rules = [rule("a", ["drift"]), rule("b", ["#Drift", "jdm"]), rule("c", ["touge"])];
    expect(producers(rules, "#drift").map((r) => r.rule.id)).toEqual(["a", "b"]);
  });

  it("keeps a name or an icon, and forgets an emptied one", () => {
    const looks = setLook({}, "#drift", { name: "Glisse" });
    expect(looks).toEqual({ "#drift": { name: "Glisse" } });
    expect(setLook(looks, "#drift", { icon: "touge" })).toEqual({ "#drift": { name: "Glisse", icon: "touge" } });
    expect(setLook(looks, "#drift", { name: "  " }), "nothing left: no entry").toEqual({});
  });

  // The Online page's words, edited as decisions over the catalogue.
  it("edits the server words as decisions over the catalogue", () => {
    const catalog = { drift: "#drift", traffic: "#freeroam" };
    let o = addKeyword({}, catalog, " Sideways ", "#drift");
    expect(o.set).toEqual({ sideways: "#drift" });
    const effective = { ...catalog, ...o.set };
    expect(keywordsOf(effective, "#drift")).toEqual(["drift", "sideways"]);

    o = removeKeyword(o, catalog, "drift");
    expect(o.removed, "a shipped word gets a tombstone").toEqual(["drift"]);
    expect(goneKeywords(catalog, { traffic: "#freeroam", sideways: "#drift" }, "#drift")).toEqual(["drift"]);

    o = restoreKeyword(o, "drift");
    expect(o.removed).toEqual([]);
    expect(addKeyword({}, catalog, "drift", "#drift"), "restating the catalogue is no decision").toEqual({
      set: {},
      removed: [],
    });
    expect(removeKeyword({ set: { sideways: "#drift" } }, catalog, "sideways"), "his own word just goes").toEqual({
      set: {},
      removed: [],
    });
  });
});
