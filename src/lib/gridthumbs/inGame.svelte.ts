// Thumbnails of protected cars, rendered by the game (docs/SPEC-grille.md
// GRILLE§7.1): the queue behind the right click, and what the task panel shows.
//
// One game session per car, one after the other: starting the game costs
// twenty to forty seconds and two sessions cannot overlap. A session renders
// the thumbnail of **both** grid densities when their presets allow it — the
// car is loaded already, a second image costs a second, and switching density
// afterwards must not ask for another session.
import { listen } from "@tauri-apps/api/event";

import { gridThumbnail, protectedGridCars, renderInGame, type GridTemplate, type InGamePhase } from "./gridThumbs";
import { adoptGridThumb, gridThumbFailed } from "./gridThumbs.svelte";
import { gridThumbsOn, presetForDensity, renderTemplate } from "./gridThumbPrefs.svelte";
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

/** The templates one session renders: the grid's two densities, each once. */
function sessionTemplates(): GridTemplate[] {
  const seen = new Set<string>();
  const list: GridTemplate[] = [];
  for (const density of ["dense", "comfortable"] as const) {
    const template = renderTemplate(presetForDensity(density));
    const key = JSON.stringify(template);
    if (seen.has(key)) continue;
    seen.add(key);
    list.push(template);
  }
  return list;
}

/**
 * Every protected car of the library, under the presets of the two densities
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
  current: null as string | null,
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

const queue: InGameCar[] = [];
/** The car of the session under way, so a second click does not queue it. */
let currentId: string | null = null;
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
    if (car.id === currentId || queue.some((q) => q.id === car.id)) continue;
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
    let car = queue.shift();
    while (car) {
      currentId = car.id;
      progress.current = car.name;
      progress.phase = null;
      try {
        await shoot(car);
        progress.done += 1;
      } catch (e) {
        // Not remembered: the card keeps its photo, the entry stays in the
        // menu, and the report says why (GRILLE§7.1).
        console.error("vignette dans le jeu", car.id, e);
        progress.failed += 1;
        progress.lastError = typeof e === "string" ? e : String(e);
        // A game still open (stuck, or behind CSP's crash report) refuses
        // every following session: the rest of the queue would fail one by
        // one for the same reason, so it stops here, counted as failed.
        if (progress.lastError === "errors.gameShotTimedOut" || progress.lastError === "errors.gameRunning") {
          progress.failed += queue.length;
          queue.length = 0;
        }
      }
      car = queue.shift();
    }
  } finally {
    progress.running = false;
    currentId = null;
    progress.current = null;
    progress.phase = null;
    progress.finished = progress.done + progress.failed > 0;
  }
}

async function shoot(car: InGameCar): Promise<void> {
  const templates = sessionTemplates();
  // The entry names come from the backend, as for every thumbnail: they are
  // what makes the game's image expire with the mod, like the others.
  // A density whose thumbnail exists already is not rendered again.
  const known = await Promise.all(
    templates.map(async (template) => ({ template, thumb: await gridThumbnail(car.id, car.skin, template) })),
  );
  const outputs = known.filter((k) => !k.thumb.path).map((k) => ({ template: k.template, stem: k.thumb.stem }));
  if (!outputs.length) return;
  const results = await renderInGame({ carId: car.id, skin: car.skin, outputs });
  for (const result of results) {
    const output = outputs.find((o) => o.stem === result.stem);
    if (output && result.path) adoptGridThumb(output.template, car.id, car.skin, result.path);
  }
}
