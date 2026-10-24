<script lang="ts">
  // The scene's ambient sound (SPEC-story-mode, Scenes, frame and sound): a
  // loop while the place shows, faded out and the next one faded in when
  // the sound changes; muted at the player's word. Nothing is drawn.
  import { onDestroy, untrack } from "svelte";

  interface Props {
    /** The sound's URL; `null`, silence. */
    src: string | null;
    muted: boolean;
  }
  const { src, muted }: Props = $props();

  const FADE_MS = 1000;
  /** The loop playing, or fading in. */
  let current: HTMLAudioElement | null = null;
  /** The loops fading out. */
  const leaving = new Set<HTMLAudioElement>();

  /** Brings `audio`'s volume to `to` in `FADE_MS`, then calls `done`. A new
   * fade on the same element replaces the one under way. */
  const fades = new WeakMap<HTMLAudioElement, number>();
  function fade(audio: HTMLAudioElement, to: number, done?: () => void) {
    const from = audio.volume;
    const start = performance.now();
    const step = (now: number) => {
      const k = Math.min(1, (now - start) / FADE_MS);
      audio.volume = from + (to - from) * k;
      if (k < 1) fades.set(audio, requestAnimationFrame(step));
      else done?.();
    };
    cancelAnimationFrame(fades.get(audio) ?? 0);
    fades.set(audio, requestAnimationFrame(step));
  }

  function stop(audio: HTMLAudioElement) {
    leaving.add(audio);
    fade(audio, 0, () => {
      audio.pause();
      audio.removeAttribute("src");
      leaving.delete(audio);
    });
  }

  $effect(() => {
    const url = src;
    if (current?.dataset.src === url) return;
    if (current) stop(current);
    current = null;
    if (!url) return;
    const audio = new Audio(url);
    audio.dataset.src = url;
    audio.loop = true;
    audio.volume = 0;
    audio.muted = untrack(() => muted);
    // A refused start (no sound device, a file the webview cannot play)
    // leaves the scene silent: nothing the player can act on.
    audio.play().catch(() => {});
    fade(audio, 1);
    current = audio;
  });

  $effect(() => {
    const m = muted;
    if (current) current.muted = m;
    for (const a of leaving) a.muted = m;
  });

  onDestroy(() => {
    for (const a of [current, ...leaving]) {
      if (!a) continue;
      cancelAnimationFrame(fades.get(a) ?? 0);
      a.pause();
    }
  });
</script>
