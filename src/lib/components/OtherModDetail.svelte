<script lang="ts">
  // Fiche d'un « autre mod » (§7.3).
  //
  // Même raison d'être que la fiche d'un mod de son (`SoundDetail.svelte`) :
  // ses annexes (notice, images d'un mannequin de pilote nu, par exemple)
  // n'avaient nulle part où vivre — la liste plate d'`OtherMods.svelte` n'a
  // que des actions de ligne, pas d'espace pour un `ResourcesBlock`. Rien
  // n'est réinventé : `StateBadge` et `ResourcesBlock` sont ceux de la fiche
  // voiture et du son ; les actions (activer, prioritaire, dossier,
  // supprimer) sont celles déjà écrites dans `OtherMods.svelte`, reçues en
  // props plutôt que réimplémentées ici.
  import { t } from "$lib/i18n/index.svelte";
  import { type OtherModRow } from "$lib/others";
  import ResourcesBlock from "./detail/ResourcesBlock.svelte";
  import StateBadge from "./StateBadge.svelte";

  interface Props {
    row: OtherModRow;
    busy: boolean;
    warnings: string[];
    onclose: () => void;
    ontoggle: () => void;
    ontogglePriority: () => void;
    onopenFolder: () => void;
    ondelete: () => void;
  }

  const { row, busy, warnings, onclose, ontoggle, ontogglePriority, onopenFolder, ondelete }: Props = $props();

  let error = $state("");
</script>

<div class="page">
  <header class="head">
    <button class="back" type="button" onclick={onclose}>{t("others.back")}</button>
    <h2 class="lbl-screen mono">{row.id}</h2>
    <StateBadge active={row.is_active} stock={false} />
    <div class="actions">
      <button class="btn" type="button" onclick={onopenFolder} title={t("others.openFolder")}>
        {t("others.openFolder")}
      </button>
      <button
        class="btn prio"
        class:on={row.is_priority}
        type="button"
        onclick={ontogglePriority}
        disabled={busy}
        title={t("others.priorityTooltip")}
      >
        {t("others.priority")}
      </button>
      <button class="btn" type="button" onclick={ontoggle} disabled={busy}>
        {busy ? t("common.working") : row.is_active ? t("common.deactivate") : t("common.activate")}
      </button>
      <button class="btn del" type="button" onclick={ondelete} disabled={busy}>{t("common.delete")}</button>
    </div>
  </header>

  {#if error}<div class="err">{error}</div>{/if}

  <dl class="meta">
    <div>
      <dt class="lbl-key">{t("others.categoriesLabel")}</dt>
      <dd class="cats">
        {#each row.categories as c}<span class="cat">{t(`others.cat.${c}`)}</span>{/each}
      </dd>
    </div>
    {#if row.source_archive}
      <div>
        <dt class="lbl-key">{t("detail.sourceLabel")}</dt>
        <dd class="mono">{row.source_archive}</dd>
      </div>
    {/if}
    <div>
      <dt class="lbl-key">{t("apps.importedAt")}</dt>
      <dd>{new Date(row.imported_at).toLocaleString()}</dd>
    </div>
    {#if row.externally_managed}
      <div>
        <dt class="lbl-key">{t("others.managed", { count: row.externally_managed })}</dt>
        <dd class="managed" title={t("others.managedTooltip")}>{row.externally_managed}</dd>
      </div>
    {/if}
  </dl>

  {#if row.conflicts.length}
    <div class="conflicts">
      {t("others.conflictsWith")}
      {#each row.conflicts as c, i}{i > 0 ? ", " : ""}<b>{c.other_id}</b> ({c.count}){/each}
    </div>
  {/if}

  {#if warnings.length}
    <ul class="warn-list">
      {#each warnings as w}<li>{w}</li>{/each}
    </ul>
  {/if}

  <div class="body">
    <ResourcesBlock modId={row.id} source="other" onerror={(m) => (error = m)} />
  </div>
</div>

<style>
  .page {
    max-width: 860px;
  }
  .head {
    display: flex;
    align-items: center;
    gap: 12px;
    flex-wrap: wrap;
    margin-bottom: 12px;
  }
  .head h2 {
    flex: 1;
    min-width: 0;
    overflow-wrap: anywhere;
  }
  .back {
    background: none;
    border: none;
    padding: 0;
    font: inherit;
    font-size: 11.5px;
    color: var(--muted);
    cursor: pointer;
  }
  .back:hover,
  .back:focus-visible {
    color: var(--rosso-bright);
  }
  .actions {
    display: flex;
    gap: 6px;
  }
  .btn {
    background: var(--raised);
    color: var(--txt2);
    border: 1px solid var(--line);
    font-size: 11px;
    padding: 6px 12px;
    flex: none;
    cursor: pointer;
  }
  .btn:hover {
    border-color: var(--rosso-border);
    color: var(--rosso-bright);
  }
  .btn:disabled {
    opacity: 0.5;
  }
  .btn.prio {
    color: var(--muted);
  }
  .btn.prio.on {
    color: var(--rosso-bright);
    border-color: var(--rosso-border);
  }
  .btn.del {
    color: var(--muted);
  }
  .btn.del:hover {
    border-color: var(--rosso-border);
    color: var(--rosso-bright);
  }
  .meta {
    display: flex;
    flex-wrap: wrap;
    gap: 4px 28px;
    margin-bottom: 14px;
  }
  .meta dd {
    font-size: 12px;
    color: var(--txt2);
    margin-top: 2px;
    overflow-wrap: anywhere;
  }
  .cats {
    display: flex;
    gap: 4px;
    flex-wrap: wrap;
  }
  .cat {
    font-size: 9.5px;
    text-transform: uppercase;
    letter-spacing: 0.5px;
    color: var(--muted2);
    border: 1px solid var(--line);
    padding: 1px 6px;
    white-space: nowrap;
  }
  .managed {
    color: var(--blue);
  }
  .conflicts {
    margin-bottom: 14px;
    font-size: 11px;
    color: var(--yellow);
  }
  .warn-list {
    margin-bottom: 14px;
    padding-left: 16px;
    font-size: 11px;
    color: var(--muted);
  }
  .body {
    margin-top: 14px;
  }
  .err {
    margin-bottom: 10px;
    padding: 8px 10px;
    border: 1px solid var(--rosso-border);
    background: var(--rosso-dim);
    color: var(--rosso-bright);
    font-size: 11.5px;
  }
</style>
