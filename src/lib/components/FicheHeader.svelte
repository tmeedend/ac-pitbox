<script lang="ts">
  // En-tête de fiche, le même pour TOUS les types (REFONTE§6.1).
  //
  // Avant lui, cinq fiches, cinq anatomies : la fiche voiture avait une tuile,
  // un nom éditable, un sous-titre et un menu ⋮ ; celles d'app, de mod
  // « autre », de son et de pack affichaient un **nom de fichier en
  // monospace** suivi d'une rangée de boutons. Le titre d'une fiche y était
  // donc l'identifiant technique — `policeman__ext_config.ini` — c'est-à-dire
  // la seule chose qu'on ne cherche pas en ouvrant une fiche.
  //
  // Trois règles que ce composant impose par construction :
  //
  //  1. **La tuile montre la chose réelle** quand elle existe (badge de marque,
  //     tracé, vignette) et un pictogramme de TYPE sinon. Jamais deux lettres
  //     tirées du nom : « PO » pour `policeman` n'apprend rien et ressemble à
  //     un badge de marque qui n'existe pas.
  //  2. **Le nom est repris à la main sur tous les types**, pas seulement les
  //     voitures (§5bis.3 étendu) — ce sont justement les autres qui en ont le
  //     plus besoin.
  //  3. **Un seul vocabulaire d'état** (§12), rendu ici et nulle part ailleurs.
  //
  // Les actions passent dans le ⋮, comme la fiche voiture l'a fait avant les
  // autres (§6.3) : une rangée de boutons dans un en-tête coûte toute la
  // largeur du titre pour des gestes qu'on fait une fois par mod.
  import type { Snippet } from "svelte";
  import ContextMenu from "./ContextMenu.svelte";
  import InlineEdit from "./InlineEdit.svelte";
  import StateBadge from "./StateBadge.svelte";
  import { t } from "$lib/i18n/index.svelte";

  export interface FicheAction {
    label: string;
    onclick: () => void;
    disabled?: boolean;
    danger?: boolean;
  }

  /** Déploiement (§12), la seule des deux notions d'état qui vit dans
   * l'en-tête — la provenance (contenu de base, importé le…) appartient à la
   * carte Origine. Nommée `deployment` et pas `state` : Svelte 5 refuse un
   * `state` local, qu'il confondrait avec la rune `$state`. */
  export interface FicheDeployment {
    active: boolean;
    stock?: boolean;
    unmanaged?: boolean;
    pending?: boolean;
  }

  export interface FicheRename {
    /** Nom tel que l'annonce le fichier, pour proposer d'y revenir. */
    original: string | null;
    overridden: boolean;
    onsave: (value: string | null) => void;
  }

  interface Props {
    onback: () => void;
    /** Infobulle du ← : « Retour aux apps », « Retour à la bibliothèque ». */
    backLabel: string;
    /** Pictogramme du type, montré quand il n'y a pas d'image. */
    glyph: string;
    /** La chose réelle : badge de marque, tracé, vignette. */
    image?: string | null;
    imageAlt?: string;
    /** Nom d'affichage. Jamais l'identifiant technique (§6.1). */
    name: string;
    /** Sous-titre lisible : marque · année · par auteur. */
    subtitle?: string;
    /** Seul tag qui fait un travail — la composition de plateau (§7.5). */
    category?: string | null;
    rename?: FicheRename;
    deployment?: FicheDeployment;
    favorite?: { on: boolean; ontoggle: () => void };
    actions?: FicheAction[];
    /** Bande pleine largeur posée sur le fond de la page (fiche voiture, qui
     * déborde les marges de l'écran). Par défaut l'en-tête est transparent et
     * suit la marge de la page qui le contient — même distinction que le
     * `flush` de `Tabs.svelte`, et pour la même raison : l'anatomie est la
     * même partout, le contenant ne l'est pas. */
    flush?: boolean;
    /** Le **seul** contrôle qu'une fiche peut garder visible dans son en-tête,
     * quand il est la raison d'être de l'écran : la clé de contact d'un mod de
     * son. Tout le reste passe par le ⋮ — deux exceptions, et l'en-tête
     * redevient la rangée de boutons qu'on vient d'en retirer. */
    control?: Snippet;
  }

  let {
    onback,
    backLabel,
    glyph,
    image,
    imageAlt,
    name,
    subtitle,
    category,
    rename,
    deployment,
    favorite,
    actions,
    flush = false,
    control,
  }: Props = $props();

  let menuPos = $state<{ x: number; y: number } | null>(null);

  function openMenu(e: MouseEvent) {
    // Sans ça, ce même clic bulle jusqu'à `document` juste après le montage de
    // `ContextMenu` (son propre listener `click` de fermeture) et referme le
    // menu dans la foulée — il s'ouvrait et se refermait dans le même geste,
    // invisible à l'œil (bug réel : le survol fonctionnait, le clic semblait
    // « ne rien faire »).
    e.stopPropagation();
    // Ancré sous le bouton, pas au curseur : un menu d'en-tête se rattache à
    // ce qui l'ouvre. `ContextMenu` attend des pixels réels de fenêtre et
    // divise lui-même par le zoom — `getBoundingClientRect` en rend, comme
    // `clientX` (voir `zoom.svelte.ts`).
    const rect = (e.currentTarget as HTMLElement).getBoundingClientRect();
    menuPos = { x: rect.left, y: rect.bottom + 4 };
  }
</script>

<header class="head" class:flush>
  <button class="back" type="button" onclick={onback} title={backLabel} aria-label={backLabel}>←</button>
  {#if image}
    <img class="tile tile-img" src={image} alt={imageAlt ?? ""} />
  {:else}
    <span class="tile" aria-hidden="true">{glyph}</span>
  {/if}
  <div class="title">
    <div class="t-name">
      <span class="t-name-txt">{name}</span>
      {#if rename}
        <!-- Le repère montré pendant l'édition est le nom du FICHIER : `name`
             porte déjà la surcharge quand il y en a une, il ne dirait donc pas
             à quoi on reviendrait. -->
        <InlineEdit
          value={name}
          original={rename.overridden ? rename.original : null}
          overridden={rename.overridden}
          label={t("detail.renameLabel")}
          onsave={rename.onsave}
        />
      {/if}
      {#if category}<span class="cat">{category}</span>{/if}
    </div>
    {#if subtitle}<div class="t-meta mono">{subtitle}</div>{/if}
  </div>
  <div class="actions">
    {#if control}{@render control()}{/if}
    {#if deployment}
      <StateBadge
        active={deployment.active}
        stock={deployment.stock ?? false}
        unmanaged={deployment.unmanaged ?? false}
        pending={deployment.pending ?? false}
      />
    {/if}
    {#if favorite}
      <button class="fav" class:on={favorite.on} type="button" onclick={favorite.ontoggle} title={t("common.favorite")}>
        {favorite.on ? "♥" : "♡"}
      </button>
    {/if}
    {#if actions?.length}
      <button class="kebab" type="button" onclick={openMenu} title={t("detail.moreActions")}>
        <span class="kebab-dot"></span><span class="kebab-dot"></span><span class="kebab-dot"></span>
      </button>
    {/if}
  </div>
</header>

{#if menuPos && actions?.length}
  <ContextMenu x={menuPos.x} y={menuPos.y} items={actions} onclose={() => (menuPos = null)} />
{/if}

<style>
  .head {
    display: flex;
    align-items: center;
    gap: 12px;
    padding-bottom: 12px;
    border-bottom: 1px solid var(--line);
    margin-bottom: 14px;
  }
  .head.flush {
    padding: 12px 18px;
    background: var(--panel2);
    margin-bottom: 0;
  }
  .back {
    background: transparent;
    color: var(--muted);
    font-size: 18px;
    line-height: 1;
    padding: 2px 8px;
  }
  .back:hover {
    color: var(--txt);
  }
  .tile {
    width: 30px;
    height: 30px;
    background: var(--rosso);
    display: flex;
    align-items: center;
    justify-content: center;
    color: #fff;
    font-size: 15px;
    line-height: 1;
    flex: none;
  }
  .tile-img {
    background: var(--panel2);
    border: 1px solid var(--line);
    object-fit: contain;
    padding: 3px;
  }
  .title {
    min-width: 0;
    /* Prend la place disponible : en édition, le champ de saisie a besoin
       d'une largeur utile plutôt que de la seule largeur du nom. */
    flex: 1;
  }
  .t-name {
    font-size: 14px;
    font-weight: 600;
    /* 1.2 et pas 1.1 : à 1.1 la boîte de ligne est plus courte que la fonte,
       et les jambages descendants se font rogner en bas (le « g » de
       « Mugello » — retour utilisateur direct). */
    line-height: 1.2;
    /* Le crayon se pose au bout du nom, sur la même ligne de base. En édition,
       `InlineEdit` remplace le crayon par son champ : la colonne du titre
       s'élargit alors au lieu de pousser le nom hors cadre. */
    display: flex;
    align-items: center;
    gap: 6px;
    min-width: 0;
  }
  .t-name-txt {
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }
  .cat {
    color: var(--rosso-bright);
    font-family: var(--mono);
    font-size: 10px;
    font-weight: 400;
    letter-spacing: 0.5px;
    flex: none;
  }
  .t-meta {
    color: var(--muted);
    font-size: 10px;
    margin-top: 2px;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }
  .actions {
    margin-left: auto;
    display: flex;
    align-items: center;
    gap: 10px;
  }
  .fav {
    background: transparent;
    color: var(--txt2);
    font-size: 18px;
    line-height: 1;
  }
  .fav:hover,
  .fav.on {
    color: var(--rosso-bright);
  }
  /* Icône construite en CSS (3 carrés empilés) plutôt qu'un glyphe Unicode —
     le rendu du caractère « ⋮ » dépend trop de la police (fin, peu lisible,
     cible de clic minuscule dans certains cas). */
  .kebab {
    background: transparent;
    display: flex;
    flex-direction: column;
    align-items: center;
    justify-content: center;
    gap: 3px;
    padding: 4px 7px;
  }
  .kebab-dot {
    width: 4px;
    height: 4px;
    background: var(--txt2);
    border-radius: 1px;
  }
  .kebab:hover .kebab-dot {
    background: var(--rosso-bright);
  }
</style>
