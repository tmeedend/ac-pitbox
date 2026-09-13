// Power, weight and the kg/bhp ratio, read off `ui_car.json` (§3.4).
//
// **The reading is deliberately conservative, and that is the whole point of
// this file.** The `specs` of a `ui_car.json` are free text typed by the mod
// author, and a wrong ratio is worse than no ratio: it silently puts a car in
// a performance band it does not belong to, and nothing on screen says so.
// So a value is either read with certainty or declared unreadable — never
// estimated, never converted by inference.
//
// Measured on a real library of 311 cars (2026-09-13), which is what settled
// the rules below:
//
//   power   266 `bhp` · 19 `hp` · 10 `495+bhp` · 7 `ps` · 6 `whp` · 2 `л.с.` · 1 `--bhp`
//   weight  286 `kg`  · 18 `1525 kg*` · 5 `--kg` · 1 bare number · 1 `кг`
//
// 291 of the 311 yield a ratio; the 20 others are rejected for three distinct
// reasons, none of which a conversion factor could repair honestly:
//
//   - **`whp` is not `bhp`** (6 mods). Wheel horsepower is measured after the
//     drivetrain; the gap depends on the car, not on a constant. Converting it
//     would be inventing a number.
//   - **`ps` / `л.с.` are metric horsepower** (9 mods). The factor is exact
//     (0.9863) but applying it means deciding that the author meant the metric
//     unit, and `ps` typed by a modder is as often a synonym for `bhp`. Out.
//   - **`--` is the author's own "unknown"** (5 mods, e.g. `ks_ferrari_sf70h`).
//     Honouring it is the easy part.
//
// `hp` IS accepted as `bhp`, with no conversion: in English-language AC mods
// the two are the same word, and imperial hp and bhp are numerically identical.
// The residual risk is an author writing `hp` for the metric unit — 1.4 % off,
// far below the ±5 % of the narrowest band this feature offers.
//
// No Svelte, no DOM: plain functions, so the filter, the grid and the tests
// share them.

/** What a car's spec sheet yields, or does not. `null` means unreadable —
 * never zero, never a guess. */
export interface CarPerf {
  /** Brake horsepower. */
  bhp: number | null;
  /** Kilograms. */
  kg: number | null;
  /** kg/bhp. `null` as soon as either side is unreadable. */
  ratio: number | null;
}

/** Units accepted for power, all meaning brake horsepower exactly. */
const POWER_UNITS = ["bhp", "hp"];
/** Units accepted for weight. A bare number is NOT accepted: the field is free
 * text, and "1195.7" could be pounds as easily as kilograms. One car in 311. */
const WEIGHT_UNITS = ["kg"];

/**
 * Number and unit of one spec field.
 *
 * Tolerated around the number: leading/trailing spaces, a decimal comma, a
 * space or nothing before the unit, and the two decorations authors add to
 * say "about" — the `+` of `495+bhp` and the `*` of `1525 kg*`, which both
 * footnote the value without changing what it is.
 */
function parse(raw: string | null | undefined, units: string[]): number | null {
  if (!raw) return null;
  const m = /^\s*(\d+(?:[.,]\d+)?)\s*(.*)$/.exec(raw);
  if (!m) return null;
  const n = Number(m[1].replace(",", "."));
  if (!Number.isFinite(n) || n <= 0) return null;
  // The decorations are stripped wherever the author put them — `495+bhp` has
  // its `+` before the unit, `1525 kg*` its `*` after.
  const unit = m[2].toLowerCase().replace(/[+*.\s]/g, "");
  // An empty unit is rejected, not assumed: see WEIGHT_UNITS.
  return units.includes(unit) ? n : null;
}

export const readPower = (raw: string | null | undefined): number | null => parse(raw, POWER_UNITS);
export const readWeight = (raw: string | null | undefined): number | null => parse(raw, WEIGHT_UNITS);

/** The pair plus their ratio. Both sides must be readable for the ratio to
 * exist — half a spec sheet is not half a ratio. */
export function carPerf(bhp: string | null, weight: string | null): CarPerf {
  const p = readPower(bhp);
  const kg = readWeight(weight);
  return { bhp: p, kg, ratio: p != null && kg != null ? kg / p : null };
}

/** Shown in the grid and in the filter editor. Two decimals: on a real library
 * the ratios run from about 1.0 to 30, and the third decimal separates nothing
 * a driver can feel. */
export function formatRatio(ratio: number | null): string {
  return ratio == null ? "—" : ratio.toFixed(2);
}

/** The band a tolerance draws around a reference ratio, in kg/bhp.
 *
 * **The band is wider on the slow side than on the fast one**, and that is not
 * a bug: a LOWER kg/bhp is a faster car, so "±15 % of my performance" has to be
 * ±15 % of the reference, which the two bounds express directly. */
export function perfBand(ref: number, pct: number): { min: number; max: number } {
  const d = ref * (pct / 100);
  return { min: ref - d, max: ref + d };
}

/** Tolerances offered by the editor: ±5 % to ±50 %, step 5 (§3.4). */
export const PERF_STEP = 5;
export const PERF_MIN_PCT = 5;
export const PERF_MAX_PCT = 50;
export const PERF_DEFAULT_PCT = 15;

export function clampPerfPct(pct: number): number {
  // The two are NOT the same thing, and folding them into a `|| default` is
  // how a typed `1 %` came back as `15 %`: an ABSENT value falls back on the
  // default, a value that merely rounds down to zero falls back on the floor.
  if (!Number.isFinite(pct) || pct <= 0) return PERF_DEFAULT_PCT;
  const rounded = Math.round(pct / PERF_STEP) * PERF_STEP;
  return Math.max(PERF_MIN_PCT, Math.min(PERF_MAX_PCT, rounded));
}
