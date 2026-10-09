<script lang="ts">
  // The strip of a fiche whose mod is in the showcase (ESPACE§4.3), and its
  // "Recover the files" panel (ESPACE§7.1).
  //
  // Same family as `UpdateBanner`, under the tabs and on every tab: whoever
  // opens the fiche must read at once why the 3D preview, the engine sound
  // and the activation are missing. It carries the reference sentence
  // (ESPACE§4.4) — the same key as the confirmation's, never a variant.
  //
  // The panel lists only the sources that exist, in the spec's order, and
  // downloads nothing without a click. Every one of them ends in the ordinary
  // import, which recognizes the version and rehydrates it (ESPACE§7.3).
  import { openUrl } from "@tauri-apps/plugin-opener";
  import { t } from "$lib/i18n/index.svelte";
  import { errorText } from "$lib/errors";
  import type { ModKind } from "$lib/library/library";
  import { showcaseSources, type RecoverySources } from "$lib/library/showcase.svelte";
  import {
    canStartUpdate,
    installUpdate,
    modUpdateDetails,
    modUpdates,
    type UpdateDetails,
  } from "$lib/library/modUpdates.svelte";
  import { reinstallFromArchive } from "$lib/workshop/maintenance";
  import { siteHost, webSearchUrl } from "$lib/webSearch";

  interface Props {
    kind: ModKind;
    id: string;
    name: string;
    /** The files are back: the fiche reloads, and this strip leaves. */
    onrecovered: () => void;
  }
  const { kind, id, name, onrecovered }: Props = $props();

  let open = $state(false);
  let sources = $state<RecoverySources | null>(null);
  /** The registry's entry for this id; `null` when it has none. */
  let registry = $state<UpdateDetails | null>(null);
  let loading = $state(false);
  let busy = $state(false);
  let error = $state<string | null>(null);

  const downloading = $derived(modUpdates.busy?.kind === kind && modUpdates.busy?.id === id ? modUpdates.busy : null);

  // Another mod under the same fiche: what was read described the previous.
  $effect(() => {
    void id;
    open = false;
    sources = null;
    registry = null;
    error = null;
  });

  async function toggle(): Promise<void> {
    open = !open;
    if (!open || sources) return;
    loading = true;
    error = null;
    try {
      const [s, r] = await Promise.all([
        showcaseSources(id),
        // Not in the registry is the common case, not an error.
        modUpdateDetails({ kind, id }).catch(() => null),
      ]);
      sources = s;
      registry = r;
    } catch (e) {
      error = errorText(e);
    } finally {
      loading = false;
    }
  }

  async function reinstall(): Promise<void> {
    busy = true;
    error = null;
    try {
      await reinstallFromArchive(id);
      onrecovered();
    } catch (e) {
      error = errorText(e);
    } finally {
      busy = false;
    }
  }

  function download(): void {
    void installUpdate({
      kind,
      id,
      name,
      installed: null,
      available: registry?.version ?? "",
      limited: false,
    });
  }

  /** What to search for: the archive's own name is the best key there is
   * (ESPACE§8.2), the mod's name otherwise. */
  const query = $derived(sources?.file_name ?? name);

  function search(site: string | null): void {
    openUrl(webSearchUrl(query, site)).catch((e) => (error = errorText(e)));
  }

  function visit(url: string): void {
    openUrl(url).catch((e) => (error = errorText(e)));
  }
</script>

<div class="vit">
  <div class="line">
    <span class="what">{t("showcase.explain")}</span>
    <button class="btn btn-primary" type="button" onclick={toggle} aria-expanded={open}>{t("showcase.recover")}</button>
  </div>

  {#if open}
    <div class="panel">
      {#if loading}
        <span class="note">{t("common.loading")}</span>
      {:else if sources}
        {#if sources.kept_archive}
          <div class="src">
            <span class="s-what">{t("showcase.recoverKept")}</span>
            <button class="btn" type="button" disabled={busy} onclick={reinstall}>
              {busy ? t("showcase.recovering") : t("showcase.recoverKeptGo")}
            </button>
          </div>
        {/if}
        {#if registry}
          <div class="src">
            <span class="s-what">
              {t("showcase.recoverCup")}
              {#if registry.version}<span class="mono ver">{t("showcase.recoverCupVersion", { version: registry.version })}</span>{/if}
            </span>
            {#if downloading}
              <span class="note">{downloading.phase === "download" ? t("modUpdates.phaseDownload") : t("modUpdates.phaseImport")}</span>
            {:else}
              <button class="btn" type="button" disabled={!canStartUpdate()} onclick={download}>{t("showcase.recoverCupGo")}</button>
            {/if}
          </div>
        {/if}
        {#if sources.page_url}
          <div class="src">
            <span class="s-what">{t("showcase.recoverPage")}</span>
            <button class="btn" type="button" onclick={() => visit(sources!.page_url!)}>{t("showcase.open")}</button>
          </div>
        {/if}
        {#if sources.author_url}
          <div class="src">
            <span class="s-what">{t("showcase.recoverAuthor")}</span>
            <button class="btn" type="button" onclick={() => visit(sources!.author_url!)}>{t("showcase.open")}</button>
          </div>
        {/if}
        {#if sources.source_site}
          <div class="src">
            <span class="s-what">{t("showcase.searchSite", { site: siteHost(sources.source_site) })}</span>
            <button class="btn" type="button" onclick={() => search(sources!.source_site)}>{t("showcase.search")}</button>
          </div>
        {/if}
        <div class="src">
          <span class="s-what">{t("showcase.searchWeb")}</span>
          <button class="btn" type="button" onclick={() => search(null)}>{t("showcase.search")}</button>
        </div>
        {#if sources.attached.length}
          <!-- ESPACE§7.5: its layers, skins and sounds come back with their
               own archives, never with the mod's — named here with them. -->
          <div class="attached">
            <span>{t("showcase.recoverAttached")}</span>
            {#each sources.attached as a (a.kind + a.name)}
              <span class="a-row">{a.name}{#if a.archive} <span class="mono file">{a.archive}</span>{/if}</span>
            {/each}
          </div>
        {/if}
        <div class="foot">
          {#if sources.file_name}
            <span>{t("showcase.fileName")}</span>
            <span class="mono file">{sources.file_name}</span>
          {/if}
          <span>{t("showcase.dropHint")}</span>
        </div>
      {/if}
      {#if error}<span class="note err">{error}</span>{/if}
    </div>
  {/if}
</div>

<style>
  .vit {
    margin: 10px 18px 0;
    padding: 8px 10px;
    background: var(--panel2);
    border: 1px solid var(--line);
    font-size: 11.5px;
  }
  .line {
    display: flex;
    align-items: center;
    gap: 10px;
  }
  .what {
    flex: 1;
    min-width: 0;
    color: var(--txt2);
    line-height: 1.45;
  }
  .panel {
    margin-top: 8px;
    padding-top: 8px;
    border-top: 1px solid var(--line);
    display: flex;
    flex-direction: column;
    gap: 6px;
  }
  .src {
    display: flex;
    align-items: center;
    gap: 10px;
  }
  .s-what {
    flex: 1;
    min-width: 0;
    color: var(--txt2);
  }
  .ver {
    margin-left: 6px;
    color: var(--muted);
  }
  .attached {
    display: flex;
    flex-direction: column;
    gap: 2px;
    margin-top: 4px;
    color: var(--muted);
  }
  .a-row {
    color: var(--txt2);
  }
  .foot {
    display: flex;
    flex-wrap: wrap;
    gap: 6px;
    margin-top: 4px;
    color: var(--muted);
  }
  /* The name is the search key: it must be selectable, whole. */
  .file {
    color: var(--txt2);
    user-select: all;
    overflow-wrap: anywhere;
  }
  .note {
    color: var(--muted);
  }
  .note.err {
    color: var(--rosso-bright);
  }
</style>
