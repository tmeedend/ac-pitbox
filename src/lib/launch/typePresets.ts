// Presets de session par type (SESSION§3): what the session screen remembers
// for each session type — practice, hotlap, race, trackday — and how a preset
// written by an older version is read back.
//
// Pure, and tested: the read-back is a stack of migrations (bounds turned into
// centre ± spread, a floor raised from 60 to 70, booleans turned into three
// assist levels, grips recalibrated on the game's list), each of which once
// reset a setting without a word when it was missing.
import { centerSpreadOf } from "$lib/launch/aiBand";
import type {
  AssistLevel,
  PracticeStart,
  RaceSetup,
  Season,
  StartMode,
  TrackStateRef,
} from "$lib/launch/launch";
import { clampAiLevel } from "$lib/launch/opponent";
import type { SavedPool } from "$lib/launch/gridRules";
import { START_MODES, assistLevelFrom, nearestGrip } from "$lib/launch/setupRules";

/** A per-type preset as `launch_state.json` stores it. The pool fields
 * (`grid_filters`, and the tab-era ones still read back) come from `SavedPool`,
 * next to the migration that reads them. */
export interface TypePreset extends SavedPool {
  /** Centre et écart (SETUP§2.9). Un preset d'avant porte encore `ai_level_min`
   * et `ai_level_max` : `readPreset` les convertit, il ne les jette pas. */
  ai_level?: number; ai_spread?: number; aggression_spread?: number;
  ai_level_min?: number; ai_level_max?: number;
  opponent_count: number;
  /** Absents sur un preset antérieur au §4.4 : les défauts de Content
   * Manager, dernier sur la grille et agressivité nulle. */
  aggression?: number; start_mode?: StartMode; ghost_advantage?: number;
  laps: number; time_hours: number;
  penalties: boolean; jump_start_penalty: number;
  /** L'état de piste entier (L4§4.7). `grip` reste écrit pour qu'un retour en
   * arrière de version retrouve quelque chose, et relu quand `track_state`
   * manque. */
  track_state?: TrackStateRef | null; grip: number;
  practice_enabled: boolean; practice_minutes: number;
  qualify_enabled: boolean; qualify_minutes: number; ghost_car: boolean; practice_start: PracticeStart;
  damage: number; fuel_rate: number; tyre_wear: number; tyre_blankets: boolean; intent: string; season: Season;
  /** Trois états depuis SESSION§3 ; `abs_auto`/`traction_control_auto` sont les
   * booléens d'avant, relus une dernière fois par `assistLevelFrom`. */
  abs?: AssistLevel; traction_control?: AssistLevel;
  abs_auto?: boolean; traction_control_auto?: boolean;
  ideal_line: boolean;
}

/** What a preset carries of the opponents grid, as the grid serializes it. */
export interface PresetGrid {
  count: number;
  pool: string;
  pinned: string[];
}

/** The fields of `RaceSetup` a preset sets back. */
export type PresetSetup = Pick<
  RaceSetup,
  | "ai_level" | "ai_spread" | "aggression" | "aggression_spread" | "start_mode" | "ghost_advantage"
  | "laps" | "time_hours" | "penalties" | "jump_start_penalty" | "track_state" | "grip"
  | "practice_enabled" | "practice_minutes" | "qualify_enabled" | "qualify_minutes" | "ghost_car" | "practice_start"
  | "damage" | "fuel_rate" | "tyre_wear" | "tyre_blankets" | "ideal_line"
>;

/** A preset read back, migrations applied. The assists are apart from `setup`:
 * the screen sets them through their store, which is their live value. */
export interface PresetValues {
  setup: PresetSetup;
  opponentCount: number;
  abs: AssistLevel;
  tractionControl: AssistLevel;
  season: Season;
  intent: string;
}

/** The preset of the current settings, as it is written. */
export function presetFromSetup(setup: RaceSetup, grid: PresetGrid, intent: string, season: Season): TypePreset {
  return {
    ai_level: setup.ai_level, ai_spread: setup.ai_spread, aggression_spread: setup.aggression_spread,
    aggression: setup.aggression, start_mode: setup.start_mode, ghost_advantage: setup.ghost_advantage,
    opponent_count: grid.count,
    grid_filters: grid.pool, grid_pinned: [...grid.pinned],
    laps: setup.laps, time_hours: setup.time_hours,
    penalties: setup.penalties, jump_start_penalty: setup.jump_start_penalty,
    track_state: setup.track_state ? { ...setup.track_state } : null, grip: setup.grip,
    practice_enabled: setup.practice_enabled, practice_minutes: setup.practice_minutes,
    qualify_enabled: setup.qualify_enabled, qualify_minutes: setup.qualify_minutes, ghost_car: setup.ghost_car,
    practice_start: setup.practice_start,
    damage: setup.damage, fuel_rate: setup.fuel_rate, tyre_wear: setup.tyre_wear, tyre_blankets: setup.tyre_blankets,
    intent, season,
    abs: setup.abs, traction_control: setup.traction_control, ideal_line: setup.ideal_line,
  };
}

/** Reads a preset back, whatever version wrote it. */
export function readPreset(p: TypePreset): PresetValues {
  // Recalés : un preset enregistré quand le plancher était 60 porte des
  // valeurs que Content Manager n'accepte pas, et les envoyer telles quelles
  // ferait courir une session que l'écran n'annonce pas.
  //
  // Et converti : un preset d'avant le modèle centre ± écart porte deux
  // bornes. Les convertir plutôt que retomber sur le défaut, sinon une
  // difficulté réglée depuis des mois se réinitialise sans un mot.
  let ai_level: number;
  let ai_spread: number;
  if (p.ai_level != null) {
    ai_level = clampAiLevel(p.ai_level);
    ai_spread = Math.max(0, p.ai_spread ?? 0);
  } else {
    const band = centerSpreadOf(clampAiLevel(p.ai_level_min ?? 92), clampAiLevel(p.ai_level_max ?? 98));
    ai_level = band.center;
    ai_spread = band.spread;
  }
  return {
    setup: {
      ai_level,
      ai_spread,
      aggression_spread: Math.max(0, Math.min(100, p.aggression_spread ?? 0)),
      aggression: Math.max(0, Math.min(100, p.aggression ?? 0)),
      // Les quatre valeurs courantes se relisent telles quelles ; seul le
      // `"custom"` d'avant les segments retombe sur le défaut. La liste était
      // écrite à l'envers — elle n'acceptait que `first` et `last`, donc un
      // preset portant `second` ou `random` revenait sur `random` en silence.
      start_mode: START_MODES.includes(p.start_mode as StartMode) ? (p.start_mode as StartMode) : "random",
      ghost_advantage: Math.max(0, Math.min(5, p.ghost_advantage ?? 0)),
      laps: p.laps,
      time_hours: p.time_hours,
      penalties: p.penalties,
      jump_start_penalty: p.jump_start_penalty ?? 0,
      // L'état entier s'il est là, le pourcentage seul sinon : `TrackConditionBlock`
      // retrouve alors l'état natif le plus proche, ce que faisait l'ancien select.
      track_state: p.track_state ?? null,
      grip: nearestGrip(p.grip ?? 100),
      practice_enabled: p.practice_enabled ?? false,
      practice_minutes: p.practice_minutes ?? 20,
      qualify_enabled: p.qualify_enabled ?? true,
      qualify_minutes: p.qualify_minutes ?? 10,
      ghost_car: p.ghost_car ?? false,
      practice_start: p.practice_start ?? "pit",
      damage: p.damage ?? 50,
      fuel_rate: p.fuel_rate ?? 100,
      tyre_wear: p.tyre_wear ?? 100,
      tyre_blankets: p.tyre_blankets ?? false,
      ideal_line: p.ideal_line ?? false,
    },
    opponentCount: p.opponent_count ?? 7,
    abs: assistLevelFrom(p.abs, p.abs_auto),
    tractionControl: assistLevelFrom(p.traction_control, p.traction_control_auto),
    season: p.season ?? "",
    intent: p.intent,
  };
}
