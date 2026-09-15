<script lang="ts">
  // Fiche d'un « autre mod » (§7.3).
  //
  // Même raison d'être que la fiche d'un mod de son (`SoundDetail.svelte`) :
  // ses annexes (notice, images d'un mannequin de pilote nu, par exemple)
  // n'avaient nulle part où vivre — une liste n'a que des actions de ligne,
  // pas d'espace pour un `ResourcesBlock`. Rien n'est réinventé : `StateBadge`
  // et `ResourcesBlock` sont ceux de la fiche voiture et du son ; les actions
  // (activer, prioritaire, dossier, supprimer) sont reçues en props de
  // l'écran qui ouvre la fiche — `Inventory.svelte` ou `DetailPage.svelte` —
  // plutôt que réimplémentées ici.
  import { t } from "$lib/i18n/index.svelte";
  import { type OtherModRow } from "$lib/others";
  import ResourcesBlock from "./detail/ResourcesBlock.svelte";
  import FicheHeader from "./FicheHeader.svelte";
  import NoteBlock from "./NoteBlock.svelte";
  import { bodyThumb, requestBodyThumb } from "$lib/driverThumbs.svelte";
  import { nav } from "$lib/nav.svelte";
  import { splitProvenance } from "$lib/provenance";

  interface Props {
    row: OtherModRow;
    busy: boolean;
    warnings: string[];
    onclose: () => void;
    ontoggle: () => void;
    ontogglePriority: () => void;
    onopenFolder: () => void;
    ondelete: () => void;
    /** Nom repris à la main (§6.1). `null` = revenir à l'identifiant du mod. */
    onrename: (value: string | null) => void;
    /** Note libre (§9). `null` = effacer. */
    onnote: (value: string | null) => void;
  }

  const { row, busy, warnings, onclose, ontoggle, ontogglePriority, onopenFolder, ondelete, onrename, onnote }: Props =
    $props();

  let error = $state("");

  const provenance = $derived(splitProvenance(row.source_archive));

  /** Les chemins posés, groupés par dossier de destination : c'est le niveau
   * auquel on lit un mod (`content/driver`, `extension/config/cars`), et le
   * compteur y a un sens que le poids d'un fichier n'a pas. */
  const placedTree = $derived.by(() => {
    const map = new Map<string, string[]>();
    for (const p of row.placed) {
      const cut = p.lastIndexOf("/");
      const dir = cut > 0 ? p.slice(0, cut) : "/";
      const name = cut > 0 ? p.slice(cut + 1) : p;
      const list = map.get(dir);
      if (list) list.push(name);
      else map.set(dir, [name]);
    }
    return [...map.entries()]
      .map(([dir, names]) => ({ dir, names: names.sort() }))
      .sort((a, b) => a.dir.localeCompare(b.dir));
  });

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
  <FicheHeader
    onback={onclose}
    backLabel={t("others.back")}
    glyph="⚙"
    name={row.display_name_user ?? row.id}
    subtitle={row.display_name_user ? row.id : undefined}
    rename={{
      original: row.id,
      overridden: !!row.display_name_user,
      onsave: onrename,
    }}
    deployment={{ active: row.is_active }}
    actions={[
      { label: t("others.openFolder"), onclick: onopenFolder },
      { label: t("others.priority"), onclick: ontogglePriority, disabled: busy },
      {
        label: busy ? t("common.working") : row.is_active ? t("common.deactivate") : t("common.activate"),
        onclick: ontoggle,
        disabled: busy,
      },
      { label: t("common.delete"), onclick: ondelete, disabled: busy, danger: true },
    ]}
  />

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

  <NoteBlock value={row.notes_user} onsave={onnote} />

  <dl class="meta">
    <div>
      <dt class="lbl-key">{t("others.categoriesLabel")}</dt>
      <dd class="cats">
        {#each row.categories as c}<span class="cat">{t(`others.cat.${c}`)}</span>{/each}
      </dd>
    </div>
    <!-- Deux lignes et non une (§7.3) : la provenance d'un **reste** est la
         chaîne que l'import a fabriquée pour le nommer, `<archive>__<chemin
         dedans>`. Affichée d'un bloc sous « Provenance », elle se donnait pour
         un nom d'archive sans en être un. Les deux moitiés sont utiles, elles
         ne répondent simplement pas à la même question. -->
    {#if provenance}
      <div>
        <dt class="lbl-key">{t("detail.provenanceLabel")}</dt>
        <dd class="mono">{provenance.archive}</dd>
      </div>
      {#if provenance.inside}
        <div>
          <dt class="lbl-key">{t("others.insideArchive")}</dt>
          <dd class="mono">{provenance.inside}</dd>
        </div>
      {/if}
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

  <!-- Emplacements (§11) : où ce mod atterrit dans le jeu. C'est ce qui
       manquait le plus à la fiche d'une police ou d'un fragment de config —
       elle affichait trois métadonnées et « aucun fichier annexe », alors que
       le parcours de fichiers connaissait déjà la réponse. -->
  <section class="blk">
    <header class="blk-h">
      <span class="blk-t">{t("others.placedTitle")}</span>
      <span class="blk-n">{row.placed.length}</span>
    </header>
    <div class="blk-b">
      {#if !row.placed.length}
        <p class="muted small">{row.is_active ? t("others.placedNothing") : t("others.placedInactive")}</p>
      {:else}
        <ul class="places">
          {#each placedTree as g (g.dir)}
            <li>
              <span class="pdir mono">{g.dir}</span>
              <span class="pn mono">{t("detail.layerFileCount", { count: g.names.length })}</span>
              <ul class="pfiles">
                {#each g.names as n (n)}<li class="mono">{n}</li>{/each}
              </ul>
            </li>
          {/each}
        </ul>
      {/if}
    </div>
  </section>

  <!-- Conflits (§11) : qui d'autre vise les mêmes fichiers, et qui gagne. Le
       décompte seul ne disait pas la seule chose qui décide. -->
  {#if row.conflicts.length}
    <section class="blk">
      <header class="blk-h">
        <span class="blk-t">{t("others.conflictsTitle")}</span>
        <span class="blk-n">{row.conflicts.length}</span>
      </header>
      <div class="blk-b">
        <ul class="confl">
          {#each row.conflicts as c (c.other_id)}
            <li>
              <span class="mono cid">{c.other_id}</span>
              <span class="cn mono">{t("others.conflictFiles", { count: c.count })}</span>
              <span class="cwin" class:mine={row.is_priority}>
                {row.is_priority ? t("others.conflictWinsMine") : t("others.conflictWinsUnknown")}
              </span>
            </li>
          {/each}
        </ul>
        <p class="muted small">{t("others.conflictHint")}</p>
      </div>
    </section>
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
  .places,
  .confl {
    list-style: none;
    display: flex;
    flex-direction: column;
    gap: 8px;
  }
  .pdir {
    color: var(--txt2);
    font-size: 11.5px;
  }
  .pn,
  .cn {
    color: var(--muted2);
    font-size: 10px;
    margin-left: 8px;
  }
  .pfiles {
    list-style: none;
    padding-left: 14px;
    margin-top: 2px;
  }
  .pfiles li {
    color: var(--muted);
    font-size: 10.5px;
    overflow-wrap: anywhere;
  }
  .confl li {
    display: flex;
    align-items: baseline;
    gap: 8px;
  }
  .cid {
    flex: 1;
    min-width: 0;
    color: var(--txt2);
    font-size: 11px;
    overflow-wrap: anywhere;
  }
  /* Qui gagne : rouge quand c'est CE mod, parce que c'est lui qui remplace
     quelque chose — le barème du §7.2ter, pas une alerte. */
  .cwin {
    flex: none;
    font-size: 10px;
    color: var(--muted2);
  }
  .cwin.mine {
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
