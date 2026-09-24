<script lang="ts">
  // Now playing (MUSIQUE§5.5): title, artist and cover of the track that just
  // started, in the bottom-right stack.
  //
  // **The only toast of the stack that closes on its own.** Every other one
  // stays until dismissed, and says why in its header: a lost setting, a
  // device to configure, a report. This one is information in passing — the
  // music already changed, nothing is asked — and a card left in the corner
  // of Big Picture for every track would pile up in front of the screen it is
  // decorating. It holds while the pointer rests on it, so it can still be
  // read to the end.
  import { fade, fly } from "svelte/transition";
  import Toast from "./Toast.svelte";
  import { nowPlaying, onMusicTrack, type NowPlaying } from "$lib/shell/music";
  import { bigPictureState } from "$lib/shell/bigpicture.svelte";

  /** Long enough to read a title and an artist from a couch. */
  const VISIBLE_MS = 6000;
  /** After the pointer leaves: the reading is done, no need for the full time. */
  const AFTER_HOVER_MS = 2000;

  let track = $state<NowPlaying | null>(null);
  let timer: ReturnType<typeof setTimeout> | undefined;

  function hideIn(ms: number) {
    clearTimeout(timer);
    timer = setTimeout(() => (track = null), ms);
  }

  function hide() {
    clearTimeout(timer);
    track = null;
  }

  $effect(() => {
    const unlisten = onMusicTrack((info) => {
      // A new track while the card is up replaces its content in place and
      // restarts the clock — no second card, no exit-then-entry flicker.
      track = nowPlaying(info);
      hideIn(VISIBLE_MS);
    });
    return () => {
      clearTimeout(timer);
      unlisten.then((f) => f());
    };
  });

  // Leaving Big Picture fades the music out: the card announcing it goes too.
  $effect(() => {
    if (!bigPictureState.active) hide();
  });
</script>

{#if track}
  <!-- The wrapper carries the transitions and the hover: `Toast` owns
       neither, and the stack only lays out its direct children. -->
  <div
    class="wrap"
    role="status"
    in:fly={{ x: 40, duration: 220 }}
    out:fade={{ duration: 300 }}
    onmouseenter={() => clearTimeout(timer)}
    onmouseleave={() => hideIn(AFTER_HOVER_MS)}
  >
    <Toast icon="♪" title={track.title} truncate onclose={hide}>
      {#if track.cover || track.artist || track.album}
        <div class="np">
          {#if track.cover}<img class="cover" src={track.cover} alt="" />{/if}
          <div class="meta">
            {#if track.artist}<div class="artist">{track.artist}</div>{/if}
            {#if track.album}<div class="album">{track.album}</div>{/if}
          </div>
        </div>
      {/if}
    </Toast>
  </div>
{/if}

<style>
  .np {
    display: flex;
    align-items: center;
    gap: 12px;
  }
  .cover {
    flex: none;
    width: 56px;
    height: 56px;
    object-fit: cover;
    border: 1px solid var(--line);
  }
  .meta {
    min-width: 0;
    display: flex;
    flex-direction: column;
    gap: 3px;
  }
  .artist,
  .album {
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }
  .artist {
    color: var(--txt2);
  }
  .album {
    color: var(--muted);
  }
</style>
