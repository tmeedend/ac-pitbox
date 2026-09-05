<script lang="ts">
  // Vue « Autres mods » (§7.3) : tout mod importé qui n'est ni voiture,
  // circuit, skin, son, ni app (shaders, configs CSP, mods d'UI…). Jamais
  // perdu. Activable par junction comme les autres types, avec le même
  // garde-fou — ce n'est PAS un moteur de superposition complet façon MO2 :
  // juste priorité notée + détection de conflits de fichiers.
  import { onMount } from "svelte";
  import { confirm } from "@tauri-apps/plugin-dialog";
  import {
    listOtherMods,
    setOtherPriority,
    activateOther,
    deactivateOther,
    deleteOtherMod,
    openOtherModFolder,
    OTHER_CATEGORIES,
    type OtherModRow,
  } from "$lib/others";
  import { t } from "$lib/i18n/index.svelte";
  import LoadingState from "./LoadingState.svelte";
  import Tabs, { type TabItem } from "./Tabs.svelte";
  import OtherModDetail from "./OtherModDetail.svelte";
  import Apps from "./Apps.svelte";
  import { listApps } from "$lib/apps";
  import { nav } from "$lib/nav.svelte";

  import { errorText } from "$lib/errors";

  /** Les apps sont un onglet de plus, pas une entrée de rail (SPEC §7.3) :
   * cet écran est déjà le tiroir de ce qui n'est ni voiture ni circuit, et le
   * jeu d'onglets existait déjà — c'est une entrée de plus, pas une structure
   * de plus. **En tête de bande** : c'est la rubrique qu'on vient voir, les
   * zones du jeu sont le fourre-tout derrière.
   *
   * Il n'y a **plus d'onglet « Tous »** : sur une vraie collection il ramassait
   * tout ce que l'écran contient, ce qui n'est pas une vue mais une absence de
   * vue — et c'était celle qui s'ouvrait par défaut. */
  const APPS_TAB = "apps";
  let others = $state<OtherModRow[]>([]);
  let query = $state("");
  /** Onglet courant. `ALL_TAB` n'est pas une catégorie du backend : c'est
   * l'écran d'avant les onglets, gardé parce qu'il reste la seule vue où deux
   * mods de zones différentes se comparent. */
  let tab = $state(APPS_TAB);
  /** Décompte des apps, pour l'onglet. Chargé ici et pas remonté depuis
   * `Apps.svelte` : ce composant n'est monté que lorsque son onglet est
   * ouvert, or le décompte doit s'afficher même quand on regarde ailleurs. */
  let appCount = $state<number | null>(null);
  /** Une fiche d'app est ouverte dans l'onglet Apps : cet écran retire alors
   * son propre en-tête, comme il le fait pour ses propres fiches. */
  let appsDetail = $state(false);
  let busy = $state<string | null>(null);
  let loading = $state(true);
  let error = $state("");
  let warnings = $state<Record<string, string[]>>({});
  /** Fiche ouverte (§7.3, sur le patron de `SoundDetail`/`fullSoundId`). */
  let fullId = $state<string | null>(null);
  const fullRow = $derived(others.find((o) => o.id === fullId) ?? null);

  async function load() {
    try {
      others = await listOtherMods();
    } finally {
      loading = false;
    }
  }
  onMount(load);
  // Un échec laisse l'onglet sans décompte plutôt que d'afficher zéro : « 0 »
  // dirait qu'il n'y a pas d'app, ce qu'on ne sait pas.
  onMount(() => {
    listApps()
      .then((apps) => (appCount = apps.length))
      .catch(() => (appCount = null));
  });

  function name(id: string): string {
    return others.find((o) => o.id === id)?.id ?? id;
  }

  async function toggle(o: OtherModRow) {
    busy = o.id;
    error = "";
    const { [o.id]: _drop, ...rest } = warnings;
    warnings = rest;
    try {
      if (o.is_active) {
        await deactivateOther(o.id);
      } else {
        const res = await activateOther(o.id);
        if (res.warnings.length) warnings = { ...warnings, [o.id]: res.warnings };
      }
      await load();
    } catch (e) {
      error = errorText(e);
    } finally {
      busy = null;
    }
  }

  async function togglePriority(o: OtherModRow) {
    busy = o.id;
    error = "";
    try {
      await setOtherPriority(o.id, !o.is_priority);
      await load();
    } catch (e) {
      error = errorText(e);
    } finally {
      busy = null;
    }
  }

  async function openFolder(o: OtherModRow) {
    error = "";
    try {
      await openOtherModFolder(o.id);
    } catch (e) {
      error = errorText(e);
    }
  }

  async function remove(o: OtherModRow) {
    const ok = await confirm(t("others.confirmDelete", { id: o.id }), {
      title: t("common.delete"),
      kind: "warning",
    });
    if (!ok) return;
    busy = o.id;
    error = "";
    try {
      await deleteOtherMod(o.id);
      if (fullId === o.id) fullId = null;
      await load();
    } catch (e) {
      error = errorText(e);
    } finally {
      busy = null;
    }
  }

  const searched = $derived(
    others.filter((o) => {
      if (!query.trim()) return true;
      // Un terme par mot séparé par un espace, ET entre eux (même correction
      // que la bibliothèque, Library.svelte).
      const terms = query.toLowerCase().split(/\s+/).filter(Boolean);
      const hay = o.id.toLowerCase();
      return terms.every((term) => hay.includes(term));
    }),
  );

  /** Onglets à afficher : l'ordre connu, plus les catégories que le backend
   * renverrait sans que `OTHER_CATEGORIES` les connaisse — insérées avant
   * « Autres », qui reste la fin de liste. Sans ce rattrapage, une catégorie
   * ajoutée côté Rust et oubliée ici rendrait ses mods introuvables. */
  const categoryIds = $derived.by(() => {
    const known = new Set<string>(OTHER_CATEGORIES);
    const unknown = [...new Set(others.flatMap((o) => o.categories))].filter((c) => !known.has(c));
    return [...OTHER_CATEGORIES.filter((c) => c !== "other"), ...unknown, "other"];
  });

  // Décomptes sur la totalité, pas sur la recherche : un onglet qui se vide
  // en cours de frappe fait sauter la sélection d'un onglet à l'autre.
  const tabs = $derived<TabItem[]>([
    { id: APPS_TAB, label: t("nav.apps"), count: appCount ?? undefined },
    ...categoryIds.map((c) => {
      const count = others.filter((o) => o.categories.includes(c)).length;
      return { id: c, label: t(`others.cat.${c}`), count, disabled: count === 0 };
    }),
  ]);

  const filtered = $derived(searched.filter((o) => o.categories.includes(tab)));
</script>

{#if fullRow}
  <OtherModDetail
    row={fullRow}
    busy={busy === fullRow.id}
    warnings={warnings[fullRow.id] ?? []}
    onclose={() => (fullId = null)}
    ontoggle={() => toggle(fullRow)}
    ontogglePriority={() => togglePriority(fullRow)}
    onopenFolder={() => openFolder(fullRow)}
    ondelete={() => remove(fullRow)}
  />
{:else}
<div class="others">
  {#if !appsDetail}
    <header class="head">
      <div>
        <h2 class="lbl-screen">{t("nav.others")}</h2>
        <p class="sub">{t("others.subtitle")}</p>
      </div>
      {#if others.length && tab !== APPS_TAB}
        <input class="input search" placeholder={t("others.searchPlaceholder")} bind:value={query} />
      {/if}
    </header>

    {#if error}<div class="err">{error}</div>{/if}

    {#if loading}
      <LoadingState />
    {:else}
      <!-- La barre d'onglets ne dépend plus de la présence d'autres mods :
           sinon une collection qui n'en compte aucun rendrait l'onglet Apps
           inatteignable, alors qu'il ne parle pas d'eux. -->
      <Tabs {tabs} active={tab} onselect={(id) => (tab = id)} />
    {/if}
  {/if}
  {#if !loading && tab === APPS_TAB}
    <Apps embedded bind:detailOpen={appsDetail} />
  {:else if !loading && others.length === 0}
    <div class="empty">
      <p>{t("others.empty")}</p>
      <p class="hint">{t("others.emptyHint")}</p>
    </div>
  {:else if !loading}
    {#if filtered.length === 0}
      <div class="empty"><p>{t("others.noMatch")}</p></div>
    {/if}
    <ul class="list">
      {#each filtered as o (o.id)}
        <li class:active={o.is_active}>
          <div class="row">
            <button class="o-name o-link mono" type="button" title={t("others.detailTooltip")} onclick={() => (fullId = o.id)}>
              {o.id}
            </button>
            <span class="cats">
              {#each o.categories as c}
                <span class="cat" class:here={c === tab}>{t(`others.cat.${c}`)}</span>
              {/each}
            </span>
            {#if o.source_archive}<span class="src mono">{o.source_archive}</span>{/if}
            {#if o.externally_managed}
              <span class="managed" title={t("others.managedTooltip")}>
                {t("others.managed", { count: o.externally_managed })}
              </span>
            {/if}
            {#if o.is_active}<span class="state on">{t("common.active").toLowerCase()}</span>{:else}<span class="state">{t("common.inactive").toLowerCase()}</span>{/if}
            <button class="btn" type="button" onclick={() => openFolder(o)} title={t("others.openFolder")}>
              {t("others.openFolder")}
            </button>
            <button class="btn prio" class:on={o.is_priority} type="button" onclick={() => togglePriority(o)} disabled={busy === o.id} title={t("others.priorityTooltip")}>
              {t("others.priority")}
            </button>
            <button class="btn" type="button" onclick={() => toggle(o)} disabled={busy === o.id}>
              {busy === o.id ? t("common.working") : o.is_active ? t("common.deactivate") : t("common.activate")}
            </button>
            <button class="btn del" type="button" title={t("common.delete")} onclick={() => remove(o)} disabled={busy === o.id}>✕</button>
          </div>
          {#if o.conflicts.length}
            <div class="conflicts">
              {t("others.conflictsWith")} {#each o.conflicts as c, i}{i > 0 ? ", " : ""}<b>{name(c.other_id)}</b> ({c.count}){/each}
            </div>
          {/if}
          {#if warnings[o.id]?.length}
            <ul class="warn-list">
              {#each warnings[o.id] as w}<li>{w}</li>{/each}
            </ul>
          {/if}
        </li>
      {/each}
    </ul>
  {/if}
</div>
{/if}

<style>
  .others {
    max-width: 820px;
  }
  .head {
    display: flex;
    align-items: flex-start;
    justify-content: space-between;
    gap: 20px;
    margin-bottom: 18px;
  }
  .sub {
    color: var(--muted);
    font-size: 12px;
    margin-top: 6px;
    line-height: 1.5;
    max-width: 620px;
  }
  .search {
    width: 220px;
    flex: none;
  }
  .err {
    background: var(--rosso-dim);
    border: 1px solid var(--rosso-border);
    color: var(--rosso-bright);
    padding: 10px 12px;
    font-size: 12px;
    margin-bottom: 14px;
  }
  .list {
    list-style: none;
    display: flex;
    flex-direction: column;
    gap: 5px;
  }
  .list li {
    border: 1px solid var(--line);
    background: var(--panel2);
    padding: 9px 12px;
  }
  .list li.active {
    border-left: 3px solid var(--green-border);
  }
  .row {
    display: flex;
    align-items: flex-start;
    gap: 12px;
  }
  /* Coupé n'importe où plutôt que tronqué. Un id de reste est de la forme
     `<archive>__<chemin>` et n'a souvent aucun espace :
     `CMRT_Complete_hud_v2.1.3.zip__CMRT_flag_fuel_and_starting_lights_replacement`.
     Sans point de coupure, le mot pousse la ligne et déforme la page ; avec
     une ellipse, c'est la **fin** qui disparaît — or c'est précisément elle
     qui distingue deux restes de la même archive (§7.3). */
  .o-name {
    flex: 1;
    min-width: 0;
    overflow-wrap: anywhere;
    font-weight: 600;
    color: var(--txt);
    font-size: 12.5px;
  }
  /* Le nom ouvre la fiche du mod : un bouton, mais qui n'en a pas l'air —
     même patron que `.s-link` sur la fiche des sons (Transversal.svelte). */
  .o-link {
    background: none;
    border: none;
    padding: 0;
    font: inherit;
    text-align: left;
    cursor: pointer;
  }
  .o-link:hover,
  .o-link:focus-visible {
    color: var(--rosso-bright);
  }
  /* Zones touchées par le mod. Un mod qui en touche deux est listé sous les
     deux onglets : ces pastilles sont ce qui permet de reconnaître le même
     mod d'un onglet à l'autre, celle de l'onglet courant étant marquée. */
  .cats {
    display: flex;
    gap: 4px;
    flex: none;
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
  .cat.here {
    color: var(--txt2);
    border-color: var(--muted2);
  }
  .src {
    color: var(--muted2);
    font-size: 10px;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
    max-width: 200px;
  }
  .state {
    font-size: 10px;
    text-transform: uppercase;
    letter-spacing: 0.5px;
    color: var(--muted);
  }
  .state.on {
    color: var(--green);
  }
  /* Bleu = information, comme dans « Ajouts au jeu » (§4.5.5) : le chemin est
     partagé avec un outil externe, ce n'est pas une anomalie. */
  .managed {
    color: var(--blue);
    font-size: 10px;
    white-space: nowrap;
  }
  .btn {
    background: var(--raised);
    color: var(--txt2);
    border: 1px solid var(--line);
    font-size: 11px;
    padding: 6px 12px;
    flex: none;
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
    padding: 6px 9px;
    color: var(--muted);
  }
  .btn.del:hover {
    border-color: var(--rosso-border);
    color: var(--rosso-bright);
  }
  .conflicts {
    margin-top: 7px;
    font-size: 11px;
    color: var(--yellow);
  }
  .warn-list {
    margin-top: 6px;
    padding-left: 16px;
    font-size: 11px;
    color: var(--muted);
  }
  .empty {
    color: var(--muted);
    text-align: center;
    padding: 50px 0;
  }
  .empty .hint {
    font-size: 12px;
    color: var(--faint);
    margin-top: 8px;
  }
</style>
