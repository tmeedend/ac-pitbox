// The list behind the "Save session…" / "Load session…" dialog (SESSION§3.5):
// a full snapshot of the settings (opponents, weather, options…), recalled by
// name — distinct from the automatic per-type presets.
//
// Loading an entry is not here: it rewrites the screen's own state (setup,
// season, weather, grid), so it stays with the screen. What lives here is the
// list itself and the gestures that only touch the list.
import { t } from "$lib/i18n/index.svelte";
import { errorText } from "$lib/errors";
import type { ModCard } from "$lib/library/library";
import type { SessionType } from "./launch";
import {
  deleteSavedSession,
  formatSavedAt,
  listSavedSessions,
  saveSession,
  type SavedSession,
  type SessionPreset,
} from "./savedSessions";

export class SavedSessionList {
  dialog = $state<"save" | "load" | null>(null);
  entries = $state<SessionPreset[]>([]);
  /** The button's count only counts what loads: a preset whose mode has no
   * Pit Box equivalent is listed so one knows it exists, but announcing it in
   * "Load (12)" would promise twelve sessions. */
  loadableCount = $derived(this.entries.filter((e) => e.session).length);

  #latest = 0;

  /** The type no longer filters, it SORTS (SETUP§2.11): the list carries them
   * all, the current type's first. The type can change before the answer
   * arrives — only the latest request is applied, otherwise a late answer
   * would render a stale sort. */
  async refresh(type: SessionType) {
    const request = ++this.#latest;
    const list = await listSavedSessions(type);
    if (request === this.#latest) this.entries = list;
  }

  /** A **write** failure is never swallowed (golden rule no. 6): a read-only
   * presets folder must show, not let one believe the session is saved. It
   * throws to the caller, and the dialog stays open to retry under another
   * name. */
  async save(session: SavedSession, type: SessionType) {
    try {
      await saveSession(session);
      this.dialog = null;
    } finally {
      await this.refresh(type);
    }
  }

  /** By path, and only for ours: a preset composed in Content Manager is
   * listed here, never deleted from here (SESSION§3.6). The backend refuses
   * anyway; the cross simply is not shown. */
  async remove(path: string, type: SessionType) {
    try {
      await deleteSavedSession(path);
    } finally {
      await this.refresh(type);
    }
  }
}

/** What tells two entries apart at a glance: their type — shown since it no
 * longer filters —, their track, their date. A preset that could not be
 * converted shows its reason instead: the row stays, and says why it does not
 * load (SESSION§3.6). */
export function savedSessionMeta(e: SessionPreset, cards: ModCard[]): string {
  if (!e.session) return errorText(e.reason ?? "");
  const s = e.session;
  const track = cards.find((c) => c.id_interne === s.setup.track_id)?.display_name ?? s.setup.track_id;
  return [t(`launch.type.${s.setup.session_type}`), track, formatSavedAt(s.savedAt)].filter(Boolean).join(" · ");
}
