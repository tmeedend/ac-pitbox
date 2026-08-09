<script lang="ts">
  // Onglet Médias — sous-vue Replays (§6.1). « Lire dans CM » passe le
  // chemin du .acreplay en argument à l'exécutable Content Manager, comme
  // l'association de fichier Windows le ferait au double-clic (même
  // mécanisme que launch()/open_content_manager, voir launch.rs).
  import { listMediaReplays, linkMediaManually, openMediaFolder, launchReplay, type ReplayFile } from "$lib/media";
  import { open } from "@tauri-apps/plugin-dialog";
  import { errorText } from "$lib/errors";
  import { t } from "$lib/i18n/index.svelte";

  let {
    modId,
    onerror,
  }: {
    modId: string;
    onerror: (message: string) => void;
  } = $props();

  let files = $state<ReplayFile[]>([]);
  let linking = $state(false);
  let launching = $state<string | null>(null);

  $effect(() => {
    const current = modId;
    files = [];
    listMediaReplays(current).then((f) => {
      if (current === modId) files = f;
    });
  });

  function fmtDate(iso: string | null): string {
    if (!iso) return "—";
    const d = new Date(iso);
    return Number.isNaN(d.getTime()) ? "—" : d.toLocaleString();
  }

  async function openFolder() {
    try {
      await openMediaFolder("REPLAY");
    } catch (e) {
      onerror(errorText(e));
    }
  }

  async function playReplay(path: string) {
    if (launching) return;
    launching = path;
    try {
      await launchReplay(path);
    } catch (e) {
      onerror(errorText(e));
    } finally {
      launching = null;
    }
  }

  async function linkManually() {
    if (linking) return;
    const picked = await open({
      multiple: false,
      title: t("detail.mediaLinkPickTitle"),
      filters: [{ name: "Replay", extensions: ["acreplay"] }],
    });
    if (!picked || typeof picked !== "string") return;
    linking = true;
    try {
      await linkMediaManually(modId, "REPLAY", picked);
      files = await listMediaReplays(modId);
    } catch (e) {
      onerror(errorText(e));
    } finally {
      linking = false;
    }
  }
</script>

<section class="blk">
  <header class="blk-h">
    <span class="blk-t">{t("detail.mediaReplaysTitle")}</span>
    <span class="blk-n">{files.length}</span>
  </header>
  <div class="blk-b">
    {#if files.length}
      <ul class="replay-list">
        {#each files as f (f.path)}
          <li class="replay">
            <div class="replay-b">
              <div class="replay-name">{f.file_name}</div>
              <div class="replay-meta mono">
                <span>{fmtDate(f.recorded_at)}</span>
                {#if f.session_type}<span>{f.session_type}</span>{/if}
                {#if f.matched_counterpart}<span>{f.matched_counterpart}</span>{/if}
              </div>
            </div>
            <button class="btn-ghost play" type="button" onclick={() => playReplay(f.path)} disabled={launching === f.path}>
              {launching === f.path ? t("common.working") : t("detail.playReplay")}
            </button>
          </li>
        {/each}
      </ul>
    {:else}
      <p class="empty">{t("detail.noReplays")}</p>
    {/if}
    <div class="actions">
      <button class="btn-ghost" type="button" onclick={openFolder}>{t("detail.openMediaFolder")}</button>
      <button class="btn-ghost" type="button" onclick={linkManually} disabled={linking}>
        {t("detail.mediaLinkManually")}
      </button>
    </div>
  </div>
</section>

<style>
  .replay-list {
    list-style: none;
    display: flex;
    flex-direction: column;
    gap: 6px;
    margin-bottom: 14px;
  }
  .replay {
    display: flex;
    align-items: center;
    gap: 10px;
    border: 1px solid var(--line);
    background: var(--raised);
    padding: 8px 11px;
  }
  .replay-b {
    flex: 1;
    min-width: 0;
  }
  .replay .play {
    flex: none;
    font-size: 11px;
    padding: 6px 10px;
  }
  .replay-name {
    font-size: 12px;
    color: var(--txt2);
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }
  .replay-meta {
    display: flex;
    gap: 10px;
    color: var(--muted2);
    font-size: 10px;
    margin-top: 3px;
  }
  .empty {
    color: var(--muted);
    font-size: 12px;
    margin-bottom: 14px;
  }
  .actions {
    display: flex;
    gap: 8px;
  }
</style>
