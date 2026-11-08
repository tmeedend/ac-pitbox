// The one preview playing (▶ of a music picker, §16.8): one at a time across
// every picker on screen, so a ■ is only ever shown where it would stop
// something. The engine steps aside while it plays (`engine::PreviewHandle`).
import { listen } from "@tauri-apps/api/event";
import { musicPreviewStart, musicPreviewStop } from "./music";

const state = $state<{ source: string | null }>({ source: null });

// A preview played to its end (or stopped) gives its ■ back as ▶. Only the
// one still shown: a preview replaced by another ends too, after the new one
// has started.
void listen<string>("music:preview-ended", (e) => {
  if (state.source === e.payload) state.source = null;
}).catch((err) => console.warn("music: preview end not followed", err));

/** The source being previewed, `null` when none. Reactive. */
export function previewing(): string | null {
  return state.source;
}

/** Starts listening to `source`, or stops when it is the one playing. */
export async function togglePreview(source: string, volume: number): Promise<void> {
  if (state.source === source) return stopPreview();
  await musicPreviewStart(source, volume);
  state.source = source;
}

export async function stopPreview(): Promise<void> {
  if (!state.source) return;
  state.source = null;
  await musicPreviewStop();
}
