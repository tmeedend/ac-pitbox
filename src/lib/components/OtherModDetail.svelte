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
  import { bodyThumb, requestBodyThumb } from "$lib/driverThumbs.svelte";
  import { nav } from "$lib/nav.svelte";

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

  // --- Mannequin de pilote : montrer de quoi il a l'air ---------------------
  //
  // Un mannequin ne se reconnaît qu'à sa géométrie — casque, HANS, carrure,
  // visage. Sa fiche ne portait que son id de fichier, ce qui ne dit rien de
  // ce qu'on vient d'importer. On réemploie donc telle quelle la galerie de
  // l'écran Pilote (`driverThumbs`), qui rend le corps en 3D et garde le PNG.
  //
  // **Les corps se lisent dans les jonctions posées**, pas dans un champ
  // dédié : AC connaît un mannequin par le nom de fichier qu'il trouve dans
  // `content/driver/`, et c'est exactement ce que l'activation y a déposé.
  // Conséquence assumée : un mod inactif n'a rien à montrer, puisqu'il n'est
  // pas dans le jeu — on le dit plutôt que d'afficher un cadre vide.
  const DRIVER_KN5 = /[\\/]content[\\/]driver[\\/]([^\\/]+)\.kn5$/i;
  const bodies = $derived(
    row.junctions.map((path) => DRIVER_KN5.exec(path)?.[1]).filter((id): id is string => !!id),
  );
  const isDriverMod = $derived(row.categories.includes("driver"));
  /** **C'est la voiture qui pose le mannequin** (`prepare_body_preview`) : sans
   * duo de session, il n'y a pas de pose, donc pas de vignette. */
  const carId = $derived(nav.sessionCar?.id ?? null);
  $effect(() => {
    if (!carId) return;
    const skin = nav.sessionCar?.skin ?? null;
    for (const body of bodies) requestBodyThumb(carId, skin, body);
  });
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

  {#if error}<div class="errbox">{error}</div>{/if}

  {#if isDriverMod}
    <section class="blk">
      <header class="blk-h">
        <span class="blk-t">{t("others.driverTitle")}</span>
        {#if bodies.length > 1}<span class="blk-n">{bodies.length}</span>{/if}
      </header>
      <div class="blk-b">
        {#if !bodies.length}
          <p class="hint">{t("others.driverInactive")}</p>
        {:else if !carId}
          <p class="hint">{t("others.driverNeedsCar")}</p>
        {:else}
          <div class="dolls">
            {#each bodies as body (body)}
              {@const thumb = bodyThumb(carId + "|" + body)}
              <figure>
                {#if thumb}
                  <img src={thumb} alt="" />
                {:else}
                  <div class="doll-wait">{t("common.loading")}</div>
                {/if}
                <figcaption class="mono">{body}</figcaption>
              </figure>
            {/each}
          </div>
        {/if}
      </div>
    </section>
  {/if}

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
  .dolls {
    display: flex;
    flex-wrap: wrap;
    gap: 14px;
  }
  .dolls figure {
    margin: 0;
    width: 104px;
  }
  .dolls img,
  .doll-wait {
    width: 104px;
    height: 104px;
    display: block;
    background: var(--panel2);
    border: 1px solid var(--line);
  }
  .doll-wait {
    display: flex;
    align-items: center;
    justify-content: center;
    color: var(--muted);
    font-size: 10.5px;
  }
  .dolls figcaption {
    margin-top: 5px;
    font-size: 10px;
    color: var(--muted);
    overflow-wrap: anywhere;
  }
  .hint {
    margin: 0;
    color: var(--muted);
    font-size: 11.5px;
    line-height: 1.5;
  }
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
  .errbox {
    margin-bottom: 10px;
  }
</style>
