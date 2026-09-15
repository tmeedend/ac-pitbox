<script lang="ts">
  // Fiche d'une livrée (§8.3), voiture ou circuit.
  //
  // Elle manquait, et son absence coûtait un geste sur deux dans l'inventaire :
  // une ligne de livrée porte déjà un lien vers son hôte **à droite**, et
  // cliquer son titre menait au même endroit. Deux chemins, une destination,
  // et rien nulle part sur la livrée elle-même — ce qu'elle pèse, d'où elle
  // vient, ce qu'elle contient, et si le jeu la voit.
  //
  // Rien n'est réinventé : `FicheHeader`, `NoteBlock` et le vocabulaire d'état
  // sont ceux des cinq autres fiches (REFONTE§6.1).
  import { onMount } from "svelte";
  import { t } from "$lib/i18n/index.svelte";
  import { errorText } from "$lib/errors";
  import { fmtSize } from "$lib/format";
  import { openSkinFolder, skinDetail, skinImageSrc, type SkinDetail } from "$lib/skins";
  import { setEntityDisplayName, setEntityNote } from "$lib/userMeta";
  import FicheHeader from "./FicheHeader.svelte";
  import NoteBlock from "./NoteBlock.svelte";

  interface Props {
    subId: string;
    onclose: () => void;
    /** Aller voir la voiture/le circuit habillé — le même geste que le lien de
     * la ligne d'inventaire, repris ici parce que c'est de la fiche qu'on y
     * pense. */
    onopenHost?: () => void;
    /** Supprimer : seul l'inventaire sait relire sa liste ensuite. Absent pour
     * une livrée livrée avec son mod, que seul le mod entier peut retirer. */
    ondelete?: () => void;
  }

  const { subId, onclose, onopenHost, ondelete }: Props = $props();

  let detail = $state<SkinDetail | null>(null);
  let error = $state("");

  async function load() {
    try {
      detail = await skinDetail(subId);
      error = "";
    } catch (e) {
      error = errorText(e);
    }
  }
  onMount(load);

  /** Renommer (REFONTE§6.1) : la saisie vit dans l'overlay, à côté du nom du
   * dossier et jamais à sa place — vider le champ ramène celui-ci. */
  async function rename(value: string | null): Promise<void> {
    error = "";
    try {
      await setEntityDisplayName("SUB_MOD", subId, value ?? "");
      await load();
    } catch (e) {
      error = errorText(e);
    }
  }

  async function saveNote(value: string | null): Promise<void> {
    error = "";
    try {
      await setEntityNote("SUB_MOD", subId, value ?? "");
      await load();
    } catch (e) {
      error = errorText(e);
    }
  }

  /** L'image de la fiche : la `livery.png` d'abord — c'est la livrée seule,
   * sans la carrosserie autour, donc ce qui identifie le mieux un habillage à
   * la taille d'une tuile (même choix que le sélecteur de session, §8). */
  const tile = $derived(skinImageSrc(detail?.livery ?? detail?.preview ?? null));
  const shot = $derived(skinImageSrc(detail?.preview ?? null));

  /** Nom affiché : la reprise à la main d'abord, puis ce que déclare
   * `ui_skin.json`, puis le nom du dossier. Le sous-titre ne redit le dossier
   * que s'il apprend quelque chose. */
  const shown = $derived(detail ? (detail.displayNameUser ?? detail.uiName ?? detail.name) : subId);

  const actions = $derived.by(() => {
    const out: { label: string; onclick: () => void; danger?: boolean }[] = [];
    out.push({ label: t("skins.openFolder"), onclick: () => void openSkinFolder(subId) });
    if (onopenHost) out.push({ label: t("inventory.openHost"), onclick: onopenHost });
    if (ondelete) out.push({ label: t("common.delete"), onclick: ondelete, danger: true });
    return out;
  });
</script>

<div class="page">
  <FicheHeader
    onback={onclose}
    backLabel={t("skins.back")}
    glyph={detail?.subType === "TRACK_SKIN" ? "◠" : "▤"}
    image={tile}
    imageAlt={shown}
    name={shown}
    subtitle={detail && shown !== detail.name ? detail.name : undefined}
    rename={detail
      ? { original: detail.uiName ?? detail.name, overridden: !!detail.displayNameUser, onsave: rename }
      : undefined}
    deployment={detail?.subType === "TRACK_SKIN" ? { active: detail.isActive } : undefined}
    {actions}
  />

  {#if error}<div class="errbox">{error}</div>{/if}

  {#if detail}
    {@const d = detail}
    <!-- Le seul fait de cette fiche qui demande une action : stockée mais pas
         posée dans le dossier de l'hôte, la livrée n'existe pas pour le jeu.
         Sans cette ligne, elle est parfaitement normale partout. -->
    {#if !d.projected}
      <div class="warnbox">{t("skins.notProjected")}</div>
    {/if}

    <NoteBlock value={d.notesUser} onsave={saveNote} />

    <dl class="meta">
      <div>
        <dt class="lbl-key">{t(d.subType === "TRACK_SKIN" ? "skins.trackLabel" : "skins.carLabel")}</dt>
        <dd>
          {#if onopenHost}
            <button class="hostlink" type="button" onclick={onopenHost}>{d.parentName ?? d.parentId} →</button>
          {:else}
            {d.parentName ?? d.parentId}
          {/if}
          <span class="mono dim">{d.parentId}</span>
        </dd>
      </div>
      <div>
        <dt class="lbl-key">{t("sounds.sizeLabel")}</dt>
        <dd class="mono">{fmtSize(d.sizeBytes)}</dd>
      </div>
      <div>
        <dt class="lbl-key">{t("skins.filesLabel")}</dt>
        <dd class="mono">{d.files.length}</dd>
      </div>
      {#if d.sourceArchive}
        <div>
          <dt class="lbl-key">{t("detail.sourceLabel")}</dt>
          <dd class="mono">{d.sourceArchive}</dd>
        </div>
      {/if}
      <div>
        <dt class="lbl-key">{t("apps.importedAt")}</dt>
        <dd>{new Date(d.importedAt).toLocaleString()}</dd>
      </div>
    </dl>

    {#if shot}
      <section class="blk">
        <header class="blk-h"><span class="blk-t">{t("skins.previewLabel")}</span></header>
        <div class="blk-b">
          <img class="shot" src={shot} alt={shown} />
        </div>
      </section>
    {/if}

    <section class="blk">
      <header class="blk-h">
        <span class="blk-t">{t("skins.filesLabel")}</span>
        <span class="blk-n">{fmtSize(d.sizeBytes)}</span>
      </header>
      <div class="blk-b">
        {#if d.files.length}
          <ul class="files">
            {#each d.files as f (f.path)}
              <li>
                <span class="fname mono">{f.path}</span>
                <span class="fsize mono">{fmtSize(f.sizeBytes)}</span>
              </li>
            {/each}
          </ul>
        {:else}
          <p class="muted small">{t("skins.noFiles")}</p>
        {/if}
      </div>
    </section>
  {/if}
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
  .dim {
    color: var(--muted);
    margin-left: 6px;
  }
  .hostlink {
    background: none;
    border: 0;
    padding: 0;
    font: inherit;
    color: var(--blue);
    cursor: pointer;
  }
  .hostlink:hover {
    text-decoration: underline;
  }
  .shot {
    display: block;
    max-width: 100%;
    border: 1px solid var(--line);
  }
  .files {
    list-style: none;
    margin: 0;
    padding: 0;
    /* Une livrée de circuit peut porter des centaines de fichiers : la liste
       défile dans son bloc au lieu d'allonger la page indéfiniment. */
    max-height: 320px;
    overflow-y: auto;
  }
  .files li {
    display: flex;
    gap: 12px;
    padding: 2px 0;
    font-size: 11.5px;
  }
  .fname {
    flex: 1;
    min-width: 0;
    color: var(--txt2);
    overflow-wrap: anywhere;
  }
  .fsize {
    flex: none;
    color: var(--muted);
  }
  .muted {
    color: var(--muted);
  }
  .small {
    font-size: 11.5px;
  }
  .errbox,
  .warnbox {
    margin-bottom: 10px;
  }
</style>
