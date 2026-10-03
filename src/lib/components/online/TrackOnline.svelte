<script lang="ts">
  // "Online: 4 servers · 37 players" on a track's sheet (SPEC-play-online.md,
  // v2: the library leading to the Online page, case 5 — the tracks one
  // drives). Counted by the backend on the raw lobby list it shares with the
  // Online page (`online/lobby_cache.rs`): no library, no disk, no server. The sheet does not wait for
  // it: the line appears when the list is there, and a click opens the Online
  // page on this track.
  //
  // Nothing at all when the lobby cannot be reached or nobody drives here:
  // an error, or a zero, would be noise on a sheet about something else.
  import { onMount } from "svelte";
  import { t } from "$lib/i18n/index.svelte";
  import { trackActivity, type TrackActivity } from "$lib/online/online";
  import { openOnline } from "$lib/online/intent.svelte";

  interface Props {
    /** The track's folder. */
    trackId: string;
    /** Its layout folders, `""` for a single layout. */
    layouts: string[];
  }
  let { trackId, layouts: trackLayouts }: Props = $props();

  let activity = $state<TrackActivity | null>(null);
  onMount(() => {
    trackActivity(trackId, trackLayouts)
      .then((a) => (activity = a))
      .catch((e) => console.warn("online activity for the track sheet", e));
  });
</script>

{#if activity && activity.servers > 0}
  {@const layouts = activity.layouts}
  <button class="online" type="button" onclick={() => openOnline({ kind: "track", layouts })}>
    <span class="lbl-key">{t("nav.online")}</span>
    <span class="mono">{t("online.trackActivity", { servers: activity.servers, players: activity.players })}</span>
    <span class="go" aria-hidden="true">→</span>
  </button>
{/if}

<style>
  .online {
    display: flex;
    align-items: center;
    gap: 10px;
    width: 100%;
    padding: 8px 10px;
    background: var(--panel2);
    border: 1px solid var(--line);
    color: var(--txt2);
    font-size: 12px;
    text-align: left;
  }
  .online:hover {
    background: var(--raised);
    color: var(--txt);
  }
  .go {
    margin-left: auto;
    color: var(--muted);
  }
</style>
