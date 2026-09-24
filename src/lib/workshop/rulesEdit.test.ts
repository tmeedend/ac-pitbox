// The Rules screen's gestures as overlay decisions (REGLES§5, §8): switching
// off, forking and deleting are three different things, and confusing them
// would lose improvements (a fork is frozen) or a rule (a shipped rule is
// never deleted).
import { describe, expect, it } from "vitest";
import {
  addCategory,
  addRule,
  editRule,
  fromForm,
  moveCategory,
  removeRule,
  restoreDefault,
  setCatalogOn,
  setCategoryOn,
  setEnabled,
  toForm,
} from "./rulesEdit";
import type { RulesOverlay } from "./rules";

const bayro = { id: "pitbox.brand.bayro", name_contains: "bayro", set_brand: "BMW" };

describe("rules overlay gestures", () => {
  it("switching a rule off and on leaves no decision behind", () => {
    const off = setEnabled({}, "brand_fix", bayro.id, false);
    expect(off.brand_fix?.disabled).toEqual([bayro.id]);
    const on = setEnabled(off, "brand_fix", bayro.id, true);
    expect(on.brand_fix?.disabled).toEqual([]);
    expect(on.brand_fix?.forks ?? {}).toEqual({}); // never a fork
  });

  it("editing a shipped rule forks it, and a fork edited again keeps its origin", () => {
    const o = editRule({}, "brand_fix", bayro.id, { ...bayro, set_brand: "BMW M" });
    expect(o.brand_fix?.forks?.[bayro.id]).toEqual({ rule: { ...bayro, set_brand: "BMW M" }, forked_from: "" });
    const saved: RulesOverlay = { brand_fix: { forks: { [bayro.id]: { rule: bayro, forked_from: "abc" } } } };
    const again = editRule(saved, "brand_fix", bayro.id, { ...bayro, set_brand: "Bayro" });
    expect(again.brand_fix?.forks?.[bayro.id].forked_from).toBe("abc");
    expect(restoreDefault(again, "brand_fix", bayro.id).brand_fix?.forks).toEqual({});
  });

  it("his own rules are added first, edited in place and deleted", () => {
    const o: RulesOverlay = { brand_fix: { own: [{ id: "own-1", name_contains: "lanzo", set_brand: "RSS" }] } };
    const added = addRule(o, "brand_fix", { id: "x", name_contains: "zeta", set_brand: "Z" });
    expect(added.brand_fix?.own?.map((r) => r.id)).toEqual([undefined, "own-1"]);
    const edited = editRule(o, "brand_fix", "own-1", { name_contains: "lanzo", set_brand: "RSS Cars" });
    expect(edited.brand_fix?.own).toEqual([{ id: "own-1", name_contains: "lanzo", set_brand: "RSS Cars" }]);
    expect(edited.brand_fix?.forks).toEqual({});
    const gone = removeRule(setEnabled(o, "brand_fix", "own-1", false), "brand_fix", "own-1");
    expect(gone.brand_fix?.own).toEqual([]);
    expect(gone.brand_fix?.disabled).toEqual([]);
  });

  it("never mutates the overlay it is given", () => {
    const o: RulesOverlay = {};
    setEnabled(o, "gearbox", "g", false);
    setCatalogOn(o, false);
    expect(o).toEqual({});
    expect(setCatalogOn(setCatalogOn(o, false), true)).toEqual({});
  });
});

describe("track categories", () => {
  it("a removed shipped category comes back rather than being added", () => {
    const off = setCategoryOn({}, "#rally", false);
    const back = addCategory(off, "Rally", ["#rally"]);
    expect(back.track_categories?.removed).toEqual([]);
    expect(back.track_categories?.added).toEqual([]);
    expect(addCategory({}, "#karting", ["#rally"]).track_categories?.added).toEqual(["#karting"]);
  });

  it("moving writes the whole effective order", () => {
    const o = moveCategory({}, ["#a", "#b", "#c"], 2, -1);
    expect(o.track_categories?.order).toEqual(["#a", "#c", "#b"]);
    expect(moveCategory({}, ["#a"], 0, -1)).toEqual({});
  });
});

describe("row editor", () => {
  it("an empty 'name contains' is refused: it would match every car", () => {
    expect(fromForm("brand_fix", { a: " ", b: "BMW", c: "" })).toBeNull();
    expect(fromForm("brand_fix", { a: "Bayro", b: "BMW", c: "" })).toEqual({
      name_contains: "bayro",
      set_brand: "BMW",
    });
  });

  it("a class rule needs a class or tags, and round-trips through the form", () => {
    expect(fromForm("class_fix", { a: "rally", b: "", c: "" })).toBeNull();
    const r = fromForm("class_fix", { a: "Rally, hillclimb", b: "race", c: "#rally" });
    expect(r).toEqual({ from: ["rally", "hillclimb"], set_class: "race", add: ["#rally"] });
    expect(toForm("class_fix", r!)).toEqual({ a: "rally, hillclimb", b: "race", c: "#rally" });
  });

  it("a spec value keeps its case", () => {
    expect(fromForm("drivetrain", { a: "rwd, propulsion", b: "RWD", c: "" })).toEqual({
      from: ["rwd", "propulsion"],
      set: "RWD",
    });
  });
});
