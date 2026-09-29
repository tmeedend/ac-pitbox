import { describe, expect, it, vi } from "vitest";

// The wording is i18n's; what is tested here is which values reach the page,
// in which order, and with which provenance. The stub returns the key with its
// parameters, so a sentence stays readable in an assertion.
vi.mock("$lib/i18n/index.svelte", () => ({
  t: (key: string, params?: Record<string, string | number>) =>
    params ? `${key}(${Object.values(params).join(",")})` : key,
}));
vi.mock("@tauri-apps/api/core", () => ({ invoke: vi.fn() }));

const { aidChips, draftOf, isDerived, isEdited, keyFigures, mechanicsRows, nextAidState, parseInput, reportLines } =
  await import("./techSheet");
import type { TechSheet } from "./techSheet";

function sheet(values: TechSheet["values"]): TechSheet {
  return { values, edited: [], fallback: {} };
}

const deps = {
  locale: "fr",
  countryLabel: (n: string) => `label:${n}`,
  flagFor: (n: string) => `flag:${n}`,
};

describe("key figures", () => {
  it("formats numbers in the user's language and keeps the author's plus", () => {
    const f = keyFigures(
      sheet({
        weight: { value: { n: 1245, unit: "kg" }, source: "ui" },
        topspeed: { value: { n: 270, unit: "km/h", plus: true }, source: "ui" },
        power: { value: "(544+120)Bhp", source: "ui" },
      }),
      "fr",
    );
    // Strip order, whatever the order of the values.
    expect(f.map((x) => x.field)).toEqual(["power", "weight", "topspeed"]);
    expect(f[0].text).toBe("(544+120)Bhp");
    expect(f[1].num).toBe((1245).toLocaleString("fr"));
    expect(f[2]).toMatchObject({ num: "270+", unit: "km/h" });
  });
});

describe("mechanics rows", () => {
  it("runs the values of a row into one sentence, each part with its source", () => {
    const rows = mechanicsRows(
      sheet({
        drivetrain: { value: "RWD", source: "physics" },
        gearbox: { value: "SEQUENTIAL", source: "rules" },
        gears: { value: 6, source: "physics" },
        country: { value: "Italy", source: "ui" },
        year: { value: 2011, source: "ui" },
      }),
      deps,
    );
    expect(rows.map((r) => r.key)).toEqual(["transmission", "origin"]);
    expect(rows[0].parts.map((p) => [p.text, p.source])).toEqual([
      // The stub has no translation: a code without a label shows as itself.
      ["RWD", "physics"],
      ["SEQUENTIAL", "rules"],
      ["techsheet.gears(6)", "physics"],
    ]);
    expect(rows[1].parts[0]).toMatchObject({ text: "label:Italy", flag: "flag:Italy" });
    expect(rows[1].parts[1].text).toBe("2011");
  });

  it("leaves out a row with nothing to say (R4)", () => {
    expect(mechanicsRows(sheet({}), deps)).toEqual([]);
  });
});

describe("electronics", () => {
  it("shows an absent ABS but never an absent DRS, and hides the unknown", () => {
    const chips = aidChips(
      sheet({
        "aid.abs": { value: false, source: "physics" },
        "aid.drs": { value: false, source: "user" },
        "aid.ers": { value: true, source: "physics" },
        "aid.ers_heat": { value: true, source: "physics" },
      }),
    );
    expect(chips.map((c) => [c.field, c.present])).toEqual([
      ["aid.abs", false],
      ["aid.ers", true],
    ]);
    expect(chips[1].sub).toBe("MGU-K + MGU-H");
  });

  it("cycles present, absent, unknown", () => {
    expect(nextAidState(true)).toBe(false);
    expect(nextAidState(false)).toBe(null);
    expect(nextAidState(null)).toBe(true);
  });
});

describe("provenance", () => {
  it("puts a sign only on deduced and corrected values", () => {
    for (const s of ["physics", "ui", "table", "computed"] as const) {
      expect(isDerived(s) || isEdited(s), `${s} carries no sign`).toBe(false);
    }
    expect(isDerived("rules"), "tags → ≈").toBe(true);
    expect(isEdited("rules")).toBe(false);
    expect(isEdited("user"), "user → ✎").toBe(true);
    expect(isDerived("user")).toBe(false);
  });
});

describe("edit mode inputs", () => {
  it("starts from the number, never from the author's text", () => {
    expect(draftOf("power", { value: { n: 470, unit: "bhp" }, source: "ui" })).toBe("470");
    expect(draftOf("power", { value: "(544+120)Bhp", source: "ui" })).toBe("");
    expect(draftOf("engine_config", { value: "V8", source: "rules" })).toBe("V8");
    expect(draftOf("gears", undefined)).toBe("");
  });

  it("reads a typed number, a decimal comma included", () => {
    expect(parseInput("2,65")).toBe(2.65);
    expect(parseInput("  ")).toBe(null);
    expect(parseInput("abc")).toBe(undefined);
    expect(parseInput("-3")).toBe(undefined);
  });
});

describe("reading report", () => {
  it("lists the columns that moved, in column order, zero counts left out", () => {
    const lines = reportLines({
      cars: 397,
      fields: {
        gearbox: { gained: 61, changed: 1, lost: 0 },
        aspiration: { gained: 287, changed: 105, lost: 0 },
      },
    });
    expect(lines).toEqual([
      { field: "aspiration", counts: [["gained", 287], ["changed", 105]] },
      { field: "gearbox", counts: [["gained", 61], ["changed", 1]] },
    ]);
  });
});
