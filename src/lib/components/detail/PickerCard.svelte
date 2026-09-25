<script lang="ts">
  // Picker card of the detail page: the skins of a car or the layouts of a
  // track (REFONTE§7.3). One component for both — they were two near-identical
  // copies of the same card, the same grid and the same hundred lines of CSS.
  //
  // Le sélecteur de livrée est une carte de la colonne de données, pas une barre nue
  // sous l'aperçu : posé entre la fiche technique et le son, il en reprend le
  // cadre et l'en-tête rouge. Un contrôle n'a pas à se distinguer de ses
  // voisins pour rester un contrôle — ce qui le désigne comme tel, c'est qu'il
  // agisse, pas qu'il détonne. L'intitulé quitte le champ : l'en-tête le dit
  // déjà.
  import PickerBar from "./PickerBar.svelte";
  import { t } from "$lib/i18n/index.svelte";

  export interface PickerCardItem {
    id: string;
    name: string;
    /** Image of the compact bar (a livery swatch, a layout outline). */
    thumb: string | null;
    /** Image of the unfolded grid, where there is room for a full photo. */
    image: string | null;
    /** `livery.png` swatch overlaid on the grid cell (skins only). */
    livery?: string | null;
    /** Origin badge: this entry is brought by a layer (REFONTE§7.7). */
    origin?: { label: string; title: string } | null;
  }

  interface Props {
    title: string;
    /** `null` when the mod has nothing to pick from at all (a track without
     * track data): the card keeps its header and shows an empty body, as it
     * did before the two cards were merged. */
    items: PickerCardItem[] | null;
    index: number;
    onpick: (index: number) => void;
    expanded: boolean;
    ontoggle: () => void;
    emptyText: string;
    /** Tooltip of a grid cell. */
    cellTitle: string;
    /** "contain" for a layout outline (whole shape), "cover" for a photo. */
    fit?: "cover" | "contain";
    note?: string;
  }
  let { title, items, index, onpick, expanded, ontoggle, emptyText, cellTitle, fit = "cover", note }: Props =
    $props();

  /** Cases vides à ajouter en fin de grille (§ correctif damier) : `.skins`
   * est une vraie grille CSS à colonnes fixes, donc la dernière ligne
   * incomplète laisse des cellules sans le moindre élément dedans — rien n'y
   * peint la couleur de carte (`--panel2`), c'est le fond de la grille
   * (`--card`, la couleur DERRIÈRE les cartes) qui s'y voit à la place (bug
   * réel signalé). Une case fantôme, muette pour tout le monde (souris,
   * clavier, lecteur d'écran), comble le trou avec la bonne couleur plutôt
   * que de la laisser transparente. */
  const GRID_COLUMNS = 3;
  function gridFillerCount(n: number): number {
    return (GRID_COLUMNS - (n % GRID_COLUMNS)) % GRID_COLUMNS;
  }
</script>

<section class="blk">
  <header class="blk-h">
    <span class="blk-t">{title}</span>
    {#if items?.length}<span class="blk-n">{items.length}</span>{/if}
  </header>
  <div class="blk-b pick-b">
    {#if items}
      <PickerBar
        {fit}
        items={items.map((it) => ({ id: it.id, name: it.name, image: it.thumb }))}
        {index}
        {onpick}
        {expanded}
        {ontoggle}
        {emptyText}
        {note}
      />
      {#if expanded && items.length}
        <div class="skins">
          {#each items as it, i (it.id)}
            <button class="skin" class:preview={i === index} onclick={() => onpick(i)} title={cellTitle}>
              <div class="skin-img" class:layout-img={fit === "contain"}>
                {#if it.image}<img src={it.image} alt={it.name} loading="lazy" />{:else}<span class="skin-noimg"
                    >▦</span
                  >{/if}
                <!-- `livery.png` (SESSION§1) : couleurs/motif du skin seul, en
                     complément de la photo de la voiture — jamais sur la
                     grande image du skin sélectionné (heroImg), juste ici
                     dans la grille de choix. -->
                {#if it.livery}<img class="skin-livery" src={it.livery} alt="" loading="lazy" />{/if}
                {#if i === index}<span class="skin-apercu mono">{t("library.sessionBadge")}</span>{/if}
                <!-- Marque d'origine (REFONTE§7.7) : ce tracé n'est pas dans le mod,
                     c'est une couche qui l'apporte. -->
                {#if it.origin}<span class="skin-from mono" title={it.origin.title}>{it.origin.label}</span>{/if}
              </div>
              <div class="skin-b"><span class="skin-name">{it.name}</span></div>
            </button>
          {/each}
          {#each Array.from({ length: gridFillerCount(items.length) }) as _}
            <div class="skin-filler" aria-hidden="true"></div>
          {/each}
        </div>
      {/if}
    {/if}
  </div>
</section>

<style>
  /* Corps de la carte d'un sélecteur : la ligne de contrôle, puis la grille
     dépliée. Retrait un peu plus serré que `.blk-b` — ce corps est une ligne
     d'outils, pas un paragraphe. */
  .pick-b {
    padding: 12px;
  }
  .pick-b .skins {
    margin-top: 12px;
  }
  .skins {
    display: grid;
    grid-template-columns: repeat(3, 1fr);
    gap: 1px;
    /* Fond de page dans l'interligne de 1px entre vignettes, comme `.row`
       de DetailPage — pas une couleur de carte (bug réel signalé : `--line`,
       trop clair, y ressortait comme un gris qui ne se voyait nulle part
       ailleurs). */
    background: var(--card);
    border: 1px solid var(--line);
  }
  .skin {
    /* Même fond que les autres cartes de la fiche (`.blk`, global.css) : plus
       sombre que la page, c'est ce contraste qui détache la vignette. */
    background: var(--panel2);
    padding: 0;
    text-align: left;
    cursor: pointer;
    position: relative;
  }
  /* Case fantôme de fin de grille (§ correctif damier, `gridFillerCount`) :
     même fond que `.skin`, sans rien d'interactif. */
  .skin-filler {
    background: var(--panel2);
  }
  /* Cadre du choix de session en calque par-dessus la vignette : un `outline`
     inset était peint avant les descendants positionnés (.skin-img), donc
     masqué par le tracé/la preview qui remplit la cellule. */
  .skin.preview::after {
    content: "";
    position: absolute;
    inset: 0;
    border: 2px solid var(--rosso);
    pointer-events: none;
    z-index: 2;
  }
  .skin-img {
    /* Ratio des previews AC (~16:9) : la hauteur suit la largeur de la cellule,
       au lieu d'une hauteur fixe qui rognait la voiture. */
    aspect-ratio: 16 / 9;
    display: flex;
    align-items: center;
    justify-content: center;
    border-bottom: 1px solid var(--line);
    position: relative;
    overflow: hidden;
    background: var(--bg);
  }
  .skin-img img {
    width: 100%;
    height: 100%;
    object-fit: cover;
  }
  /* Tracé de layout : afficher la forme complète (pas de recadrage). */
  .layout-img img {
    object-fit: contain;
    padding: 4px;
  }
  .skin-noimg {
    color: var(--faint);
    font-size: 16px;
  }
  /* Même gabarit que la pastille de session, l'autre coin et le bleu des
     fichiers de mod (§7.2ter) : elle informe, elle n'alerte pas. */
  .skin-from {
    position: absolute;
    top: 3px;
    right: 3px;
    background: var(--blue-dim);
    border: 1px solid var(--blue-border);
    color: var(--blue);
    font-size: 7px;
    padding: 0 3px;
  }
  .skin-apercu {
    position: absolute;
    bottom: 3px;
    left: 3px;
    background: var(--rosso);
    color: #fff;
    font-size: 7px;
    padding: 0 3px;
  }
  /* `livery.png` (SESSION§1) : coin supérieur droit, libre (le badge session est
     en bas à gauche). Bordure pour rester lisible sur une preview claire.
     Sélecteur descendant obligatoire, et pas par style : `.skin-img img`
     (0,1,1) l'emporte sur `.skin-livery` (0,1,0) quel que soit l'ordre des
     règles, donc le médaillon héritait de `width/height: 100%` et recouvrait
     la photo de la voiture — soit exactement ce que le SESSION§1 interdit. */
  .skin-img img.skin-livery {
    position: absolute;
    top: 4px;
    right: 4px;
    width: 22px;
    height: 22px;
    object-fit: cover;
    border: 1px solid var(--line);
    background: var(--bg);
  }
  .skin-b {
    padding: 5px 7px;
    display: flex;
    align-items: center;
    gap: 5px;
  }
  .skin-name {
    font-size: 10px;
    white-space: nowrap;
    overflow: hidden;
    text-overflow: ellipsis;
    flex: 1;
  }
</style>
