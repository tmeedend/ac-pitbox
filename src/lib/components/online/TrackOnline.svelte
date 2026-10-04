<script lang="ts">
  // "Online: 4 servers · 37 players" on a track's sheet (SPEC-play-online.md,
  // v2: the library leading to the Online page, case 5 — the tracks one
  // drives). Counted by the backend on the raw lobby list it shares with the
  // Online page (`online/lobby_cache.rs`): no library, no disk, no server.
  // The sheet does not wait for it: the card appears when the count is
  // there, and its button opens the Online page on this track.
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

<!-- A card of the column like its neighbours (reported: as a bare line
     between two cards it read as dropped there by chance), and the last of
     them: it speaks of the track elsewhere, not of the track here. -->
{#if activity && activity.servers > 0}
  {@const layouts = activity.layouts}
  <section class="blk">
    <header class="blk-h">
      <span class="blk-t">{t("nav.online")}</span>
      <span class="blk-n">{t("online.trackActivityPlayers", { count: activity.players })}</span>
    </header>
    <div class="blk-b online">
      <span>{t("online.trackActivityServers", { count: activity.servers })}</span>
      <button class="btn" type="button" onclick={() => openOnline({ kind: "track", layouts })}>
        {t("online.trackActivityOpen")}
      </button>
    </div>
  </section>
{/if}

<style>
  .online {
    display: flex;
    align-items: center;
    gap: 10px;
    font-size: 12px;
    color: var(--txt2);
  }
  .online .btn {
    margin-left: auto;
  }
</style>
