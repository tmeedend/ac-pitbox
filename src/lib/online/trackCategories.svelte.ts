// What the Online page knows of track categories beyond the library, shared
// by the list's filter and the panel's category line (`categories.ts`). Read
// once when the page opens, and again after each choice made on the panel.
import { invoke } from "@tauri-apps/api/core";
import { NO_KNOWLEDGE, type CategoryKnowledge } from "./categories";

export const trackCategories = $state<{ knowledge: CategoryKnowledge }>({ knowledge: NO_KNOWLEDGE });

/** Loads the vocabulary, the choices and the catalogue. A failure leaves the
 * page on the library alone, logged: the list still filters what it knows. */
export async function loadTrackCategories(): Promise<void> {
  try {
    trackCategories.knowledge = await invoke<CategoryKnowledge>("online_track_categories");
  } catch (e) {
    console.error("online track categories", e);
  }
}

/** Records the categories chosen for a track — none forgets the choice. An
 * error is the caller's to show: a choice that did not stick must not look
 * made. */
export async function chooseTrackCategory(trackId: string, categories: string[]): Promise<void> {
  trackCategories.knowledge = await invoke<CategoryKnowledge>("choose_online_track_category", {
    trackId,
    categories,
  });
}
