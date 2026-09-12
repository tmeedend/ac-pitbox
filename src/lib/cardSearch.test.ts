import { describe, expect, it } from "vitest";
import { matchesQuery } from "./cardSearch";
import type { ModCard } from "./library";

/** Only the fields the search reads; the rest of a `ModCard` plays no part. */
function card(over: Partial<ModCard>): ModCard {
  return {
    id_interne: "mod_id",
    display_name: "Some Car",
    brand: null,
    category: null,
    source_pack: null,
    notes_user: null,
    tags_from_mod: [],
    tags_from_rule: [],
    tags_manual: [],
    ...over,
  } as ModCard;
}

describe("matchesQuery", () => {
  /** Real bug: the terms were matched as one glued substring, so a name with a
   * word inserted in the middle no longer came up. */
  it("matches words separately rather than as one glued substring", () => {
    const c = card({ display_name: "RSS GT-M Adonis Evo" });
    expect(matchesQuery(c, "GT-M Evo")).toBe(true);
    expect(matchesQuery(c, "Evo GT-M")).toBe(true);
    expect(matchesQuery(c, "GT-M Zonda")).toBe(false);
  });

  it("requires every term, not just one", () => {
    const c = card({ display_name: "Ferrari 488 GT3" });
    expect(matchesQuery(c, "ferrari gt3")).toBe(true);
    expect(matchesQuery(c, "ferrari gt4")).toBe(false);
  });

  /** §4.4 — searching a pack's name must bring up all of its cars, whose own
   * names say nothing about it. */
  it("searches the pack name and the user's own note", () => {
    const c = card({ display_name: "Adonis", source_pack: "RSS Formula Hybrid", notes_user: "à régler, sous-virage" });
    expect(matchesQuery(c, "formula hybrid")).toBe(true);
    expect(matchesQuery(c, "sous-virage")).toBe(true);
  });

  it("searches tags whatever their origin", () => {
    const c = card({ display_name: "Z4", tags_from_mod: ["#gt3"], tags_manual: ["favoris"] });
    expect(matchesQuery(c, "#gt3")).toBe(true);
    expect(matchesQuery(c, "favoris")).toBe(true);
  });

  it("is case-insensitive and matches an empty query against everything", () => {
    const c = card({ display_name: "Mazda MX-5" });
    expect(matchesQuery(c, "mAzDa")).toBe(true);
    expect(matchesQuery(c, "")).toBe(true);
    expect(matchesQuery(c, "   ")).toBe(true);
  });
});
