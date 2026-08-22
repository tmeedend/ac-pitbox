<script lang="ts">
  // Vue Apps (§12bis.4) : type autonome, activable/désactivable par junction.
  //
  // La liste ne montre plus les ressources elle-même : elles vivent, avec les
  // ajouts au jeu, sur la **fiche** de l'app (`AppDetail`) — même règle que
  // pour les voitures et les circuits (§4.5.5), les listes de fichiers vivent
  // dans la page pleine. Un dépliant au milieu d'une liste ne tient pas quand
  // l'app pose trente configs CSP.
  import { onMount } from "svelte";
  import { listApps, activateApp, deactivateApp, deleteApp, openAppFolder, type AppItem } from "$lib/apps";
  import { confirm } from "@tauri-apps/plugin-dialog";
  import { t } from "$lib/i18n/index.svelte";
  import AppDetail from "./AppDetail.svelte";
  import LoadingState from "./LoadingState.svelte";
  import StateBadge from "./StateBadge.svelte";

  import { errorText } from "$lib/errors";
  let apps = $state<AppItem[]>([]);
  let query = $state("");
  let busy = $state<string | null>(null);
  let loading = $state(true);
  let error = $state("");
  /** Fiche ouverte, `null` = la liste. Même schéma que `Library`/`DetailPage`. */
  let fullId = $state<string | null>(null);

  // Relu depuis la liste plutôt que mémorisé : après une activation, la fiche
  // doit refléter le nouvel état sans qu'on la remonte.
  const opened = $derived(apps.find((a) => a.id === fullId) ?? null);

  async function load() {
    try {
      apps = await listApps();
    } finally {
      loading = false;
    }
  }
  onMount(load);

  async function toggle(a: AppItem) {
    busy = a.id;
    error = "";
    try {
      if (a.active) await deactivateApp(a.id);
      else await activateApp(a.id);
      await load();
    } catch (e) {
      error = errorText(e);
    } finally {
      busy = null;
    }
  }

  async function openFolder(id: string) {
    try {
      await openAppFolder(id);
    } catch (e) {
      error = errorText(e);
    }
  }

  async function remove(a: AppItem) {
    const ok = await confirm(t("apps.confirmDelete", { id: a.id }), {
      title: t("common.delete"),
      kind: "warning",
    });
    if (!ok) return;
    busy = a.id;
    error = "";
    try {
      await deleteApp(a.id);
      await load();
    } catch (e) {
      error = errorText(e);
    } finally {
      busy = null;
    }
  }

  const filtered = $derived(
    apps.filter((a) => {
      if (!query.trim()) return true;
      // Un terme par mot séparé par un espace, ET entre eux (même correction
      // que la bibliothèque, Library.svelte).
      const terms = query.toLowerCase().split(/\s+/).filter(Boolean);
      const hay = a.id.toLowerCase();
      return terms.every((term) => hay.includes(term));
    }),
  );
</script>

{#if opened}
  <AppDetail app={opened} onclose={() => (fullId = null)} onchange={load} />
{:else}
  <div class="apps">
    <header class="head">
      <div>
        <h2 class="lbl-screen">{t("nav.apps")}</h2>
        <p class="sub">{t("apps.subtitle")}</p>
      </div>
      {#if apps.length}
        <input class="input search" placeholder={t("apps.searchPlaceholder")} bind:value={query} />
      {/if}
    </header>

    {#if error}<div class="err">{error}</div>{/if}

    {#if loading}
      <LoadingState />
    {:else if apps.length === 0}
      <div class="empty">
        <p>{t("apps.empty")}</p>
        <p class="hint">{t("apps.emptyHint", { path: "apps/python/<App>/ · apps/lua/<App>/" })}</p>
      </div>
    {:else}
      <ul class="list">
        {#each filtered as a (a.id)}
          <li class:active={a.active}>
            <div class="row">
              <!-- Le nom ouvre la fiche : c'est la cible la plus large de la
                   ligne, et l'endroit où on clique naturellement. -->
              <button class="a-name mono" type="button" title={t("apps.detailTooltip")} onclick={() => (fullId = a.id)}>
                {a.id}
              </button>
              {#if a.source_archive}<span class="src mono">{a.source_archive}</span>{/if}
              <StateBadge active={a.active} stock={false} />
              <button class="btn" type="button" onclick={() => openFolder(a.id)} title={t("apps.openFolderTooltip")}>
                {t("detail.openFolder")}
              </button>
              <button class="btn" type="button" onclick={() => toggle(a)} disabled={busy === a.id}>
                {busy === a.id ? t("common.working") : a.active ? t("common.deactivate") : t("common.activate")}
              </button>
              <button class="btn del" type="button" title={t("common.delete")} onclick={() => remove(a)} disabled={busy === a.id}>✕</button>
            </div>
          </li>
        {/each}
      </ul>
    {/if}
  </div>
{/if}

<style>
  .apps {
    max-width: 860px;
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
    max-width: 520px;
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
  }
  .list li.active {
    border-left: 3px solid var(--green-border);
  }
  .row {
    display: flex;
    align-items: center;
    gap: 12px;
    padding: 9px 12px;
  }
  /* Bouton et non `<span onclick>` : la liste doit rester atteignable au
     clavier et à la manette, comme les noms cliquables du rapport d'import. */
  .a-name {
    flex: 1;
    min-width: 0;
    background: none;
    border: none;
    padding: 0;
    text-align: left;
    cursor: pointer;
    font-weight: 600;
    color: var(--txt);
    font-size: 12.5px;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }
  .a-name:hover,
  .a-name:focus-visible {
    color: var(--rosso-bright);
  }
  .src {
    color: var(--muted2);
    font-size: 10px;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
    max-width: 200px;
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
  .btn.del {
    padding: 6px 9px;
    color: var(--muted);
  }
  .btn.del:hover {
    border-color: var(--rosso-border);
    color: var(--rosso-bright);
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
