// Pure logic behind gamepad navigation profiles (§7.4): device identity,
// rest-relative binding matching, and calibration capture. No Svelte, no
// persistence, no DOM — everything here is a plain function over a `Gamepad`
// snapshot, which is what makes it the piece a frontend test runner would
// target the day one is introduced (see CLAUDE.md, "Chantiers en cours").
//
// Why "rest-relative" everywhere: a gamepad axis rests at 0, but nothing else
// does. A DirectInput hat normalized by Chromium rests *outside* [-1, 1]
// (~3.2 observed), pedals rest at -1, a wheel rests wherever it was left. Any
// code comparing an axis to a fixed threshold therefore reads "held" on a
// device that is simply sitting there — that is the whole bug this module
// exists to prevent (UI elements scrolling on their own, wheel plugged in).

export type Direction = "up" | "down" | "left" | "right";

export const DIRECTIONS: readonly Direction[] = ["up", "down", "left", "right"] as const;

/** A binding is a button, or a position of an axis. Hats, sticks and buttons
 *  all reduce to this — no family enum, because the difference only matters
 *  while capturing (see `axisMode`), never while reading. */
export type Binding =
  | { kind: "button"; index: number }
  | { kind: "axis"; hint: number; mode: "equals" | "beyond"; value: number };

/** Measured, never assumed: what the device reads when untouched. */
export interface RestSnapshot {
  axes: number[];
  buttons: boolean[];
}

/** Actions au-delà du déplacement du curseur (§7.4bis) — toutes **optionnelles** :
 * beaucoup de volants n'ont pas cinq boutons à leur consacrer, et un profil
 * sans elles reste parfaitement utilisable. Un raccourci absent ne fait rien,
 * il ne bloque rien. */
export type Action = "modPrev" | "modNext" | "tabPrev" | "tabNext" | "start" | "menu";

export const ACTIONS: readonly Action[] = ["modPrev", "modNext", "tabPrev", "tabNext", "start", "menu"] as const;

/** Axe analogique dédié au défilement — le stick droit d'une manette, une
 *  molette de volant. Ce n'est pas un `Binding` : une liaison est un
 *  interrupteur (« actif ou non »), alors que défiler demande la **valeur**,
 *  c'est ce qui distingue un défilement rapide d'un défilement fin. */
export interface ScrollAxis {
  /** Index de l'axe. Ici c'est bien l'index et non un point de départ : un axe
   *  continu n'a pas de valeur caractéristique à retrouver ailleurs. */
  index: number;
  /** Vrai quand pousser l'axe vers le haut donne une valeur *positive* : la
   *  convention interne est « positif = vers le bas », comme `deltaY`. */
  invert: boolean;
}

export interface NavProfile {
  dirs: Partial<Record<Direction, Binding>>;
  confirm?: Binding;
  back?: Binding;
  /** Raccourcis optionnels — voir `Action`. */
  actions?: Partial<Record<Action, Binding>>;
  /** Défilement analogique, optionnel lui aussi. */
  scroll?: ScrollAxis;
  rest: RestSnapshot;
}

/** En deçà, c'est de la dérive analogique, pas une intention. Plus haut que le
 *  seuil de capture : un stick relâché revient rarement exactement à zéro, et
 *  une liste qui glisse toute seule est pire que pas de défilement du tout. */
export const SCROLL_DEADZONE = 0.25;

/** Position de l'axe de défilement, entre -1 et 1, **positif vers le bas**, et
 *  0 dans la zone morte. Relatif au repos, comme tout le reste de ce module —
 *  un stick de volant ne repose pas forcément à zéro. */
export function scrollAmount(gp: Gamepad, scroll: ScrollAxis | undefined, rest: RestSnapshot): number {
  if (!scroll) return 0;
  const v = gp.axes[scroll.index];
  if (v === undefined) return 0;
  const delta = (v - (rest.axes[scroll.index] ?? 0)) * (scroll.invert ? -1 : 1);
  if (Math.abs(delta) < SCROLL_DEADZONE) return 0;
  // Renormalisé au-delà de la zone morte : sans ça, le premier cran utile
  // partirait déjà à 25 % de la vitesse maximale.
  const usable = (Math.abs(delta) - SCROLL_DEADZONE) / (1 - SCROLL_DEADZONE);
  return Math.sign(delta) * Math.min(1, usable);
}

export interface DeviceRecord {
  key: string;
  /** Raw `Gamepad.id`, kept so a device can be listed by a readable name even
   *  once unplugged (Settings lists known devices, connected or not). */
  label: string;
  /** Absence of a record entirely means "never asked" — `use: false` means
   *  the user answered "no", which is a different thing and never re-asked. */
  use: boolean;
  profile?: NavProfile;
  answeredAt: string;
}

/** An axis is "on" its recorded position within this much. Hat positions sit
 *  far apart (Fanatec V2.5: -1, -3/7, 1/7, 5/7 — 0.57 apart at the closest),
 *  so this is loose enough for driver jitter and tight enough to never
 *  confuse two directions. */
export const EQUALS_TOLERANCE = 0.1;

/** Threshold for a `beyond` binding (stick pushed toward an extreme), as a
 *  distance from rest. Deliberately high: a diagonal push on a stick reads
 *  ~0.7 on both axes, and 0.5 keeps the two directions from both firing. */
export const BEYOND_DEADZONE = 0.5;

/** An axis has "moved" (calibration capture, neutral checks) past this much
 *  from its rest. Below it, driver noise and analog drift. */
export const CAPTURE_THRESHOLD = 0.3;

/** Two samples of the same untouched device still differ slightly frame to
 *  frame; this is what "nothing changed" means when waiting for a device to
 *  settle before measuring its rest. */
export const REST_JITTER = 0.05;

/** An axis reaching this far is at a mechanical extreme — a stick pushed all
 *  the way, not a hat sitting on an intermediate discrete value. */
export const EXTREME = 0.9;

/** `"0eb7:0e04"` when VID/PID are present, otherwise the normalized raw id.
 *
 *  Never `Gamepad.index`: that is a slot, reassigned as devices come and go,
 *  so it identifies nothing across a reconnect. Two identical XInput pads
 *  share one id and therefore one key — indistinguishable, and harmlessly so:
 *  the decision is about the model. Do not try to separate them. */
export function deviceKey(id: string): string {
  const m = /Vendor:\s*([0-9a-f]{4})\s+Product:\s*([0-9a-f]{4})/i.exec(id);
  return m ? `${m[1]}:${m[2]}`.toLowerCase() : id.trim().toLowerCase();
}

/** The vendor/model part of an id, with the `(Vendor: … Product: …)` suffix
 *  stripped. A wheel usually shows up as several `Gamepad` entries (base plus
 *  a button box, sometimes under different PIDs) that share this prefix —
 *  adopting one marks its siblings answered, so a full rig is settled in one
 *  gesture instead of five identical questions. */
export function deviceFamily(id: string): string {
  return id
    .replace(/\([^)]*\)/g, " ")
    .replace(/\s+/g, " ")
    .trim()
    .toLowerCase();
}

export function isSameFamily(a: string, b: string): boolean {
  const fa = deviceFamily(a);
  const fb = deviceFamily(b);
  return fa.length > 0 && fa === fb;
}

export function measureRest(gp: Gamepad): RestSnapshot {
  return { axes: Array.from(gp.axes), buttons: gp.buttons.map((b) => b.pressed) };
}

export const EMPTY_REST: RestSnapshot = { axes: [], buttons: [] };

export function hasRest(rest: RestSnapshot | undefined | null): boolean {
  return !!rest && rest.axes.length > 0;
}

/** True when two snapshots describe the same physical state — used to wait
 *  for a device to hold still before taking its rest as reference. */
export function sampleEqual(a: RestSnapshot, b: RestSnapshot): boolean {
  if (a.axes.length !== b.axes.length || a.buttons.length !== b.buttons.length) return false;
  for (let i = 0; i < a.axes.length; i++) if (Math.abs(a.axes[i] - b.axes[i]) > REST_JITTER) return false;
  for (let i = 0; i < a.buttons.length; i++) if (a.buttons[i] !== b.buttons[i]) return false;
  return true;
}

function equalsMatch(gp: Gamepad, target: number, hint: number, rest: RestSnapshot): boolean {
  // Axis indices are not stable across driver/enumeration changes, so `hint`
  // is a starting point and the real recognition is by value across every
  // axis (same auto-location the shipped Fanatec override already relied on).
  for (let n = -1; n < gp.axes.length; n++) {
    const i = n < 0 ? hint : n;
    const v = gp.axes[i];
    if (v === undefined) continue;
    if (Math.abs(v - target) > EQUALS_TOLERANCE) continue;
    // An axis that *rests* on the target value is not a press. Without this,
    // a pedal (rest -1) answers for a hat whose "up" is also -1, and the
    // focus walks off on its own the moment the wheel is switched on.
    const r = rest.axes[i];
    if (r !== undefined && Math.abs(r - target) <= EQUALS_TOLERANCE) continue;
    return true;
  }
  return false;
}

export function bindingActive(gp: Gamepad, b: Binding, rest: RestSnapshot): boolean {
  if (b.kind === "button") return gp.buttons[b.index]?.pressed ?? false;
  if (b.mode === "equals") return equalsMatch(gp, b.value, b.hint, rest);
  // `beyond` is a threshold on a continuous axis: it has no characteristic
  // value to search for, so `hint` is the index, not a suggestion.
  const v = gp.axes[b.hint];
  if (v === undefined) return false;
  const delta = v - (rest.axes[b.hint] ?? 0);
  return b.value > 0 ? delta > BEYOND_DEADZONE : delta < -BEYOND_DEADZONE;
}

export function profileBindings(profile: NavProfile): Binding[] {
  const out: Binding[] = [];
  for (const d of DIRECTIONS) {
    const b = profile.dirs[d];
    if (b) out.push(b);
  }
  if (profile.confirm) out.push(profile.confirm);
  if (profile.back) out.push(profile.back);
  // Les raccourcis comptent dans l'armement au même titre que le reste : un
  // bouton d'action maintenu au branchement doit retarder le premier
  // événement, sinon on retombe sur le bug que `anyBindingActive` évite.
  for (const a of ACTIONS) {
    const b = profile.actions?.[a];
    if (b) out.push(b);
  }
  return out;
}

/** True as long as anything the profile listens to is held. The gate before
 *  a device is allowed to drive the UI: a pedal pressed at plug-in time must
 *  not produce a permanent "down" from the very first frame. */
export function anyBindingActive(gp: Gamepad, profile: NavProfile, rest: RestSnapshot): boolean {
  return profileBindings(profile).some((b) => bindingActive(gp, b, rest));
}

export function bindingsEqual(a: Binding, b: Binding): boolean {
  if (a.kind === "button" && b.kind === "button") return a.index === b.index;
  if (a.kind === "axis" && b.kind === "axis") {
    return a.hint === b.hint && Math.abs(a.value - b.value) <= EQUALS_TOLERANCE;
  }
  return false;
}

export interface Change {
  binding: Binding;
  /** How far this input moved from its rest — the tie-breaker when several
   *  things move at once (a wheel rocking while a button is pressed). */
  magnitude: number;
}

/** The most marked change against `rest`: a button that went from released to
 *  pressed, or the axis furthest from where it rests. Returns axis bindings in
 *  `equals` mode; `axisMode` decides afterwards whether it was really a stick. */
export function strongestChange(gp: Gamepad, rest: RestSnapshot): Change | null {
  let best: Change | null = null;
  for (let i = 0; i < gp.buttons.length; i++) {
    if (!gp.buttons[i].pressed || rest.buttons[i]) continue;
    // A button is binary: nothing beats it, and nothing ties with it either —
    // first pressed button wins over any axis excursion.
    return { binding: { kind: "button", index: i }, magnitude: 1 };
  }
  for (let i = 0; i < gp.axes.length; i++) {
    const v = gp.axes[i];
    const delta = Math.abs(v - (rest.axes[i] ?? 0));
    if (delta < CAPTURE_THRESHOLD) continue;
    if (!best || delta > best.magnitude) {
      best = { binding: { kind: "axis", hint: i, mode: "equals", value: v }, magnitude: delta };
    }
  }
  return best;
}

/** Hat or stick? Both are an axis; the difference is only visible *during* the
 *  gesture — a stick crosses intermediate values on its way, a hat jumps from
 *  one discrete value to another. This is not cosmetic: a threshold applied to
 *  a hat whose "up" reads -0.71 never fires at all. */
export function axisMode(sawIntermediate: boolean, finalValue: number): "equals" | "beyond" {
  return sawIntermediate && Math.abs(finalValue) >= EXTREME ? "beyond" : "equals";
}

/** True while an axis sits between its rest and an extreme — the sampling that
 *  tells a stick from a hat over the frames of one capture. */
export function isIntermediate(value: number, restValue: number): boolean {
  const delta = Math.abs(value - restValue);
  return delta > REST_JITTER && Math.abs(value) < EXTREME;
}

/** Human-readable binding, for the calibration recap and the technical panel.
 *  Deliberately terse and untranslated: these are index/value diagnostics, not
 *  user-facing advice. */
export function describeBinding(b: Binding | undefined): string {
  if (!b) return "—";
  if (b.kind === "button") return `button ${b.index}`;
  return `axis ${b.hint} ${b.mode === "equals" ? "=" : "→"} ${b.value.toFixed(2)}`;
}

/** Payload of the "share this profile" button: the model, its shape, and the
 *  bindings. Nothing identifying — no user name, no path, no machine id. */
export function profileReport(
  device: { id: string; mapping: string; axisCount: number; buttonCount: number },
  profile: NavProfile,
  appVersion: string,
): string {
  return JSON.stringify(
    {
      id: device.id,
      key: deviceKey(device.id),
      mapping: device.mapping,
      axes: device.axisCount,
      buttons: device.buttonCount,
      dirs: profile.dirs,
      confirm: profile.confirm,
      back: profile.back,
      actions: profile.actions,
      scroll: profile.scroll,
      rest: profile.rest,
      pitbox: appVersion,
    },
    null,
    2,
  );
}
