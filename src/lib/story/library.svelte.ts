// The player's library, as the story editor sees it: the cars and tracks an
// event picks from (SPEC-story-mode, Create Your Own: Events). Read once
// per run of the app - the library rarely changes while an author writes -
// a track's layouts only when an event shows that track, and a car's
// liveries only when a grid shows that car. The game's countries are the
// app's own cache (`$lib/flags.svelte`).
import { getModDetail, listLibrary, type LayoutItem, type ModCard, type ModKind } from "$lib/library/library";
import { listModSkins, type SkinItem } from "$lib/launch/launch";

export const studioLibrary = $state<{
  cards: ModCard[] | null;
  /** Layouts by track id, once read. */
  layouts: Record<string, LayoutItem[]>;
  /** Liveries by car id, once read. */
  skins: Record<string, SkinItem[]>;
}>({ cards: null, layouts: {}, skins: {} });

let loading: Promise<void> | null = null;

export function loadLibrary(): Promise<void> {
  loading ??= listLibrary().then(
    (cards) => {
      studioLibrary.cards = cards;
    },
    (e) => {
      // Tried again on the next visit; until then the ids show as they are.
      loading = null;
      console.warn("story studio: library not read", e);
    },
  );
  return loading;
}

/** The library's card for `id`, `undefined` while it is not read or when
 * the player does not have it. */
export function cardOf(kind: ModKind, id: string | undefined): ModCard | undefined {
  if (!id) return undefined;
  return studioLibrary.cards?.find((c) => c.kind === kind && c.id_interne === id);
}

const layoutReads: Record<string, Promise<LayoutItem[]>> = {};

/** The layouts of `trackId`, read once; `studioLibrary.layouts` follows. */
export function loadLayouts(trackId: string): Promise<LayoutItem[]> {
  layoutReads[trackId] ??= getModDetail(trackId).then(
    (d) => (studioLibrary.layouts[trackId] = d?.track?.layouts ?? []),
    (e) => {
      delete layoutReads[trackId];
      console.warn("story studio: layouts not read", trackId, e);
      return [];
    },
  );
  return layoutReads[trackId];
}

const skinReads: Record<string, Promise<SkinItem[]>> = {};

/** The liveries of `carId`, read once; `studioLibrary.skins` follows. */
export function loadSkins(carId: string): Promise<SkinItem[]> {
  skinReads[carId] ??= listModSkins(carId).then(
    (skins) => (studioLibrary.skins[carId] = skins),
    (e) => {
      delete skinReads[carId];
      console.warn("story studio: liveries not read", carId, e);
      return [];
    },
  );
  return skinReads[carId];
}
