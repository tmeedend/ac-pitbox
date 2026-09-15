<script lang="ts">
  // Fiche d'une app (§8.4). Page pleine, comme `DetailPage` — et pour la
  // même raison qu'elle (§4.5.5) : les listes de fichiers vivent dans la page
  // pleine, pas dans un panneau ni dans un dépliant au milieu d'une liste. Une
  // app qui pose trente configs CSP ferait déborder la vue Apps.
  //
  // Sans tags ni fiche technique, contrairement à une voiture : une app n'en a
  // pas. Ce qui la décrit tient sur une ligne — nom, langage, provenance — et
  // le reste de la page est ce qu'elle met sur le disque.
  import { activateApp, deactivateApp, deleteApp, openAppFolder, type AppItem } from "$lib/inventory/apps";
  import { confirm } from "@tauri-apps/plugin-dialog";
  import type { LayerRow } from "$lib/library/library";
  import { errorText } from "$lib/errors";
  import { setEntityDisplayName, setEntityNote } from "$lib/detail/userMeta";
  import { t } from "$lib/i18n/index.svelte";
  import ExtrasBlock from "$lib/components/detail/ExtrasBlock.svelte";
  import LayersBlock from "$lib/components/detail/LayersBlock.svelte";
  import ResourcesBlock from "$lib/components/detail/ResourcesBlock.svelte";
  import FicheHeader from "$lib/components/detail/FicheHeader.svelte";
  import NoteBlock from "$lib/components/detail/NoteBlock.svelte";
  import Tabs from "$lib/components/ui/Tabs.svelte";

  interface Props {
    app: AppItem;
    /** Retour à la liste. */
    onclose: () => void;
    /** L'app a changé d'état ou a disparu : la liste doit se relire. */
    onchange: () => void;
  }
  let { app, onclose, onchange }: Props = $props();

  let tab = $state("resources");
  /** Fiche d'une couche ouverte par-dessus celle de l'app (REFONTE§8.4). */
  let openLayer = $state<{ layer: LayerRow; siblings: number } | null>(null);
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

  /** Renommer (§6.1) : la saisie vit dans l'overlay, à côté du nom dérivé du
   * fichier et jamais à sa place — vider le champ ramène donc celui-ci. */
  /** Note libre (§9). Passe par la commande commune à tous les types plutôt
   * que par `setModField` : c'est la même colonne sur les cinq tables, et le
   * même geste. */
  async function saveNote(value: string | null): Promise<void> {
    error = "";
    try {
      await setEntityNote("APP", app.id, value ?? "");
      onchange();
    } catch (e) {
      error = errorText(e);
    }
  }

  async function rename(value: string | null): Promise<void> {
    error = "";
    try {
      await setEntityDisplayName("APP", app.id, value ?? "");
      onchange();
    } catch (e) {
      error = errorText(e);
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
  <FicheHeader
    onback={onclose}
    backLabel={t("apps.back")}
    glyph="◈"
    name={app.display_name_user ?? app.id}
    subtitle={app.display_name_user ? app.id : undefined}
    rename={{
      original: app.id,
      overridden: !!app.display_name_user,
      onsave: rename,
    }}
    deployment={{ active: app.active }}
    actions={[
      { label: t("detail.openFolder"), onclick: openFolder },
      {
        label: busy ? t("common.working") : app.active ? t("common.deactivate") : t("common.activate"),
        onclick: toggle,
        disabled: busy,
      },
      { label: t("common.delete"), onclick: remove, disabled: busy, danger: true },
    ]}
  />

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

  <NoteBlock value={app.notes_user} onsave={saveNote} />

  {#if error}<div class="errbox">{error}</div>{/if}

  <Tabs {tabs} active={tab} onselect={(id) => (tab = id)} />

  <div class="body">
    {#if tab === "resources"}
      <ResourcesBlock modId={app.id} source="app" onerror={(m) => (error = m)} />
    {:else if tab === "extras"}
      <ExtrasBlock modId={app.id} source="app" />
    {:else}
      <!-- Le composant des couches d'un mod, repris tel quel : il ne connaît
           qu'un id et quatre commandes, et une app est un hôte comme un autre
           (§8.4). Recomposer change l'état de l'app — d'où `onchange`. -->
      <LayersBlock
        modId={app.id}
        hostKind="App"
        hostName={app.display_name_user ?? app.id}
        onchanged={onchange}
        onerror={(m) => (error = m)}
        onopen={(layer, siblings) => (openLayer = { layer, siblings })}
      />
    {/if}
  </div>
</div>

<style>
  .page {
    max-width: 860px;
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
  .errbox {
    margin-bottom: 10px;
  }
</style>
