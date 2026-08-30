<script lang="ts">
  // Fiche d'une app (§12bis.4). Page pleine, comme `DetailPage` — et pour la
  // même raison qu'elle (§4.5.5) : les listes de fichiers vivent dans la page
  // pleine, pas dans un panneau ni dans un dépliant au milieu d'une liste. Une
  // app qui pose trente configs CSP ferait déborder la vue Apps.
  //
  // Sans tags ni fiche technique, contrairement à une voiture : une app n'en a
  // pas. Ce qui la décrit tient sur une ligne — nom, langage, provenance — et
  // le reste de la page est ce qu'elle met sur le disque.
  import { activateApp, deactivateApp, deleteApp, openAppFolder, type AppItem } from "$lib/apps";
  import { confirm } from "@tauri-apps/plugin-dialog";
  import { errorText } from "$lib/errors";
  import { t } from "$lib/i18n/index.svelte";
  import ExtrasBlock from "./detail/ExtrasBlock.svelte";
  import LayersBlock from "./detail/LayersBlock.svelte";
  import ResourcesBlock from "./detail/ResourcesBlock.svelte";
  import StateBadge from "./StateBadge.svelte";
  import Tabs from "./Tabs.svelte";

  interface Props {
    app: AppItem;
    /** Retour à la liste. */
    onclose: () => void;
    /** L'app a changé d'état ou a disparu : la liste doit se relire. */
    onchange: () => void;
  }
  let { app, onclose, onchange }: Props = $props();

  let tab = $state("resources");
  let busy = $state(false);
  let error = $state("");

  const tabs = $derived([
    { id: "resources", label: t("detail.tabResources") },
    { id: "extras", label: t("detail.tabExtras") },
    { id: "layers", label: t("detail.layersTitle") },
  ]);

  async function toggle(): Promise<void> {
    busy = true;
    error = "";
    try {
      if (app.active) await deactivateApp(app.id);
      else await activateApp(app.id);
      onchange();
    } catch (e) {
      error = errorText(e);
    } finally {
      busy = false;
    }
  }

  async function openFolder(): Promise<void> {
    try {
      await openAppFolder(app.id);
    } catch (e) {
      error = errorText(e);
    }
  }

  async function remove(): Promise<void> {
    const ok = await confirm(t("apps.confirmDelete", { id: app.id }), {
      title: t("common.delete"),
      kind: "warning",
    });
    if (!ok) return;
    busy = true;
    error = "";
    try {
      await deleteApp(app.id);
      onclose();
      onchange();
    } catch (e) {
      error = errorText(e);
      busy = false;
    }
  }
</script>

<div class="page">
  <header class="head">
    <button class="back" type="button" onclick={onclose}>{t("apps.back")}</button>
    <h2 class="lbl-screen mono">{app.id}</h2>
    <StateBadge active={app.active} stock={false} />
    <div class="actions">
      <button class="btn" type="button" onclick={openFolder} title={t("apps.openFolderTooltip")}>
        {t("detail.openFolder")}
      </button>
      <button class="btn" type="button" onclick={toggle} disabled={busy}>
        {busy ? t("common.working") : app.active ? t("common.deactivate") : t("common.activate")}
      </button>
      <button class="btn del" type="button" title={t("common.delete")} onclick={remove} disabled={busy}>✕</button>
    </div>
  </header>

  <!-- Tout ce qui décrit une app tient là : d'où elle vient, quand elle est
       arrivée, et sous quel `apps/<langue>/` elle est posée. -->
  <dl class="meta">
    <div>
      <dt class="lbl-key">{t("apps.langLabel")}</dt>
      <dd class="mono">{app.lang === "lua" ? t("apps.langLua") : t("apps.langPython")}</dd>
    </div>
    {#if app.source_archive}
      <div>
        <dt class="lbl-key">{t("detail.sourceLabel")}</dt>
        <dd class="mono">{app.source_archive}</dd>
      </div>
    {/if}
    <div>
      <dt class="lbl-key">{t("apps.importedAt")}</dt>
      <dd>{new Date(app.imported_at).toLocaleString()}</dd>
    </div>
  </dl>

  {#if error}<div class="err">{error}</div>{/if}

  <Tabs {tabs} active={tab} onselect={(id) => (tab = id)} />

  <div class="body">
    {#if tab === "resources"}
      <ResourcesBlock modId={app.id} source="app" onerror={(m) => (error = m)} />
    {:else if tab === "extras"}
      <ExtrasBlock modId={app.id} source="app" />
    {:else}
      <!-- Le composant des couches d'un mod, repris tel quel : il ne connaît
           qu'un id et quatre commandes, et une app est un hôte comme un autre
           (§12bis.4). Recomposer change l'état de l'app — d'où `onchange`. -->
      <LayersBlock modId={app.id} onchanged={onchange} onerror={(m) => (error = m)} />
    {/if}
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
  .actions .del {
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
