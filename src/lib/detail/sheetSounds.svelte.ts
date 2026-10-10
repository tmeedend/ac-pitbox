// The engine sounds of a car's sheet (§8.3): the list of sound mods for the
// car, picking one — an exclusive swap of the game's files, the original
// restorable — and listening to one without deploying anything.
//
// Out of `DetailPage.svelte`, like `actions.svelte.ts`: the page keeps what is
// shown, this keeps the sound gestures and their state. `EngineSoundBlock`
// only draws what it is given — it is unmounted on every tab switch.
import { errorText } from "$lib/errors";
import { activateSound, listSubMods, restoreSound, type SubModRow } from "$lib/inventory/submods";
import { stopEngine, toggleEngine } from "$lib/detail/enginePlayer.svelte";

/** What the sounds read from the sheet. Getters: both change under it. */
export interface SheetSubsHost {
  /** The mod the sheet is open on — a reply for another one is dropped. */
  readonly id: string;
  /** The mod once its detail is loaded: no gesture before that. */
  readonly loadedId: string | null;
  /** The sheet's error line; `""` clears it. */
  setError(message: string): void;
}

export class SheetSounds {
  #host: SheetSubsHost;

  list = $state<SubModRow[]>([]);
  /** A sound is being deployed: the radio buttons are disarmed meanwhile. */
  busy = $state(false);

  /** Must be built while the page initialises: it registers the effect that
   * stops the engine. */
  constructor(host: SheetSubsHost) {
    this.#host = host;
    // An engine outliving its button is an engine nobody can stop any more.
    //
    // The effect **reads `id`**: without that read it would never run again,
    // and moving to a neighbouring car (`openSibling`, which replaces the
    // content without unmounting the sheet) would leave the previous car's
    // engine running under the next one.
    $effect(() => {
      void host.id;
      return () => stopEngine();
    });
  }

  async load(parent: string) {
    const all = await listSubMods(parent);
    if (parent !== this.#host.id) return;
    this.list = all.filter((s) => s.sub_type === "SOUND");
  }

  /** Sound = exclusive swap (§8.3): one active, the original restorable. */
  async pick(subId: string | null) {
    const parent = this.#host.loadedId;
    if (!parent || this.busy) return;
    this.busy = true;
    this.#host.setError("");
    try {
      if (subId) await activateSound(subId);
      else await restoreSound(parent);
      await this.load(parent);
    } catch (e) {
      this.#host.setError(errorText(e));
    } finally {
      this.busy = false;
    }
  }

  /**
   * Listen to an entry, deploying nothing — not to be confused with `pick`
   * just above, which replaces the game's files.
   *
   * Does not set `busy`: that flag disarms the radio buttons for the time of a
   * deployment, and listening is not one. Both gestures must stay possible at
   * the same time.
   */
  async listen(subId: string | null) {
    const parent = this.#host.loadedId;
    if (!parent) return;
    this.#host.setError("");
    try {
      const clip = await toggleEngine(parent, subId);
      // Which sample was picked, and how: invisible on screen, but it is what
      // the log needs when someone reports having heard the horn.
      if (clip) {
        console.info(
          `[son moteur] ${clip.codec} #${clip.sampleIndex}` +
            ` ${clip.sampleName ?? "(sans nom)"} par ${clip.pickedBy}, ${clip.seconds.toFixed(1)} s`,
        );
      }
    } catch (e) {
      this.#host.setError(errorText(e));
    }
  }
}
