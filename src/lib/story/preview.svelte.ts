// What the Content screen's preview is set with, by content item: the values
// the author set to preview with, and the replies chosen so far. Kept while
// Pit Box runs, never written: it is a way of looking at an item, not part of
// the story.
import type { PreviewInputs } from "./preview";

export const previewInputs = $state<Record<string, PreviewInputs>>({});

/** The inputs of item `id`, made once. */
export function inputsOf(id: string): PreviewInputs {
  previewInputs[id] ??= { setup: {}, replies: [] };
  return previewInputs[id];
}
