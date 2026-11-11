// Thumbnails of protected cars, rendered by the game (docs/SPEC-grille.md
// GRILLE§7.1): the queue behind the right click, and what the task panel shows.
//
// Up to twelve cars per game session, one session after the other: starting
// the game costs twenty to forty seconds and two sessions cannot overlap. A
// session renders the thumbnail of **every preset in use** — both grid
// densities and the session car — the car is loaded already, a second image
// costs a second, and switching density afterwards must not ask for another
// session.
import { listen } from "@tauri-apps/api/event";

import { gridThumbnail, protectedGridCars, renderInGame, type GridTemplate, type InGamePhase } from "./gridThumbs";
import { adoptGridThumb, gridThumbFailed } from "./gridThumbs.svelte";
import { THUMB_SLOTS, gridThumbsOn, presetFor, renderTemplate } from "./gridThumbPrefs.svelte";
import { listLibrary } from "$lib/library/library";

/** A car to photograph: its id, the skin its card shows, its name for the
 * task panel. */
export interface InGameCar {
  id: string;
  skin: string | null;
  name: string;
}

/**
 * Whether the right click offers the in-game render for this car: thumbnails
 * on, and a car known not to render in Pit Box under the template of the view.
 * Every template is renderable by the game — its floor, baked background, disc
 * of light and auto exposure included (GRILLE§7.1).
 */
export function canRenderInGame(template: GridTemplate, car: InGameCar): boolean {
  return gridThumbsOn() && gridThumbFailed(template, car.id, car.skin);
}

/** The templates one session renders: the grid's two densities and the
 * session car's, each once — a protected car picked for a session is shown
 * there too. */
function sessionTemplates(): GridTemplate[] {
  const seen = new Set<string>();
  const list: GridTemplate[] = [];
  for (const slot of THUMB_SLOTS) {
    const template = renderTemplate(presetFor(slot));
    const key = JSON.stringify(template);
    if (seen.has(key)) continue;
    seen.add(key);
    list.push(template);
  }
  return list;
}

/**
 * Every protected car of the library, under the presets in use
 * (GRILLE§7.1): what « render them in
 * the game » takes in one go, from the generation report or the settings.
 * The names are the cards' own, for the task panel.
 */
export async function protectedCarsForGame(): Promise<InGameCar[]> {
  if (!gridThumbsOn()) return [];
  const [library, ...lists] = await Promise.all([
    listLibrary(),
    ...sessionTemplates().map((template) => protectedGridCars(template)),
  ]);
  const names = new Map(library.map((card) => [card.id_interne, card.display_name ?? card.id_interne]));
  const cars = new Map<string, InGameCar>();
  for (const car of lists.flat()) {
    if (!cars.has(car.carId)) cars.set(car.carId, { id: car.carId, skin: car.skin, name: names.get(car.carId) ?? car.carId });
  }
  return [...cars.values()];
}

const progress = $state({
  running: false,
  finished: false,
  total: 0,
  done: 0,
  failed: 0,
  /** The first car of the session under way, and how many others it holds. */
  current: null as string | null,
  others: 0,
  phase: null as InGamePhase | null,
  /** Why the last car failed — an i18n key or a technical message. */
  lastError: null as string | null,
});

/** What the task panel shows. */
export function inGameProgress() {
  return progress;
}

/** Closes the end report. */
export function dismissInGameReport(): void {
  progress.finished = false;
}

/** Cars photographed in one game session: `gameshot::CARS_PER_SESSION`. */
const CARS_PER_SESSION = 12;

const queue: InGameCar[] = [];
/** The cars of the session under way, so a second click does not queue them. */
let current = new Set<string>();
let listening = false;

/** Queues cars; a car already waiting is not queued twice. */
export function requestInGame(cars: InGameCar[]): void {
  if (!progress.running) {
    progress.total = 0;
    progress.done = 0;
    progress.failed = 0;
    progress.lastError = null;
  }
  for (const car of cars) {
    if (current.has(car.id) || queue.some((q) => q.id === car.id)) continue;
    queue.push(car);
    progress.total += 1;
  }
  void drain();
}

async function drain(): Promise<void> {
  if (progress.running || queue.length === 0) return;
  // Before the first `await`: a second click during it must find the queue
  // running, or two sessions would start at once.
  progress.running = true;
  progress.finished = false;
  try {
    if (!listening) {
      listening = true;
      await listen<InGamePhase>("gameshot://phase", (e) => (progress.phase = e.payload));
    }
    // One game session per batch: the session costs about 25 seconds
    // whatever the number of cars, a car about 3 more (GRILLE§7.1).
    let batch = queue.splice(0, CARS_PER_SESSION);
    while (batch.length) {
      current = new Set(batch.map((car) => car.id));
      progress.current = batch[0].name;
      progress.others = batch.length - 1;
      progress.phase = null;
      try {
        const failed = await shoot(batch);
        progress.done += batch.length - failed;
        progress.failed += failed;
        if (failed) progress.lastError = "errors.gameShotNoImage";
      } catch (e) {
        // Not remembered: the cards keep their photo, the entry stays in the
        // menu, and the report says why (GRILLE§7.1).
        console.error("vignettes dans le jeu", batch.map((car) => car.id), e);
        progress.failed += batch.length;
        progress.lastError = typeof e === "string" ? e : String(e);
        // A game still open (stuck, or behind CSP's crash report) refuses
        // every following session: the rest of the queue would fail batch by
        // batch for the same reason, so it stops here, counted as failed.
        if (progress.lastError === "errors.gameShotTimedOut" || progress.lastError === "errors.gameRunning") {
          progress.failed += queue.length;
          queue.length = 0;
        }
      }
      batch = queue.splice(0, CARS_PER_SESSION);
    }
  } finally {
    progress.running = false;
    current = new Set();
    progress.current = null;
    progress.others = 0;
    progress.phase = null;
    progress.finished = progress.done + progress.failed > 0;
  }
}

/** One game session for these cars. Returns how many came back without any
 * image (the others are shown at once). */
async function shoot(cars: InGameCar[]): Promise<number> {
  const templates = sessionTemplates();
  // The entry names come from the backend, as for every thumbnail: they are
  // what makes the game's image expire with the mod, like the others.
  // A preset whose thumbnail exists already is not rendered again.
  const wanted = await Promise.all(
    cars.map(async (car) => {
      const known = await Promise.all(
        templates.map(async (template) => ({ template, thumb: await gridThumbnail(car.id, car.skin, template) })),
      );
      const outputs = known.filter((k) => !k.thumb.path).map((k) => ({ template: k.template, stem: k.thumb.stem }));
      return { car, outputs };
    }),
  );
  const toRender = wanted.filter((w) => w.outputs.length > 0);
  if (!toRender.length) return 0;
  const results = await renderInGame(
    toRender.map(({ car, outputs }) => ({ carId: car.id, skin: car.skin, outputs })),
  );
  let failed = 0;
  for (const { car, outputs } of toRender) {
    let any = false;
    for (const output of outputs) {
      const path = results.find((r) => r.stem === output.stem)?.path;
      if (path) {
        adoptGridThumb(output.template, car.id, car.skin, path);
        any = true;
      }
    }
    if (!any) failed += 1;
  }
  return failed;
}
