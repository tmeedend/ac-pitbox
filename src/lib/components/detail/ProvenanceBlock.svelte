<script lang="ts">
  // Bloc « Provenance » de la fiche détail (§4.7) : d'où vient ce mod — pack
  // d'origine, archive source, URL — et les autres entités du même pack.
  //
  // Présentationnel : les trois actions (filtrer par pack, ouvrir une entité
  // sœur, désinstaller le pack) sont déléguées au parent, qui possède la
  // navigation, la bannière d'erreur et la fermeture de la fiche. Désinstaller
  // un pack ferme la page — ça ne peut pas se décider ici.
  import type { ModCard, ModDetail } from "$lib/library";
  import { t } from "$lib/i18n/index.svelte";

  let {
    detail,
    siblings,
    busy,
    onfilterbypack,
    onopensibling,
    onuninstallpack,
  }: {
    detail: ModDetail;
    /** Autres entités du même pack (le mod courant exclu). */
    siblings: ModCard[];
    /** Désinstallation en cours côté parent. */
    busy: boolean;
    onfilterbypack: () => void;
    onopensibling: (sibling: ModCard) => void;
    onuninstallpack: () => void;
  } = $props();

  /** Archive dont provient la version **active** (§4.2). */
  const archive = $derived(
    detail.versions.find((v) => v.id === detail.active_version_id)?.source_archive ?? null,
  );
</script>

{#if detail.source_pack || archive || detail.source_url}
  <div class="lbl section">{t("detail.sourceLabel")}</div>
  <div class="srcbox">
    <div class="src-h">{t("detail.provenanceTitle")}</div>
    {#if detail.source_pack}
      <div class="srcrow">
        <span class="src-k">{t("detail.packLabel")}</span>
        <button class="chip" type="button" onclick={onfilterbypack} title={t("detail.viewPackTooltip")}>
          ⬢ {detail.source_pack}
          <span class="chip-n">· {t("detail.modCount", { count: siblings.length + 1 })}</span>
        </button>
      </div>
    {/if}
    <div class="srcrow">
      <span class="src-k">{t("detail.archiveLabel")}</span>
      <span class="src-v">{archive ?? "—"}</span>
    </div>
    <div class="srcrow">
      <span class="src-k">{t("detail.originUrlLabel")}</span>
      {#if detail.source_url}
        <span class="src-v url">{detail.source_url}</span>
      {:else}
        <span class="src-empty">{t("detail.noUrl")}</span>
      {/if}
    </div>
  </div>

  {#if detail.source_pack}
    <div class="lbl section">{t("detail.siblingsLabel", { count: siblings.length })}</div>
    {#if siblings.length}
      <div class="siblings">
        {#each siblings as c (c.id_interne)}
          <button class="sib" type="button" onclick={() => onopensibling(c)} title={t("detail.openSheetTooltip")}>
            <span class="sib-dot">{c.kind === "Track" ? "🏁" : "🚗"}</span>
            <span class="sib-nm">{c.display_name ?? c.id_interne}</span>
          </button>
        {/each}
      </div>
    {:else}
      <div class="muted small">{t("detail.onlyEntity")}</div>
    {/if}
    <div class="prov-note">{t("detail.packNote")}</div>
    <div class="prov-actions">
      <button class="btn" type="button" onclick={onfilterbypack}>⌕ {t("detail.filterByPack")}</button>
      <button class="btn danger" type="button" onclick={onuninstallpack} disabled={busy}>
        {busy ? t("common.working") : `🗑 ${t("detail.uninstallPack")}`}
      </button>
    </div>
  {/if}
{/if}

<style>
  /* Styles repris de la fiche : le CSS Svelte étant scopé par composant, un
     bloc extrait doit emporter les siens (voir l'en-tête de global.css). */
  .lbl {
    color: var(--faint);
    font-size: 9px;
    letter-spacing: 1.5px;
    margin-bottom: 8px;
    display: flex;
    align-items: center;
    text-transform: uppercase;
  }
  .lbl.section {
    margin-top: 14px;
  }
  .muted {
    color: var(--muted);
  }
  .small {
    font-size: 11px;
  }
  .srcbox {
    border: 1px solid var(--line);
  }
  .src-h {
    background: var(--raised);
    padding: 5px 10px;
    border-bottom: 1px solid var(--line);
    color: var(--muted);
    font-size: 8px;
    letter-spacing: 1.5px;
  }
  .srcrow {
    display: flex;
    align-items: center;
    gap: 9px;
    padding: 8px 11px;
    border-bottom: 1px solid var(--line);
  }
  /* Sans ça, l'encadré affiche un double filet en bas (sa bordure + celle de
     la dernière ligne). */
  .srcrow:last-child {
    border-bottom: none;
  }
  .src-k {
    color: var(--faint);
    font-size: 8px;
    letter-spacing: 1px;
    width: 84px;
    flex-shrink: 0;
  }
  .src-v {
    font-size: 10.5px;
    font-family: var(--mono);
    color: var(--txt2);
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }
  .src-v.url {
    color: var(--blue);
  }
  .src-empty {
    color: var(--muted2);
    font-size: 9.5px;
    font-family: var(--mono);
    font-style: italic;
  }
  .chip {
    display: inline-flex;
    align-items: center;
    gap: 5px;
    background: var(--rosso-dim);
    border: 1px solid var(--rosso-border);
    color: var(--rosso-bright);
    font-size: 10px;
    font-family: var(--mono);
    padding: 3px 9px;
  }
  .chip .chip-n {
    color: var(--muted);
  }
  .siblings {
    display: grid;
    grid-template-columns: 1fr 1fr;
    gap: 1px;
    background: var(--line);
    border: 1px solid var(--line);
  }
  .sib {
    background: var(--card);
    padding: 7px 9px;
    display: flex;
    align-items: center;
    gap: 7px;
    text-align: left;
  }
  .sib:hover {
    background: var(--raised);
  }
  .sib-dot {
    font-size: 13px;
    flex: none;
  }
  .sib-nm {
    font-size: 9.5px;
    color: var(--txt2);
    white-space: nowrap;
    overflow: hidden;
    text-overflow: ellipsis;
  }
  .prov-note {
    margin-top: 8px;
    background: var(--blue-dim);
    border: 1px solid var(--blue-border);
    color: var(--blue);
    font-size: 9px;
    font-family: var(--mono);
    padding: 6px 9px;
  }
  .prov-actions {
    display: flex;
    gap: 7px;
    margin-top: 10px;
  }
  .btn.danger {
    color: var(--muted);
  }
  .btn.danger:hover {
    border-color: var(--rosso-border);
    color: var(--rosso-bright);
  }
</style>
