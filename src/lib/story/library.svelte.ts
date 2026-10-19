// The player's library, as the story editor sees it: the cars and tracks an
// event picks from (SPEC-story-mode, Create Your Own: Events). Read once
// per run of the app - the library rarely changes while an author writes -
// and a track's layouts only when an event shows that track.
import { getModDetail, listLibrary, type LayoutItem, type ModCard, type ModKind } from "$lib/library/library";

export const studioLibrary = $state<{
  cards: ModCard[] | null;
  /** Layouts by track id, once read. */
  layouts: Record<string, LayoutItem[]>;
}>({ cards: null, layouts: {} });

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
