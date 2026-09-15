<script lang="ts">
  // Sélecteur compact de livrée ou de tracé (REFONTE§7.3).
  //
  // Il remplace une grille de vignettes reléguée en bas de fiche, qu'il fallait
  // faire défiler pour atteindre — alors que c'est un **contrôle** et non de la
  // documentation : il agit sur l'image juste au-dessus et sur ce qui partira
  // en session. D'où sa place, collée sous l'aperçu (§7.2).
  //
  // Trois gestes, pour trois façons de chercher, et c'est délibéré :
  //
  //  - **les flèches** font défiler d'une livrée à l'autre en gardant l'œil sur
  //    l'aperçu. C'est le geste le plus fréquent ;
  //  - **le nom** ouvre la liste déroulante : on cherche par nom quand on le
  //    connaît ;
  //  - **« Voir les N »** déplie la grille de vignettes : beaucoup de livrées
  //    de mods s'appellent `skin_01`, et une liste de noms n'en dit alors rien.
  //    Une liste déroulante seule ne suffirait pas.
  import ImageSelectDropdown from "$lib/components/ui/ImageSelectDropdown.svelte";
  import { t } from "$lib/i18n/index.svelte";

  export interface PickerItem {
    id: string;
    name: string;
    image: string | null;
  }

  interface Props {
    /** Intitulé posé dans le champ lui-même : « LIVRÉE », « TRACÉ ».
     *
     * **Optionnel, et absent quand la carte qui l'entoure le nomme déjà.** Le
     * sélecteur vit maintenant dans une carte dont l'en-tête dit « SKINS 28 » ;
     * répéter l'intitulé dans le champ mangeait la largeur dont la ligne a
     * besoin pour tenir d'un seul tenant. */
    label?: string;
    items: PickerItem[];
    /** Index courant dans `items`. */
    index: number;
    onpick: (index: number) => void;
    /** Grille dépliée ? Repliée à chaque ouverture de fiche (§7.3) : l'état
     * appartient donc à la fiche, pas à ce composant. */
    expanded: boolean;
    ontoggle: () => void;
    /** "contain" pour un tracé (forme entière), "cover" pour une photo. */
    fit?: "cover" | "contain";
    emptyText: string;
    /** Mention accolée au compteur : « dont 1 ajouté » quand une couche apporte
     * une partie de la liste (REFONTE§7.7). Absente quand il n'y a rien à dire. */
    note?: string;
  }
  let { label, items, index, onpick, expanded, ontoggle, fit = "cover", emptyText, note }: Props = $props();

  /** Boucle plutôt que butée, comme la bande d'onglets : sur deux livrées, une
   * butée rendrait l'une des deux flèches inerte la moitié du temps. */
  function step(delta: number) {
    if (items.length < 2) return;
    onpick((index + delta + items.length) % items.length);
  }
</script>

<div class="picker">
  <ImageSelectDropdown
    {label}
    {fit}
    options={items}
    selectedId={items[index]?.id ?? null}
    placeholder={emptyText}
    {emptyText}
    onselect={(id) => {
      const i = items.findIndex((it) => it.id === id);
      if (i >= 0) onpick(i);
    }}
  />
  {#if items.length > 1}
    <span class="pos mono">{index + 1} / {items.length}{#if note} · {note}{/if}</span>
    <span class="arrows">
      <button type="button" onclick={() => step(-1)} title={t("detail.pickerPrev")} aria-label={t("detail.pickerPrev")}
        >‹</button
      >
      <button type="button" onclick={() => step(1)} title={t("detail.pickerNext")} aria-label={t("detail.pickerNext")}
        >›</button
      >
    </span>
    <button class="btn" type="button" onclick={ontoggle} aria-expanded={expanded}>
      {expanded ? t("detail.pickerCollapse") : t("detail.pickerExpand", { count: items.length })}
    </button>
  {/if}
</div>

<style>
  .picker {
    display: flex;
    align-items: center;
    gap: 8px;
    /* **Une seule ligne, quoi qu'il arrive.** Le sélecteur est désormais le
       corps d'une carte de la colonne de droite, donc il travaille dans deux
       fois moins de largeur qu'avant : sans `nowrap`, le compteur, les flèches
       et « Voir les N » partaient à la ligne — et avec eux la lecture d'un
       contrôle qui doit se saisir d'un coup d'œil. C'est la liste déroulante,
       et elle seule, qui absorbe la variation (`flex: 1; min-width: 0`). */
    flex-wrap: nowrap;
    /* Aucun retrait : la carte porte le sien (`.blk-b`). */
    padding: 0;
    min-width: 0;
  }
  /* La liste déroulante prend la place restante ; le reste de la ligne est
     dimensionné par son contenu. */
  .picker :global(.isd) {
    flex: 1;
    min-width: 0;
  }
  .pos {
    flex: none;
    color: var(--muted);
    font-size: 10.5px;
  }
  .arrows {
    flex: none;
    display: flex;
    border: 1px solid var(--line);
  }
  .arrows button {
    background: var(--panel2);
    color: var(--muted);
    font-size: 14px;
    line-height: 1;
    padding: 5px 9px;
  }
  .arrows button:first-child {
    border-right: 1px solid var(--line);
  }
  .arrows button:hover {
    color: var(--txt);
  }
  .btn {
    flex: none;
  }
</style>
