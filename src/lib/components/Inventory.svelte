<script lang="ts">
  // Écran « Compléments » : l'inventaire (refonte §4).
  //
  // Tout ce qui n'est pas un contenu autonome, dans une seule liste. Trois
  // écrans le précédaient — Add-ons voiture, Add-ons circuit, et un
  // fourre-tout — qui classaient par **mécanique d'installation**, c'est-à-dire
  // par la complexité que l'app existe pour absorber.
  //
  // **Cet écran n'organise pas, il inventorie** : une ligne par chose, et les
  // facettes font le tri. C'est ce qui permet de n'y rien perdre, y compris ce
  // que Pit Box n'a pas su reconnaître.
  import { onMount } from "svelte";
  import { t } from "$lib/i18n/index.svelte";
  import { errorText } from "$lib/errors";
  import { fmtSize } from "$lib/format";
  import { listInventory, type InventoryRow } from "$lib/inventory";
  import { layerDisplayName } from "$lib/layerName";
  import { nav, requestSection } from "$lib/nav.svelte";
  import LoadingState from "./LoadingState.svelte";
  import Seg from "./Seg.svelte";
  import StateBadge from "./StateBadge.svelte";
  import ContextMenu from "./ContextMenu.svelte";
  import OtherModDetail from "./OtherModDetail.svelte";
  import SoundDetail from "./SoundDetail.svelte";
  import { confirm } from "@tauri-apps/plugin-dialog";
  import {
    listOtherMods,
    activateOther,
    deactivateOther,
    deleteOtherMod,
    openOtherModFolder,
    setOtherPriority,
    setOtherAttachment,
    type OtherModRow,
  } from "$lib/others";
  import { setEntityDisplayName, setEntityNote } from "$lib/userMeta";

  let rows = $state<InventoryRow[]>([]);
  let loading = $state(true);
  let error = $state("");
  let query = $state("");
  type GroupBy = "none" | "archive" | "host";
  type SortBy = "name" | "size" | "import";
  let groupBy = $state<GroupBy>("none");
  let sortBy = $state<SortBy>("name");

  /** Facettes tri-état : une valeur absente ne filtre pas, `1` inclut, `-1`
   * exclut. Même polarité que les filtres de la bibliothèque — « sauf les
   * dépendances » est une question aussi fréquente que « seulement elles ». */
  let facets = $state<Record<string, 1 | -1>>({});
  let busy = $state<string | null>(null);
  let menu = $state<{ x: number; y: number; row: InventoryRow } | null>(null);
  /** Fiche ouverte par-dessus l'inventaire. Un mod « autre » et un son en ont
   * une ; une livrée, un habillage et une couche vivent sur la fiche de leur
   * hôte, où le lien de rattachement mène. */
  let fullOther = $state<OtherModRow | null>(null);
  let fullSound = $state<string | null>(null);
  /** Les lignes complètes des mods « autres », chargées à la demande : elles
   * coûtent un second parcours de fichiers, qu'on ne paie qu'en ouvrant une
   * fiche. */
  let otherRows = $state<OtherModRow[] | null>(null);

  function cycle(key: string) {
    const next = { ...facets };
    if (next[key] === undefined) next[key] = 1;
    else if (next[key] === 1) next[key] = -1;
    else delete next[key];
    facets = next;
  }

  async function load() {
    loading = true;
    try {
      rows = await listInventory();
      error = "";
    } catch (e) {
      error = errorText(e);
    } finally {
      loading = false;
    }
  }
  onMount(load);

  /** Nom affiché : une couche porte un nom d'archive, qui se dérive comme sur
   * sa fiche — la même fonction, pour que les deux ne divergent pas. */
  function nameOf(r: InventoryRow): string {
    if (r.kind === "LAYER" && r.name === r.tech_id) return layerDisplayName(r.name, r.attachment.target_name);
    return r.name;
  }

  /** Les trois axes de facette d'une ligne, sous forme de clés. */
  function keysOf(r: InventoryRow): string[] {
    const out = [`attach:${r.attachment.kind}`, `nature:${r.attachment.nature}`];
    // Une livrée n'a pas d'état de déploiement (voir `active` côté Rust) :
    // elle ne compte donc ni dans « Actif » ni dans « Inactif ». Lui en
    // inventer un ferait mentir les deux compteurs à la fois.
    if (r.active !== null) out.push(r.active ? "state:ACTIVE" : "state:INACTIVE");
    if (r.has_note) out.push("state:NOTE");
    return out;
  }

  const searched = $derived(
    rows.filter((r) => {
      if (!query.trim()) return true;
      const terms = query.toLowerCase().split(/\s+/).filter(Boolean);
      const hay = `${nameOf(r)} ${r.tech_id} ${r.source_archive ?? ""} ${r.attachment.target_name ?? ""}`.toLowerCase();
      return terms.every((term) => hay.includes(term));
    }),
  );

  const filtered = $derived(
    searched.filter((r) => {
      const mine = keysOf(r);
      const on = Object.entries(facets);
      // Exclusions d'abord, et elles gagnent toujours : « sans les
      // dépendances » ne doit pas laisser passer une ligne qui l'est aussi.
      if (on.some(([k, sign]) => sign === -1 && mine.includes(k))) return false;
      const includes = on.filter(([, sign]) => sign === 1).map(([k]) => k);
      if (!includes.length) return true;
      // Les inclusions d'un MÊME axe sont un OU (voiture ou circuit), celles
      // d'axes différents un ET (une voiture ET de l'apparence).
      const axes = new Set(includes.map((k) => k.split(":")[0]));
      return [...axes].every((axis) => includes.filter((k) => k.startsWith(`${axis}:`)).some((k) => mine.includes(k)));
    }),
  );

  /** Décompte par valeur de facette, calculé sur la recherche et **pas** sur le
   * résultat filtré : un chiffre qui bouge à chaque facette posée ne sert à
   * rien pour décider de la suivante (même règle que la bibliothèque). */
  const counts = $derived.by(() => {
    const map = new Map<string, number>();
    for (const r of searched) for (const k of keysOf(r)) map.set(k, (map.get(k) ?? 0) + 1);
    return map;
  });

  const sorted = $derived.by(() => {
    const list = [...filtered];
    if (sortBy === "size") list.sort((a, b) => b.size_bytes - a.size_bytes);
    else if (sortBy === "import") list.sort((a, b) => b.imported_at.localeCompare(a.imported_at));
    else list.sort((a, b) => nameOf(a).localeCompare(nameOf(b), undefined, { sensitivity: "base" }));
    return list;
  });

  /** Groupes affichés. « Aucun » rend un seul groupe sans en-tête : la liste
   * reste un seul `{#each}`, et le rendu n'a pas deux formes à maintenir. */
  const groups = $derived.by(() => {
    if (groupBy === "none") return [{ key: "", label: "", rows: sorted }];
    const map = new Map<string, InventoryRow[]>();
    for (const r of sorted) {
      const key =
        groupBy === "archive"
          ? (r.source_archive ?? t("inventory.noArchive"))
          : (r.attachment.target_name ?? r.attachment.target_id ?? t("inventory.attachGAME"));
      const list = map.get(key);
      if (list) list.push(r);
      else map.set(key, [r]);
    }
    return [...map.entries()]
      .sort((a, b) => a[0].localeCompare(b[0]))
      .map(([key, list]) => ({ key, label: key, rows: list }));
  });

  async function openFiche(r: InventoryRow) {
    if (r.kind === "SOUND") {
      fullSound = r.id;
      return;
    }
    if (r.kind !== "OTHER") {
      await openHost(r);
      return;
    }
    if (!otherRows) {
      try {
        otherRows = await listOtherMods();
      } catch (e) {
        error = errorText(e);
        return;
      }
    }
    fullOther = otherRows.find((o) => o.id === r.id) ?? null;
  }

  /** Enveloppe commune des actions : état occupé, erreur remontée, relecture.
   * La liste tient les états (actif, prioritaire) : elle doit se relire, sinon
   * l'écran ment jusqu'au prochain changement d'écran. */
  async function run(id: string, action: () => Promise<unknown>) {
    busy = id;
    error = "";
    try {
      await action();
      otherRows = null;
      await load();
    } catch (e) {
      error = errorText(e);
    } finally {
      busy = null;
    }
  }

  async function removeRow(r: InventoryRow) {
    const ok = await confirm(t("others.confirmDelete", { id: r.tech_id }), {
      title: t("common.delete"),
      kind: "warning",
    });
    if (!ok) return;
    await run(r.id, () => deleteOtherMod(r.id));
  }

  /** Actions d'une ligne (§4.2) : **aucun bouton exposé**. Ouvrir le dossier,
   * priorité, désactiver, supprimer passent tous par le ⋮ — ce qui reste
   * visible sur la ligne est ce qui se *lit*, pas ce qui se clique. */
  function menuItems(r: InventoryRow) {
    const items: { label: string; onclick: () => void; disabled?: boolean; danger?: boolean }[] = [];
    if (r.kind === "OTHER" || r.kind === "SOUND") {
      items.push({ label: t("inventory.openFiche"), onclick: () => void openFiche(r) });
    }
    if (r.attachment.target_id) {
      items.push({ label: t("inventory.openHost"), onclick: () => void openHost(r) });
    }
    if (r.kind === "OTHER") {
      items.push(
        { label: t("others.openFolder"), onclick: () => void openOtherModFolder(r.id) },
        {
          label: r.priority ? t("inventory.priorityOff") : t("inventory.priorityOn"),
          onclick: () => void run(r.id, () => setOtherPriority(r.id, !r.priority)),
          disabled: busy === r.id,
        },
        {
          label: r.active ? t("common.deactivate") : t("common.activate"),
          onclick: () => void run(r.id, () => (r.active ? deactivateOther(r.id) : activateOther(r.id))),
          disabled: busy === r.id,
        },
        {
          label: t("inventory.detach"),
          onclick: () => void run(r.id, () => setOtherAttachment(r.id, "")),
          disabled: busy === r.id || r.attachment.signal !== "USER",
        },
        { label: t("common.delete"), onclick: () => void removeRow(r), disabled: busy === r.id, danger: true },
      );
    }
    return items;
  }

  /** Ce que le clic sur la ligne va ouvrir — dit en infobulle, parce que la
   * destination n'est pas la même selon le type et qu'on ne le devine pas. */
  function fichePromise(r: InventoryRow): string {
    if (r.kind === "OTHER" || r.kind === "SOUND") return "inventory.openFiche";
    return r.attachment.target_id ? "inventory.openHost" : "inventory.noFiche";
  }

  const ICONS: Record<InventoryRow["kind"], string> = {
    SKIN: "▤",
    SOUND: "♪",
    TRACK_SKIN: "◠",
    LAYER: "▦",
    OTHER: "⚙",
  };

  /** Va voir l'hôte d'une ligne (§4.2) : c'est un lien, pas un libellé. */
  async function openHost(r: InventoryRow) {
    const target = r.attachment.target_id;
    if (!target) return;
    const section = r.attachment.kind === "TRACK" ? "tracks" : r.attachment.kind === "APP" ? "apps" : "cars";
    if (await requestSection(section)) nav.openFull = target;
  }

  const FACETS: { axis: string; labelKey: string; values: string[] }[] = [
    { axis: "attach", labelKey: "inventory.facetAttach", values: ["CAR", "TRACK", "APP", "GAME", "STANDALONE"] },
    {
      axis: "nature",
      labelKey: "inventory.facetNature",
      values: ["APPEARANCE", "BEHAVIOUR", "DEPENDENCY", "UNRECOGNISED"],
    },
    { axis: "state", labelKey: "inventory.facetState", values: ["ACTIVE", "INACTIVE", "NOTE"] },
  ];
</script>

{#if fullOther}
  <OtherModDetail
    row={fullOther}
    busy={busy === fullOther.id}
    warnings={[]}
    onclose={() => (fullOther = null)}
    ontoggle={() => void run(fullOther!.id, () => (fullOther!.is_active ? deactivateOther(fullOther!.id) : activateOther(fullOther!.id)))}
    ontogglePriority={() => void run(fullOther!.id, () => setOtherPriority(fullOther!.id, !fullOther!.is_priority))}
    onopenFolder={() => void openOtherModFolder(fullOther!.id)}
    ondelete={() => {
      const id = fullOther!.id;
      fullOther = null;
      void run(id, () => deleteOtherMod(id));
    }}
    onrename={(v) => void run(fullOther!.id, () => setEntityDisplayName("OTHER", fullOther!.id, v ?? ""))}
    onnote={(v) => void run(fullOther!.id, () => setEntityNote("OTHER", fullOther!.id, v ?? ""))}
  />
{:else if fullSound}
  <SoundDetail subId={fullSound} onclose={() => (fullSound = null)} />
{:else}
<div class="screen">
  <header class="head">
    <h2 class="lbl-screen">{t("inventory.title")}</h2>
    <p class="lbl-sub">{t("inventory.subtitle")}</p>
  </header>

  <div class="tools">
    <input class="input search" placeholder={t("inventory.searchPlaceholder")} bind:value={query} />
    <span class="lbl-key mono up">{t("inventory.groupLabel")}</span>
    <Seg
      size="toolbar"
      tone="neutral"
      value={groupBy}
      onselect={(v) => (groupBy = v as GroupBy)}
      items={[
        { value: "none", label: t("inventory.groupNone") },
        { value: "archive", label: t("inventory.groupArchive") },
        { value: "host", label: t("inventory.groupHost") },
      ]}
    />
    <span class="lbl-key mono up">{t("inventory.sortLabel")}</span>
    <Seg
      size="toolbar"
      tone="neutral"
      value={sortBy}
      onselect={(v) => (sortBy = v as SortBy)}
      items={[
        { value: "name", label: t("inventory.sortName") },
        { value: "size", label: t("inventory.sortSize") },
        { value: "import", label: t("inventory.sortImport") },
      ]}
    />
    <span class="count mono">{filtered.length} / {rows.length}</span>
  </div>

  <!-- Facettes : chaque valeur porte son compteur, et un deuxième clic
       l'exclut. Sans le compteur, choisir une facette est un pari. -->
  <div class="facets">
    {#each FACETS as f (f.axis)}
      <div class="facet">
        <span class="lbl-key mono up">{t(f.labelKey)}</span>
        <div class="chips">
          {#each f.values as v (v)}
            {@const key = `${f.axis}:${v}`}
            {@const n = counts.get(key) ?? 0}
            <button
              class="chip"
              class:on={facets[key] === 1}
              class:off={facets[key] === -1}
              type="button"
              disabled={n === 0 && facets[key] === undefined}
              onclick={() => cycle(key)}
            >
              {#if facets[key] === -1}<span class="minus">−</span>{/if}
              {t(`inventory.${f.axis}${v}`)}
              <u class="mono">{n}</u>
            </button>
          {/each}
        </div>
      </div>
    {/each}
  </div>

  {#if error}<div class="errbox">{error}</div>{/if}

  {#if loading}
    <LoadingState />
  {:else if !sorted.length}
    <p class="empty">{rows.length ? t("inventory.emptyFiltered") : t("inventory.empty")}</p>
  {:else}
    {#each groups as g (g.key)}
      {#if g.label}
        <div class="ghead">
          <span class="gname">{g.label}</span>
          <span class="gmeta mono">
            {t("inventory.groupCount", { count: g.rows.length })} · {fmtSize(
              g.rows.reduce((sum, r) => sum + r.size_bytes, 0),
            )}
          </span>
        </div>
      {/if}
      <ul class="rows">
        {#each g.rows as r (r.uid)}
          <li class="row" class:inactive={r.active === false}>
            <span class="ic" aria-hidden="true">{ICONS[r.kind]}</span>
            <!-- Deux niveaux : le nom lisible, l'identifiant technique en
                 dessous. C'est ce qui permet de renommer sans rien perdre.
                 Cliquable : une ligne d'inventaire mène toujours quelque part —
                 à sa propre fiche quand elle en a une, à celle de son hôte
                 sinon, où une livrée et une couche vivent réellement. -->
            <button class="nm" type="button" onclick={() => void openFiche(r)} title={t(fichePromise(r))}>
              <span class="n1">{nameOf(r)}{#if r.has_note}<span class="noteflag" title={t("notes.title")}>✎</span>{/if}</span>
              <span class="n2 mono">{r.tech_id}</span>
            </button>
            {#if r.attachment.target_id}
              <button class="att" type="button" onclick={() => openHost(r)} title={t("inventory.openHost")}>
                {r.attachment.target_name ?? r.attachment.target_id} →
              </button>
            {:else}
              <span class="att game">{t(`inventory.attach${r.attachment.kind}`)}</span>
            {/if}
            <span class="nat mono" class:unknown={r.attachment.nature === "UNRECOGNISED"}>
              {t(`inventory.nature${r.attachment.nature}`)}
            </span>
            {#if r.priority}<span class="prio" title={t("others.priorityTooltip")}>★</span>{/if}
            <span class="st">
              {#if r.active !== null}<StateBadge active={r.active} stock={false} />{/if}
            </span>
            <button
              class="kebab"
              type="button"
              title={t("detail.moreActions")}
              onclick={(e) => {
                e.stopPropagation();
                const rect = (e.currentTarget as HTMLElement).getBoundingClientRect();
                menu = { x: rect.left, y: rect.bottom + 4, row: r };
              }}
            >
              <span class="kd"></span><span class="kd"></span><span class="kd"></span>
            </button>
          </li>
        {/each}
      </ul>
    {/each}
  {/if}

  {#if menu}
    <ContextMenu x={menu.x} y={menu.y} items={menuItems(menu.row)} onclose={() => (menu = null)} />
  {/if}
</div>
{/if}

<style>
  .screen {
    max-width: 1100px;
  }
  .head {
    margin-bottom: 18px;
  }
  .up {
    text-transform: uppercase;
  }
  .tools {
    display: flex;
    align-items: center;
    gap: 12px;
    flex-wrap: wrap;
    margin-bottom: 14px;
  }
  .search {
    width: 280px;
    flex: none;
  }
  .count {
    margin-left: auto;
    color: var(--faint);
    font-size: 11px;
  }
  .facets {
    display: flex;
    flex-wrap: wrap;
    gap: 18px;
    padding-bottom: 14px;
    margin-bottom: 14px;
    border-bottom: 1px solid var(--line);
  }
  .facet {
    display: flex;
    flex-direction: column;
    gap: 6px;
  }
  .chips {
    display: flex;
    gap: 6px;
    flex-wrap: wrap;
  }
  .chip {
    display: inline-flex;
    align-items: center;
    gap: 6px;
    background: var(--panel2);
    border: 1px solid var(--line);
    color: var(--muted);
    font-size: 11px;
    padding: 4px 9px;
  }
  .chip:hover:not(:disabled) {
    color: var(--txt2);
  }
  .chip.on {
    border-color: var(--rosso-border);
    color: var(--txt);
    background: var(--rosso-dim);
  }
  /* Exclusion : le signe porte le sens, pas la couleur — un rouge de plus ici
     concurrencerait l'inclusion (barème du rouge, §7.2ter). */
  .chip.off {
    border-color: var(--faint2);
    color: var(--muted2);
    text-decoration: line-through;
  }
  .chip:disabled {
    opacity: 0.4;
    cursor: default;
  }
  .chip u {
    text-decoration: none;
    color: var(--muted2);
    font-size: 10px;
  }
  .minus {
    color: var(--muted);
  }
  .ghead {
    display: flex;
    align-items: baseline;
    gap: 12px;
    margin: 16px 0 6px;
  }
  .gname {
    font-size: 12px;
    color: var(--txt2);
    overflow-wrap: anywhere;
  }
  .gmeta {
    color: var(--muted2);
    font-size: 10.5px;
  }
  .rows {
    list-style: none;
    display: flex;
    flex-direction: column;
    gap: 2px;
  }
  .row {
    display: flex;
    align-items: center;
    gap: 12px;
    padding: 7px 10px;
    background: var(--panel2);
    border: 1px solid var(--line);
  }
  .row.inactive {
    opacity: 0.72;
  }
  .ic {
    flex: none;
    width: 26px;
    height: 26px;
    display: flex;
    align-items: center;
    justify-content: center;
    background: var(--raised);
    color: var(--muted);
    font-size: 13px;
  }
  .nm {
    flex: 1;
    min-width: 0;
    display: flex;
    flex-direction: column;
    align-items: flex-start;
    gap: 1px;
    background: transparent;
    border: none;
    padding: 0;
    text-align: left;
  }
  .nm:hover .n1 {
    color: var(--txt);
  }
  .n1 {
    font-size: 12px;
    color: var(--txt2);
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }
  .n2 {
    font-size: 10px;
    color: var(--muted2);
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }
  .noteflag {
    color: var(--muted2);
    font-size: 10px;
    margin-left: 6px;
  }
  /* Bleu-lien, réservé à cet usage : aller voir l'hôte. */
  .att {
    flex: none;
    max-width: 220px;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
    background: transparent;
    border: none;
    padding: 0;
    color: var(--blue);
    font-size: 11px;
    text-align: right;
  }
  .att:hover {
    text-decoration: underline;
  }
  .att.game {
    color: var(--muted2);
  }
  .nat {
    flex: none;
    width: 110px;
    color: var(--muted2);
    font-size: 9px;
    letter-spacing: 1px;
    text-transform: uppercase;
  }
  .nat.unknown {
    color: var(--yellow);
  }
  .prio {
    flex: none;
    color: var(--rosso-bright);
    font-size: 11px;
  }
  .st {
    flex: none;
    width: 78px;
    font-size: 11px;
  }
  .kebab {
    flex: none;
    background: transparent;
    display: flex;
    flex-direction: column;
    align-items: center;
    gap: 3px;
    padding: 4px 6px;
  }
  .kd {
    width: 3px;
    height: 3px;
    background: var(--muted2);
    border-radius: 1px;
  }
  .kebab:hover .kd {
    background: var(--rosso-bright);
  }
  .empty {
    color: var(--muted);
    font-size: 12px;
    padding: 40px 0;
    text-align: center;
  }
</style>
