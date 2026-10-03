<script lang="ts">
  // "Notify me" on a server one cannot enter yet (SPEC-play-online.md, v2:
  // "Prévenir quand un slot se libère"): the server full, or the chosen car
  // without a free slot. The watch itself runs with the shell
  // (`online/watch.svelte.ts`), whatever screen is open; here it is asked for,
  // shown while it runs, and stopped. Not offered when the background watch is
  // switched off (Settings › General).
  import { t } from "$lib/i18n/index.svelte";
  import { serverKey, type CarSlots, type ServerSummary } from "$lib/online/online";
  import { onlineWatchOn, slotWatch, stopSlotWatch, watchSlot } from "$lib/online/watch.svelte";

  interface Props {
    /** The server as last read (`/INFO` when the panel has it). */
    server: ServerSummary;
    chosen: CarSlots | null;
  }
  let { server, chosen }: Props = $props();

  const full = $derived(server.clients >= server.max_clients);
  /** The car the slot is awaited for: the chosen one when it is the one
   * without room, else any. */
  const car = $derived(chosen && chosen.free === 0 ? chosen.id : null);
  const watching = $derived(slotWatch() !== null && serverKey(slotWatch()!.server) === serverKey(server));
  const offered = $derived(onlineWatchOn() && !server.booking && (full || car !== null));
</script>

{#if watching}
  <div class="notify">
    <span>{t("online.notifyWatching")}</span>
    <button class="btn" type="button" onclick={stopSlotWatch}>{t("online.notifyStop")}</button>
  </div>
{:else if offered}
  <div class="notify">
    <button class="btn" type="button" onclick={() => watchSlot(server, car)}>{t("online.notifyMe")}</button>
  </div>
{/if}

<style>
  .notify {
    display: flex;
    align-items: center;
    gap: 10px;
    padding: 8px 16px;
    border-top: 1px solid var(--line);
    font-size: 12px;
    color: var(--txt2);
  }
  .notify .btn {
    margin-left: auto;
  }
</style>
