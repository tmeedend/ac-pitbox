// The values a saved session setting is recalibrated against when it is read
// back: the track grips the game offers, the start positions, the assist
// levels; and the session a screen starts from before anything is read back.
//
// Its own module for the same mechanical reason as `opponent.ts`: `launch.ts`
// imports runes and cannot be loaded by Vitest, and `typePresets.ts`, which is
// tested, needs these. `launch.ts` re-exports them, so its callers do not
// change.
import type { AssistLevel, RaceSetup, StartMode } from "$lib/launch/launch";

/** The session before any choice, preset or saved selection is applied: the
 * session screen's, and a story event's before its own settings. */
export function defaultSetup(): RaceSetup {
  return {
    car_id: "",
    car_skin: null,
    driver: null,
    track_id: "",
    track_layout: null,
    session_type: "practice",
    opponents: [],
    ai_level: 95,
    ai_spread: 3,
    aggression: 0,
    aggression_spread: 0,
    player_ballast: 0,
    player_restrictor: 0,
    start_mode: "random",
    laps: 5,
    weather: "",
    time_hours: 13,
    ambient_c: null,
    road_c: null,
    wind_speed_kmh: null,
    wind_direction_deg: null,
    season: null,
    season_date: null,
    penalties: false,
    jump_start_penalty: 0,
    track_state: null,
    grip: 96,
    practice_enabled: false,
    practice_minutes: 20,
    qualify_enabled: true,
    qualify_minutes: 10,
    ghost_car: false,
    ghost_advantage: 0,
    practice_start: "pit",
    damage: 50,
    fuel_rate: 100,
    tyre_wear: 100,
    tyre_blankets: false,
    abs: "factory",
    traction_control: "factory",
    ideal_line: false,
  };
}

/** Les six états de piste du jeu, par grip de départ croissant — la liste que
 * Content Manager propose, et qui vient du même fichier (`tracks.ini`). Le
 * pourcentage de départ **est** l'identifiant de l'état : c'est le seul des
 * quatre paramètres que l'écran retient, `quickdrive.rs` retrouve les trois
 * autres à partir de lui. */
export const TRACK_GRIPS = [86, 89, 95, 96, 98, 100] as const;

/** « Auto » : l'état de piste est laissé à la météo — le `WeatherDefined` de
 * Content Manager, et la première entrée de sa liste. Sentinelle plutôt qu'un
 * second champ : l'écran n'offre qu'un choix parmi sept, et deux champs pour
 * une seule décision finissent toujours par se contredire. Aucun état réel ne
 * vaut 0 %. */
export const GRIP_WEATHER = 0;

/** Recale un grip enregistré sur la liste offerte.
 *
 * Un preset antérieur à l'alignement sur la table du jeu peut porter une
 * valeur qui n'y figure plus (92 % a existé, inventé) : sans ce recalage, le
 * segmenté n'en marquerait aucun comme actif. À égale distance le plus
 * adhérent gagne, comme côté Rust — une piste un peu plus roulante est le
 * repli indulgent. */
export function nearestGrip(grip: number): number {
  // « Auto » n'est pas un grip : le recalage ne doit pas le prendre pour une
  // valeur basse et le remplacer par la piste la plus glissante.
  if (grip === GRIP_WEATHER) return grip;
  let best: number = TRACK_GRIPS[0];
  for (const g of TRACK_GRIPS) {
    const d = Math.abs(g - grip);
    const bd = Math.abs(best - grip);
    if (d < bd || (d === bd && g > best)) best = g;
  }
  return best;
}

/** Les quatre valeurs, pour relire un preset sans en oublier une. Une liste
 * écrite à la main dans la relecture avait laissé tomber `second` et `random`,
 * qui revenaient donc au défaut en silence. */
export const START_MODES: StartMode[] = ["random", "first", "second", "last"];

/** Reprise d'un réglage d'aide enregistré avant les trois états (SESSION§3).
 *
 * L'ancien champ était un booléen nommé `abs_auto`, et « auto » voulait dire
 * `Abs: 1` dans le preset Quick Drive, c'est-à-dire **exactement** le niveau
 * qui s'appelle aujourd'hui `factory`. Donc `true → factory`, pas `true → on` :
 * `on` vaut `2` et forcerait une aide là où l'utilisateur n'avait demandé que
 * l'équipement réel de la voiture. Une migration ne change pas ce qui part en
 * jeu — c'est la même raison qui fait que `false → off` plutôt que le nouveau
 * défaut, l'un et l'autre conservant à la lettre la valeur déjà envoyée.
 *
 * Absent (`undefined`, distinct de `false`) : sauvegarde plus ancienne encore,
 * ou preset neuf — `factory`, le défaut. */
export function assistLevelFrom(level: AssistLevel | undefined, legacy: boolean | undefined): AssistLevel {
  if (level) return level;
  if (legacy === undefined) return "factory";
  return legacy ? "factory" : "off";
}
