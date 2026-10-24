<script lang="ts" module>
  /** Stops the sound playing anywhere in the editor: one at a time. */
  let stopPlaying: (() => void) | null = null;
</script>

<script lang="ts">
  // Listen to one of the story's sounds from where it is picked - a place's
  // ambience, a phone's tap - to check it is the right one: play, pause,
  // and it stops when the sound picked changes or the field goes away. One
  // sound at a time across the editor: starting one stops the last.
  import { onDestroy } from "svelte";
  import { t } from "$lib/i18n/index.svelte";
  import { mediaUrl } from "$lib/story/player/speakers";

  interface Props {
    /** The story's folder; `null` for the sample. */
    folder: string | null;
    /** A path inside `media/`; empty for none. */
    path: string;
  }
  const { folder, path }: Props = $props();

  let playing = $state(false);
  let audio: HTMLAudioElement | null = null;

  function stop() {
    audio?.pause();
    audio = null;
    playing = false;
  }

  function toggle() {
    if (playing) return stop();
    const src = mediaUrl(folder, path);
    if (!src) return;
    stopPlaying?.();
    audio = new Audio(src);
    audio.onended = stop;
    // A file the webview cannot play stays silent: the button goes back.
    audio.play().then(
      () => (playing = true),
      () => stop(),
    );
    stopPlaying = stop;
  }

  // Another sound picked: the one playing was the last one's.
  $effect(() => {
    void path;
    return stop;
  });
  onDestroy(stop);
</script>

<button
  class="btn btn-ghost play"
  disabled={!path}
  aria-pressed={playing}
  title={t(playing ? "story.frames.pauseSound" : "story.frames.playSound")}
  aria-label={t(playing ? "story.frames.pauseSound" : "story.frames.playSound")}
  onclick={toggle}>{playing ? "⏸" : "▶"}</button
>

<style>
  .play {
    padding: 4px 9px;
    flex-shrink: 0;
  }
</style>
