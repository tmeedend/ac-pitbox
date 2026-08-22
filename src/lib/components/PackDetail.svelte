<script lang="ts">
  // Fiche d'un pack (§4.4). Page pleine, comme `DetailPage` et `AppDetail`, et
  // pour la même raison : ce qu'il y a à montrer, ce sont des listes de
  // fichiers, qui n'ont leur place ni dans un panneau ni dans un dépliant.
  //
  // Un pack n'est qu'un nom d'archive porté par la colonne `source_pack` de
  // chacun de ses mods. Il possède pourtant des fichiers que ses membres
  // n'ont pas — ce qui entourait les mods sans appartenir à aucun — et rien
  // ne les montrait : `list_mod_extras` ne regarde que `extras/<type>/<id>`.
  // Cas réel : un pack de 94 voitures livrant `content/{driver,fonts,texture}`,
  // 82 fichiers bien posés dans le jeu et introuvables dans l'app.
  import { confirm } from "@tauri-apps/plugin-dialog";
  import { getPackDetail, type PackDetail } from "$lib/packs";
  import { deletePack } from "$lib/maintenance";
  import { previewSrc } from "$lib/library";
  import { fmtSize } from "$lib/format";
  import { errorText } from "$lib/errors";
  import { t } from "$lib/i18n/index.svelte";
  import ExtrasBlock from "./detail/ExtrasBlock.svelte";
  import ResourcesBlock from "./detail/ResourcesBlock.svelte";
  import LoadingState from "./LoadingState.svelte";
  import Tabs from "./Tabs.svelte";

  interface Props {
    pack: string;
    /** Retour d'où l'on vient. */
    onclose: () => void;
    /** Ouvrir la fiche d'un des mods du pack. */
    onopenmod: (id: string) => void;
    /** Le pack vient d'être désinstallé : ses mods n'existent plus, donc la
     * fiche de mod restée derrière celle-ci non plus. */
    onuninstalled: () => void;
  }
  let { pack, onclose, onopenmod, onuninstalled }: Props = $props();

  let detail = $state<PackDetail | null>(null);
  let loading = $state(true);
  let busy = $state(false);
  let error = $state("");
  let tab = $state("members");

  // Même garde que les blocs de la fiche détail : une réponse tardive d'un
  // pack précédent ne doit pas écraser celui qu'on regarde.
  $effect(() => {
    const current = pack;
    loading = true;
    getPackDetail(current)
      .then((d) => {
        if (current === pack) detail = d;
      })
      .catch((e) => {
        if (current === pack) error = errorText(e);
      })
      .finally(() => {
        if (current === pack) loading = false;
      });
  });

  const tabs = $derived([
    { id: "members", label: t("pack.tabMembers"), count: detail?.members.length },
    { id: "extras", label: t("detail.tabExtras"), count: detail?.extras.length },
    { id: "resources", label: t("detail.tabResources") },
  ]);

  async function uninstall() {
    if (!detail) return;
    const ok = await confirm(t("pack.uninstallConfirm", { name: detail.name, count: detail.members.length }), {
      title: t("pack.uninstall"),
      kind: "warning",
    });
    if (!ok) return;
    busy = true;
    error = "";
    try {
      await deletePack(detail.name);
      onuninstalled();
      onclose();
    } catch (e) {
      error = errorText(e);
      busy = false;
    }
  }
</script>

<div class="page">
  <header class="head">
    <button class="back" type="button" onclick={onclose}>{t("apps.back")}</button>
    <h2 class="lbl-screen mono">{pack}</h2>
    {#if detail}
      <div class="actions">
        <button class="btn del" type="button" onclick={uninstall} disabled={busy}>
          {busy ? t("common.working") : t("pack.uninstall")}
        </button>
      </div>
    {/if}
  </header>

  {#if error}<div class="err">{error}</div>{/if}

  {#if loading}
    <LoadingState />
  {:else if detail}
    <dl class="meta">
      <div>
        <dt class="lbl-key">{t("pack.membersLabel")}</dt>
        <dd class="mono">{detail.members.length}</dd>
      </div>
      <div>
        <dt class="lbl-key">{t("pack.sizeLabel")}</dt>
        <dd class="mono">{fmtSize(detail.members_bytes)}</dd>
      </div>
      <div>
        <dt class="lbl-key">{t("pack.extrasSizeLabel")}</dt>
        <dd class="mono">{fmtSize(detail.extras_bytes)}</dd>
      </div>
      {#if detail.imported_at}
        <div>
          <dt class="lbl-key">{t("apps.importedAt")}</dt>
          <dd>{new Date(detail.imported_at).toLocaleString()}</dd>
        </div>
      {/if}
    </dl>

    <Tabs {tabs} active={tab} onselect={(id) => (tab = id)} />

    <div class="body">
      {#if tab === "members"}
        <ul class="members">
          {#each detail.members as m (m.id_interne)}
            <li>
              <button type="button" class="member" onclick={() => onopenmod(m.id_interne)}>
                {#if m.preview}
                  <img src={previewSrc(m.preview)} alt="" loading="lazy" />
                {:else}
                  <span class="no-img" aria-hidden="true"></span>
                {/if}
                <span class="m-name">{m.display_name ?? m.id_interne}</span>
                <span class="m-id mono">{m.id_interne}</span>
              </button>
            </li>
          {/each}
        </ul>
      {:else if tab === "extras"}
        <ExtrasBlock modId={pack} source="pack" />
      {:else}
        <ResourcesBlock modId={pack} source="pack" onerror={(m) => (error = m)} />
      {/if}
    </div>
  {/if}
</div>

<style>
  .page {
    max-width: 980px;
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
    background: var(--raised);
    border: 1px solid var(--line);
    color: var(--txt2);
    font-size: 11px;
    padding: 6px 12px;
  }
  .back:hover {
    border-color: var(--rosso-border);
    color: var(--rosso-bright);
  }
  .actions {
    display: flex;
    gap: 8px;
  }
  .btn {
    background: var(--raised);
    color: var(--txt2);
    border: 1px solid var(--line);
    font-size: 11px;
    padding: 6px 12px;
  }
  .btn:hover:not(:disabled) {
    border-color: var(--rosso-border);
    color: var(--rosso-bright);
  }
  .btn:disabled {
    opacity: 0.5;
  }
  .btn.del {
    color: var(--muted);
  }
  .err {
    background: var(--rosso-dim);
    border: 1px solid var(--rosso-border);
    color: var(--rosso-bright);
    padding: 10px 12px;
    font-size: 12px;
    margin-bottom: 14px;
  }
  .meta {
    display: flex;
    flex-wrap: wrap;
    gap: 26px;
    margin-bottom: 18px;
  }
  .meta dd {
    color: var(--txt);
    font-size: 12.5px;
    margin-top: 3px;
  }
  .members {
    list-style: none;
    display: grid;
    grid-template-columns: repeat(auto-fill, minmax(190px, 1fr));
    gap: 10px;
  }
  /* Une vignette par membre : c'est ainsi que l'utilisateur reconnaît une
     voiture, un nom seul ne suffit pas sur un pack de quatre-vingt-dix. */
  .member {
    width: 100%;
    display: flex;
    flex-direction: column;
    align-items: stretch;
    gap: 0;
    background: var(--panel2);
    border: 1px solid var(--line);
    padding: 0;
    text-align: left;
  }
  .member:hover {
    border-color: var(--rosso-border);
  }
  .member img,
  .member .no-img {
    width: 100%;
    aspect-ratio: 16 / 9;
    object-fit: cover;
    display: block;
    background: var(--raised);
  }
  .m-name {
    color: var(--txt);
    font-size: 12px;
    font-weight: 600;
    padding: 7px 9px 0;
    overflow-wrap: anywhere;
  }
  .m-id {
    color: var(--muted2);
    font-size: 10px;
    padding: 2px 9px 8px;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }
</style>
