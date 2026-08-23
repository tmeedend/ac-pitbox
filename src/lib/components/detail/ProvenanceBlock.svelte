<script lang="ts">
  // Bloc « Source / origine » de la fiche détail (§4.4) : d'où vient ce mod.
  //
  // Plusieurs rubriques ont fusionné ici — auteur, archive, date de publication,
  // pack d'origine — parce qu'elles répondent toutes à la même question « d'où
  // vient ce mod ». Le sous-titre « Provenance du mod » qui doublait le titre a
  // disparu : deux en-têtes empilés pour un seul contenu n'apportaient rien.
  // L'auteur n'avait sa propre carte que côté circuit ; il n'était nulle part
  // côté voiture — les deux affichent désormais la même ligne.
  //
  // Présentationnel : les trois actions (filtrer par pack, ouvrir une entité
  // sœur, désinstaller le pack) sont déléguées au parent, qui possède la
  // navigation, la bannière d'erreur et la fermeture de la fiche. Désinstaller
  // un pack ferme la page — ça ne peut pas se décider ici.
  import type { ModCard, ModDetail } from "$lib/library";
  import { previewSrc } from "$lib/library";
  import { t } from "$lib/i18n/index.svelte";

  let {
    detail,
    siblings,
    busy,
    onfilterbypack,
    onopenpack,
    onopensibling,
    onuninstallpack,
  }: {
    detail: ModDetail;
    /** Autres entités du même pack (le mod courant exclu). */
    siblings: ModCard[];
    /** Désinstallation en cours côté parent. */
    busy: boolean;
    onfilterbypack: () => void;
    /** Ouvrir la fiche du pack (§4.4) — la seule vue où ce que le pack pose
     * dans le jeu est visible : ces fichiers n'appartiennent à aucun de ses
     * mods, donc aucune fiche de mod ne les montre. */
    onopenpack: () => void;
    onopensibling: (sibling: ModCard) => void;
    onuninstallpack: () => void;
  } = $props();

  /** Archive dont provient la version **active** (§4.2) — mod importé
   * uniquement. Pour le contenu de base Kunos (§10bis), il n'y a jamais eu
   * d'archive : jeu de base ou DLC (`detail.stock_pack`, résolu côté Rust
   * depuis `docs/kunos_content_dates.json`), jamais les deux à la fois.
   *
   * Un mod installé hors Pit Box (§12bis.1bis) partage `is_stock` sans être du
   * contenu de jeu : il ne doit surtout pas hériter du « Jeu de base » par
   * défaut ci-dessous — c'est justement le mensonge que cette distinction est
   * venue corriger. On ne sait pas d'où il vient, et on le dit. */
  const provenance = $derived(
    detail.is_unmanaged
      ? t("detail.unmanagedOrigin")
      : detail.is_stock
        ? (detail.stock_pack ?? t("detail.baseGameLabel"))
        : (detail.versions.find((v) => v.id === detail.active_version_id)?.source_archive ?? null),
  );

  /** Date seule : l'heure d'une date estimée n'a aucun sens (§6.2). */
  function fmtDay(iso: string): string {
    const d = new Date(iso);
    return Number.isNaN(d.getTime()) ? iso.slice(0, 10) : d.toLocaleDateString();
  }
</script>

<section class="blk">
  <header class="blk-h">
    <span class="blk-t">{t("detail.sourceLabel")}</span>
  </header>
  <div class="blk-b">
    <dl class="rows">
      <div class="row">
        <dt>{t("detail.authorLabel")}</dt>
        <dd class="mono">{detail.author ?? "—"}</dd>
      </div>
      <div class="row">
        <dt>{t("detail.provenanceLabel")}</dt>
        <dd class="mono">{provenance ?? "—"}</dd>
      </div>
      <div class="row">
        <dt>{t("detail.publishedLabel")}</dt>
        <dd class="mono">
          {#if detail.published_at}
            {fmtDay(detail.published_at)} <span class="hint">({t("detail.estimated")})</span>
          {:else}
            —
          {/if}
        </dd>
      </div>
      <div class="row">
        <dt>{t("detail.originUrlLabel")}</dt>
        <dd class="mono">
          {#if detail.source_url}
            <span class="url">{detail.source_url}</span>
          {:else}
            <span class="hint">{t("detail.noUrl")}</span>
          {/if}
        </dd>
      </div>
    </dl>

    {#if detail.source_pack}
      <div class="pack">
        <div class="blk-sub">{t("detail.siblingsLabel", { count: siblings.length })}</div>
        {#if siblings.length}
          <div class="siblings">
            {#each siblings as c (c.id_interne)}
              {@const img = previewSrc(c.preview)}
              <button class="sib" type="button" onclick={() => onopensibling(c)} title={t("detail.openSheetTooltip")}>
                <span class="sib-img">
                  {#if img}<img src={img} alt="" loading="lazy" />{:else}<span class="sib-none">{c.kind === "Track" ? "🏁" : "🚗"}</span>{/if}
                </span>
                <span class="sib-nm">{c.display_name ?? c.id_interne}</span>
              </button>
            {/each}
          </div>
        {:else}
          <div class="hint">{t("detail.onlyEntity")}</div>
        {/if}
        <div class="actions">
          <button class="btn" type="button" onclick={onopenpack}>{t("detail.openPack")}</button>
          <button class="btn" type="button" onclick={onfilterbypack}>{t("detail.filterByPack")}</button>
          <button class="btn danger" type="button" onclick={onuninstallpack} disabled={busy}>
            {busy ? t("common.working") : t("detail.uninstallPack")}
          </button>
        </div>
      </div>
    {/if}
  </div>
</section>

<style>
  /* Habillage propre au bloc. L'encadré, le bandeau et la sous-rubrique
     viennent des classes globales `.blk*` (voir global.css). */
  .rows {
    display: flex;
    flex-direction: column;
    gap: 10px;
  }
  /* Clé à gauche, valeur alignée à droite : les valeurs (noms d'archive,
     dates) se comparent d'un coup d'œil quand elles partagent un bord. */
  .row {
    display: flex;
    align-items: baseline;
    justify-content: space-between;
    gap: 16px;
  }
  .row dt {
    color: var(--muted);
    font-size: 12px;
    flex: none;
  }
  .row dd {
    color: var(--txt2);
    font-size: 11.5px;
    text-align: right;
    min-width: 0;
    overflow-wrap: anywhere;
  }
  .hint {
    color: var(--muted2);
    font-size: 11px;
    font-style: italic;
  }
  .url {
    color: var(--blue);
  }
  .pack {
    margin-top: 20px;
    padding-top: 16px;
    border-top: 1px solid var(--line);
  }
  .siblings {
    display: grid;
    grid-template-columns: repeat(auto-fill, minmax(120px, 1fr));
    gap: 10px;
  }
  .sib {
    background: transparent;
    padding: 0;
    text-align: left;
  }
  .sib-img {
    display: flex;
    align-items: center;
    justify-content: center;
    aspect-ratio: 16 / 9;
    background: var(--bg);
    border: 1px solid var(--line);
    overflow: hidden;
  }
  .sib-img img {
    width: 100%;
    height: 100%;
    object-fit: cover;
  }
  .sib:hover .sib-img {
    border-color: var(--rosso-border);
  }
  .sib-none {
    font-size: 18px;
  }
  .sib-nm {
    display: block;
    margin-top: 5px;
    font-size: 11px;
    color: var(--txt2);
    white-space: nowrap;
    overflow: hidden;
    text-overflow: ellipsis;
  }
  .sib:hover .sib-nm {
    color: var(--rosso-bright);
  }
  .actions {
    display: grid;
    grid-template-columns: 1fr 1fr;
    gap: 10px;
    margin-top: 16px;
  }
  .actions .btn {
    justify-content: center;
    text-align: center;
    font-family: var(--mono);
    font-size: 11px;
    letter-spacing: 1px;
    text-transform: uppercase;
    padding: 11px;
  }
  .btn.danger:hover {
    border-color: var(--rosso-border);
    color: var(--rosso-bright);
  }
</style>
