// Saved sessions, stored as Content Manager presets (SESSION§3.6).
//
// A named save is a full, recallable snapshot of the screen — distinct from
// the per-type presets ("the last settings used for this kind of session"),
// and mostly there so a carefully tuned grid is never lost. Since it became a
// `.cmpreset`, it is also a session Content Manager can start on its own, and
// the presets CM's user composed himself show up in the same list.
//
// Two things the screen must not have to know: where the file sits, and
// whether the entry came from us or from CM. Both live in `sessionpreset.rs`;
// here the entries all look alike, with an `origin` for the badge and a
// `notes` list for what a conversion could not carry.
import { invoke } from "@tauri-apps/api/core";
import { centerSpreadOf } from "./aiBand";
import { assistLevelFrom, type RaceSetup, type SessionType } from "./launch";

export type Season = "" | "spring" | "summer" | "autumn" | "winter";

export interface SavedSession {
  name: string;
  savedAt: string;
  setup: RaceSetup;
  opponentCount: number;
  /** Opponent pool (CIBLE§3.3), serialised like the library filters.
   * `undefined` on a save predating the tokens: loading then rebuilds the pool
   * from `gridMode`/`categorySelection`, kept for that one re-read and never
   * written back. */
  gridFilters?: string;
  gridPinned?: string[];
  gridMode?: "same_car" | "same_category" | "free";
  categorySelection?: string;
  season: Season;
  /** Weather intent chosen (so the right card lights up again on re-read). */
  intent: string;
  /** Track skins active when the session was saved (§8, several possible).
   * Car, driven skin, track and layout are already in `setup`; track skins are
   * not a session setting but a deployment state, hence this separate field.
   * `undefined` on a save predating the field: loading then leaves them alone
   * rather than taking an empty list for "no skin active" and undoing what the
   * user had set up. */
  trackSkins?: string[];
}

/** Where an entry comes from. `cm` is read-only: Pit Box lists a preset
 * Content Manager wrote, it never deletes or overwrites it. */
export type PresetOrigin = "pitbox" | "cm";

/** One entry of the list, as the backend hands it over. */
export interface SessionPreset {
  /** Absolute path — the identity of the entry. Two presets can share a name
   * in two of CM's subfolders, so the name is not a key. */
  path: string;
  name: string;
  origin: PresetOrigin;
  /** Path relative to `Quick Drive\`, to tell two same-named presets apart. */
  source: string;
  /** `null` when the file could not be converted — `reason` then says why. */
  session: SavedSession | null;
  /** i18n key for an entry that cannot be loaded. */
  reason: string | null;
  /** i18n keys for what a CM preset could not carry (the player's skin above
   * all). Shown in the yellow banner after loading. */
  notes: string[];
}

/** Every preset, ours and CM's. Never throws: a missing Content Manager or an
 * unreadable file is a non-result, not a failure. */
async function loadAll(): Promise<SessionPreset[]> {
  const list = await invoke<SessionPreset[]>("list_session_presets").catch((e) => {
    console.error("list_session_presets", e);
    return [] as SessionPreset[];
  });
  for (const entry of list) if (entry.session) migrate(entry.session);
  return list;
}

/** Fields of a snapshot whose shape changed since it was written.
 *
 * Runs on every entry rather than on the current type's: each save carries its
 * own `setup`, and converting only some would leave orphan booleans quietly
 * falling back to defaults on the next load. Idempotent — an entry already
 * converted carries its level, and the old boolean is no longer read.
 *
 * In place, on the way out of the backend: since a session is now a file of
 * its own, there is no "rewrite everything" pass left to make the conversion
 * durable. It is redone on each read, which costs nothing and never lies. */
function migrate(s: SavedSession): SavedSession {
  const old = s.setup as Partial<{
    abs_auto: boolean;
    traction_control_auto: boolean;
    ai_level_min: number;
    ai_level_max: number;
  }>;
  s.setup.abs = assistLevelFrom(s.setup.abs, old.abs_auto);
  s.setup.traction_control = assistLevelFrom(s.setup.traction_control, old.traction_control_auto);
  // Difficulty: two bounds before the centre ± spread model (SETUP§2.9).
  // Converted rather than reset to the default — a saved session often carries
  // a long-tuned setting, and watching it reset on reload is the worst of all.
  if (s.setup.ai_level == null && old.ai_level_min != null && old.ai_level_max != null) {
    const band = centerSpreadOf(old.ai_level_min, old.ai_level_max);
    s.setup.ai_level = band.center;
    s.setup.ai_spread = band.spread;
  }
  s.setup.aggression_spread ??= 0;
  return s;
}

/**
 * **Every** preset, the current type's first (SETUP§2.11).
 *
 * The type filter was dropped because it was **invisible**. Someone who had
 * saved a session in Race and looked for it from Practice did not see a
 * filtered list: they saw an empty one, and concluded the save had failed.
 * Loading a session switches the type — it is part of what is saved — so
 * loading one from another type is perfectly valid and there was nothing to
 * hide. The practical benefit of the filter survives as **sorting**.
 *
 * Entries that cannot be converted come last: they are named, as the grid
 * import names what it skipped, but they are not what the user came for.
 */
export async function listSavedSessions(sessionType: SessionType): Promise<SessionPreset[]> {
  const all = await loadAll();
  return all.sort((a, b) => {
    const rank = (e: SessionPreset) => (!e.session ? 2 : e.session.setup.session_type === sessionType ? 0 : 1);
    const ra = rank(a);
    const rb = rank(b);
    if (ra !== rb) return ra - rb;
    return (b.session?.savedAt ?? "").localeCompare(a.session?.savedAt ?? "");
  });
}

/** Saves (or overwrites, if the name is already taken) one session.
 *
 * No `invokeSafe` here: this is a **write**, and a mute fallback would make
 * "saved" out of a command that wrote nothing (golden rule 6). A failure is
 * logged and rethrown, never swallowed. */
export async function saveSession(session: SavedSession): Promise<string> {
  return invoke<string>("save_session_preset", {
    name: session.name,
    setup: session.setup,
    snapshot: session,
  }).catch((e) => {
    console.error("save_session_preset", e);
    throw e;
  });
}

/** Deletes one of our presets, by path. A preset Content Manager wrote is
 * refused by the backend — listed here, never removed from here. */
export async function deleteSavedSession(path: string): Promise<void> {
  return invoke<void>("delete_session_preset", { path }).catch((e) => {
    console.error("delete_session_preset", e);
    throw e;
  });
}

/** Save date, in the user's time zone.
 *
 * `savedAt` is a UTC ISO string (`new Date().toISOString()`), so slicing it by
 * hand (`iso.slice(0, 16)`) used to display UTC: a save made at 2 pm in France
 * showed "12:00". Storage stays UTC — that is what makes `localeCompare`
 * sorting correct — only the display goes back to local time. Format
 * deliberately fixed (short ISO) rather than `toLocaleString`: the same
 * monospace column for all six locales. */
export function formatSavedAt(iso: string): string {
  const d = new Date(iso);
  if (Number.isNaN(d.getTime())) return iso.slice(0, 16).replace("T", " ");
  const p = (n: number) => String(n).padStart(2, "0");
  return `${d.getFullYear()}-${p(d.getMonth() + 1)}-${p(d.getDate())} ${p(d.getHours())}:${p(d.getMinutes())}`;
}
