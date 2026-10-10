// The skins of a track's sheet (§8): several can be active at once, and
// switching one recomposes the track's `skins/default/` on the backend.
//
// Out of `DetailPage.svelte`, like `actions.svelte.ts` and `sheetSounds`: the
// page keeps what is shown, this keeps the list, its state and the toggle.
import { errorText } from "$lib/errors";
import {
  listActiveTrackSkins,
  listSubMods,
  setTrackSkinActive,
  syncTrackSkins,
  type SubModRow,
} from "$lib/inventory/submods";
import type { SheetSubsHost } from "$lib/detail/sheetSounds.svelte";

export class SheetTrackSkins {
  #host: SheetSubsHost;

  list = $state<SubModRow[]>([]);
  /** Names of the active skins. */
  active = $state<string[]>([]);
  loading = $state(true);
  /** A switch is being applied: one at a time. */
  busy = $state(false);

  constructor(host: SheetSubsHost) {
    this.#host = host;
  }

  async load(parent: string) {
    try {
      // First recognise the skins bundled with the mod (§8) — otherwise they
      // would not be in the `listSubMods` that follows yet.
      await syncTrackSkins(parent);
      if (parent !== this.#host.id) return;
      const [all, active] = await Promise.all([listSubMods(parent), listActiveTrackSkins(parent)]);
      if (parent !== this.#host.id) return;
      this.list = all.filter((s) => s.sub_type === "TRACK_SKIN");
      this.active = active;
    } finally {
      if (parent === this.#host.id) this.loading = false;
    }
  }

  async toggle(name: string) {
    if (this.busy) return;
    this.busy = true;
    const wasActive = this.active.includes(name);
    try {
      await setTrackSkinActive(this.#host.id, name, !wasActive);
      this.active = wasActive ? this.active.filter((n) => n !== name) : [...this.active, name];
    } catch (e) {
      this.#host.setError(errorText(e));
    } finally {
      this.busy = false;
    }
  }
}
