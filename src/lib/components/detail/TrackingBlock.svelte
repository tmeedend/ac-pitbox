<script lang="ts">
  // Bloc « suivi » de la fiche détail : versions, historique, date de
  // publication (§3.2 / §6.2).
  //
  // Les trois sont réunis dans un seul composant parce qu'ils forment un même
  // panneau à l'écran et lisent tous `ModDetail` sans rien charger de plus —
  // les séparer produirait trois fichiers dont deux de quelques lignes, chacun
  // devant recopier les mêmes styles.
  //
  // Purement présentationnel, à une exception près : activer une version est
  // une action réelle, déléguée au parent (qui possède déjà `busy`, la
  // relecture de la fiche et la bannière d'erreur).
  import type { ModDetail } from "$lib/library";
  import { historyEventLabel, historyDetails } from "$lib/history";
  import { t } from "$lib/i18n/index.svelte";

  let {
    detail,
    busy,
    onactivateversion,
  }: {
    detail: ModDetail;
    /** Une action est déjà en cours côté parent : les boutons se désactivent. */
    busy: boolean;
    onactivateversion: (versionId: string) => void;
  } = $props();

  /** Date+heure locales ; repli sur l'ISO tronqué si la chaîne est illisible. */
  function fmtDate(iso: string): string {
    const d = new Date(iso);
    return Number.isNaN(d.getTime()) ? iso.slice(0, 16).replace("T", " ") : d.toLocaleString();
  }

  // Activer/désactiver n'est pas un événement de cycle de vie du mod : ces
  // lignes pollueraient l'historique sans rien apprendre (§3.2).
  const events = $derived(detail.history.filter((h) => h.event !== "ACTIVATE" && h.event !== "DEACTIVATE"));
</script>

<div class="lbl section">{t("detail.versionsLabel", { count: detail.versions.length })}</div>
{#each detail.versions as v}
  <div class="ver" class:active={v.id === detail.active_version_id}>
    <span class="v-label mono">{v.version_label ?? t("detail.noVersionNumber")}</span>
    {#if v.id === detail.active_version_id}
      <span class="tag cat tiny">{t("common.active").toUpperCase()}</span>
    {:else}
      <button class="v-activate" type="button" onclick={() => onactivateversion(v.id)} disabled={busy}>
        {t("common.activate")}
      </button>
    {/if}
    <span class="v-meta mono">{fmtDate(v.imported_at)}</span>
  </div>
{/each}

<div class="lbl section">{t("detail.historyLabel")}</div>
<ul class="history">
  {#each events as h}
    <li>
      <span class="ev">{historyEventLabel(h.event)}</span>
      <span class="det">{historyDetails(h.details)}</span>
      <span class="ts mono">{fmtDate(h.timestamp)}</span>
    </li>
  {/each}
</ul>

<div class="lbl section">{t("detail.publishedLabel")}</div>
<div class="srcbox">
  <div class="srcrow">
    <span class="src-k">{t("detail.estimated")}</span>
    <span class="src-v">{detail.published_at ? fmtDate(detail.published_at) : "—"}</span>
  </div>
</div>

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
  .ver {
    display: flex;
    align-items: center;
    gap: 6px;
    border: 1px solid var(--line);
    background: var(--panel2);
    padding: 6px 10px;
    margin-bottom: 5px;
  }
  .ver.active {
    border-left: 3px solid var(--rosso);
  }
  .v-label {
    font-size: 10px;
    font-weight: 600;
  }
  .v-activate {
    background: var(--raised);
    border: 1px solid var(--line);
    color: var(--txt2);
    font-size: 9px;
    padding: 2px 7px;
  }
  .v-activate:hover {
    border-color: var(--rosso-border);
    color: var(--rosso-bright);
  }
  .v-meta {
    margin-left: auto;
    color: var(--faint);
    font-size: 9px;
  }
  .tag {
    font-size: 10px;
    padding: 2px 8px;
    font-family: var(--mono);
    border: 1px solid var(--line);
  }
  .tag.tiny {
    font-size: 7px;
    padding: 0 5px;
  }
  .tag.cat {
    background: var(--rosso-dim);
    color: var(--rosso-bright);
    border-color: var(--rosso-border);
  }
  .history {
    list-style: none;
    display: flex;
    flex-direction: column;
    gap: 5px;
  }
  .history li {
    display: flex;
    flex-direction: column;
    font-size: 11px;
    border-left: 2px solid var(--line);
    padding-left: 8px;
  }
  .history .ev {
    color: var(--rosso-bright);
    font-weight: 600;
    font-size: 9px;
    letter-spacing: 0.5px;
  }
  .history .det {
    color: var(--txt2);
  }
  .history .ts {
    color: var(--muted2);
    font-size: 9px;
  }
  .srcbox {
    border: 1px solid var(--line);
  }
  .srcrow {
    display: flex;
    align-items: center;
    gap: 9px;
    padding: 8px 11px;
    border-bottom: 1px solid var(--line);
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
</style>
