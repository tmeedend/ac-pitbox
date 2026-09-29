// Starting a session, from the button to Content Manager (SESSION§2.3).
//
// Two things the screen used to carry inline: the Steam gate, with the modal
// state it drives, and the one-time resolution of what `setup` does not hold
// (the driver). The screen keeps what is its own — saving the preset first,
// and the banner that reports the outcome — and passes it in as `run`.
import { carClassOf, driverFor, isEmpty } from "$lib/driver/driverOverride.svelte";
import { getModDetail } from "$lib/library/library";
import { isSteamRunning, launchSession, type RaceSetup } from "./launch";

// A failure of the check itself must not keep anyone from playing: in doubt
// it lets through, a failure on CM's side remaining the worst case.
async function steamReady(): Promise<boolean> {
  try {
    return await isSteamRunning();
  } catch {
    return true;
  }
}

/** The Steam gate in front of a launch. Assetto Corsa is a Steam game: without
 * Steam, the launch fails on Content Manager's side after Pit Box has let go —
 * no error comes back, the user just sees a session that never starts. The
 * only moment left to explain is before launching. */
export class LaunchSequence {
  launching = $state(false);
  /** The "start Steam first" modal is open. */
  steamPromptOpen = $state(false);
  /** The user said Steam was started, and it still is not. */
  steamStillMissing = $state(false);
  steamChecking = $state(false);

  #canLaunch: () => boolean;
  #run: () => Promise<void>;

  constructor(opts: { canLaunch: () => boolean; run: () => Promise<void> }) {
    this.#canLaunch = opts.canLaunch;
    this.#run = opts.run;
  }

  async launch() {
    if (this.launching || !this.#canLaunch()) return;
    if (!(await steamReady())) {
      this.steamStillMissing = false;
      this.steamPromptOpen = true;
      return;
    }
    await this.#go();
  }

  async confirmSteamStarted() {
    if (this.steamChecking) return;
    this.steamChecking = true;
    const ok = await steamReady();
    this.steamChecking = false;
    if (!ok) {
      this.steamStillMissing = true;
      return;
    }
    this.steamPromptOpen = false;
    await this.#go();
  }

  dismissSteamPrompt() {
    this.steamPromptOpen = false;
  }

  async #go() {
    this.launching = true;
    try {
      await this.#run();
    } finally {
      this.launching = false;
    }
  }
}

/** Sends the session to the game. The driver is resolved **at launch**, not
 * kept up to date in `setup`: its source is the per-car cascade (`driverFor`),
 * which depends on the chosen car and on the default outfit, two things that
 * move elsewhere in the app. The car's class decides which of the two default
 * outfits applies: asked here, once, when it serves. */
export async function startSession(setup: RaceSetup): Promise<void> {
  const carDetail = setup.car_id ? await getModDetail(setup.car_id).catch(() => null) : null;
  const outfit = driverFor(setup.car_id || null, carClassOf(carDetail?.car_class));
  await launchSession({
    ...$state.snapshot(setup),
    driver: isEmpty(outfit) ? null : { model: outfit.body, suit: outfit.suit, gloves: outfit.gloves, helmet: outfit.helmet },
  });
}
