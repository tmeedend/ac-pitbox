<script lang="ts">
  // What the Online watch found (`online/watch.svelte.ts`): a friend who just
  // connected, a slot freed on the server one waited for. One notification
  // each, in the stack; "Open" leads to the server's panel on the Online
  // page, whatever screen is open.
  import Toast from "./Toast.svelte";
  import { t } from "$lib/i18n/index.svelte";
  import { dismissAlert, watchAlerts, type WatchAlert } from "$lib/online/watch.svelte";
  import { openOnline } from "$lib/online/intent.svelte";

  function open(alert: WatchAlert) {
    dismissAlert(alert.id);
    void openOnline({ kind: "server", server: alert.server });
  }
</script>

{#each watchAlerts() as alert (alert.id)}
  <Toast
    title={alert.kind === "friend" ? t("online.watchFriend", { name: alert.name }) : t("online.watchSlot")}
    tone="info"
    onclose={() => dismissAlert(alert.id)}
  >
    {#snippet actions()}
      <button class="btn" type="button" onclick={() => open(alert)}>{t("online.watchOpen")}</button>
    {/snippet}
    <p class="server">{alert.server.name}</p>
  </Toast>
{/each}

<style>
  .server {
    font-size: 12px;
    color: var(--txt2);
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }
</style>
