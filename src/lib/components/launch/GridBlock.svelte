<script lang="ts">
  // Le **plateau** : une ligne par adversaire, ses cellules éditables, et les
  // deux gestes qui le sauvegardent.
  //
  // **La sortie du générateur, pas son enfant.** Le plateau vivait dans un cadre
  // à l'intérieur du cadre « Adversaires ». Les deux sont pourtant deux objets —
  // c'est toute la conception du CIBLE§3.3 : le filtre définit le vivier, jamais le
  // plateau, et il faut un geste explicite pour passer de l'un à l'autre.
  //
  // Depuis le lot 5 les deux ont leur **page**, en pleine largeur, et plus aucun
  // cadre entre eux : le générateur en haut, le plateau dessous, dans un seul
  // enchaînement. La hauteur de la table est plafonnée à une dizaine de lignes
  // avec défilement interne (L5§5.3) — c'est le seul défilement imbriqué autorisé
  // dans l'app, et une table de données est précisément le composant pour lequel
  // cette convention existe : sans lui, la hauteur de la page serait fonction du
  // nombre d'IA, et le cas normal d'une course GT3 en aligne 24.
  //
  // Presentation only: generation and the livery cache live in
  // `$lib/launch/opponentGrid.svelte.ts`, which `Launch.svelte` also drives
  // from elsewhere (presets, saved sessions). The rows themselves are
  // `OpponentTable`, which a story's event grid shares.
  import type { CardIndex } from "$lib/library/filters";
  import type { Nationality, Opponent, RaceSetup, SkinItem } from "$lib/launch/launch";
  import type { ModCard } from "$lib/library/library";
  import { t } from "$lib/i18n/index.svelte";
  import OpponentTable from "./OpponentTable.svelte";

  let {
    setup,
    carPool,
    skinsByCarId,
    index,
    poolCount,
    nationalityList,
    duplicateDrivers,
    onchoose,
    onregenerate,
    onremove,
    onduplicate,
    onsetlevel,
    onsetcell,
    onsavegrid,
    onloadgrid,
    onopenpicker,
  }: {
    setup: RaceSetup;
    carPool: ModCard[];
    skinsByCarId: Record<string, SkinItem[]>;
    index: CardIndex;
    /** Le vivier, pour le libellé de « Choisir dans le vivier · N voitures » :
     * c'est le geste qui relie les deux blocs. */
    poolCount: number;
    /** Les nationalités que le jeu connaît, avec leur drapeau. */
    nationalityList: Nationality[];
    /** Deux pilotes sous la même identité (SETUP§1.9). Calculé par l'écran et non
     * ici depuis que l'alerte doit aussi remonter sur l'entrée de navigation
     * (L5§1.3) : une seule source, deux lecteurs. */
    duplicateDrivers: boolean;
    onchoose: () => void;
    onregenerate: () => void;
    onremove: (index: number) => void;
    onduplicate: (index: number) => void;
    onsetlevel: (index: number, level: number | null) => void;
    onsetcell: (index: number, patch: Partial<Opponent>) => void;
    onsavegrid: () => void;
    onloadgrid: () => void;
    onopenpicker: (index: number) => void;
  } = $props();
</script>

<section class="grid-blk">
  <!-- Un problème de configuration qui appelle une action, donc le jaune est à
       sa place ici — contrairement à l'explication de la force, passée au ⓘ. Il
       vit avec le plateau, qui est ce qui le produit. -->
  {#if duplicateDrivers}
    <p class="warnbox dup">{t("launch.duplicateDrivers")}</p>
  {/if}

  <div class="oppo">
    <!-- `Regenerate` is NOT `Fill` with another name: it keeps the cars and
         re-rolls what was drawn on them (skin, strength), where `Fill` draws
         the cars themselves. Two gestures one actually wants separately — the
         grid is right but the liveries repeat, versus the grid is wrong. -->
    <div class="oppo-h lbl">
      <span>{t("launch.gridHeader", { count: setup.opponents.length })}</span>
      <!-- La position de départ a rejoint SESSION OPTIONS (SETUP§2.5) : elle dépend
           du type de session, et tout ce qui en dépend vit là-bas. -->
      <span class="oppo-sp"></span>
      <button class="oppo-regen" type="button" disabled={!setup.opponents.length} onclick={onregenerate}
        >{t("launch.regenerateGrid")}</button
      >
    </div>

    <OpponentTable
      opponents={setup.opponents}
      {carPool}
      {skinsByCarId}
      {index}
      {nationalityList}
      {onremove}
      {onduplicate}
      {onsetlevel}
      {onsetcell}
      {onopenpicker}
    />
    <!-- The pool count in the label is the point: it says what the filter
         bought — this many rows to read instead of the whole library. -->
    <button class="oppo-add" type="button" disabled={poolCount === 0} onclick={onchoose}
      >{poolCount === 1 ? t("launch.chooseFromPoolOne") : t("launch.chooseFromPool", { count: poolCount })}</button
    >

    <!-- A grid is worth saving on its own, apart from the session that holds
         it: the same GT3 field on ten tracks (§5). -->
    <div class="oppo-foot">
      <button class="oppo-regen" type="button" disabled={!setup.opponents.length} onclick={onsavegrid}
        >{t("launch.saveGrid")}</button
      >
      <button class="oppo-regen" type="button" onclick={onloadgrid}>{t("launch.loadGrid")}</button>
    </div>
  </div>
</section>

<style>
  .dup {
    margin-bottom: 10px;
  }
  .oppo-foot {
    display: flex;
    gap: 7px;
    padding: 7px 10px;
    border-top: 1px solid var(--line);
  }
  /* Pushes the start position and `Regenerate` to the right of the title. */
  .oppo-sp {
    flex: 1;
  }
  /* Compteur d'adversaires, tirage et difficulté : tout sur une ligne (retombe
     seulement si la largeur manque). */
  .oppo {
    border: 1px solid var(--line);
  }
  /* Couleur/taille/interlettrage/majuscules viennent de `.lbl` (global,
     harmonisation §chantier libellés) : ne reste ici que le fond en bandeau
     et l'annulation de la marge basse (`.lbl` en prévoit une pour une
     rubrique de carte, pas pour un bandeau suivi directement des lignes). */
  .oppo-h {
    background: var(--raised);
    padding: 6px 10px;
    margin-bottom: 0;
    /* `.lbl` est déjà en flex : reste à écarter les deux bouts. L'espace du
       milieu attend la position de départ (L3). */
    justify-content: space-between;
    gap: 10px;
  }
  /* Bouton secondaire neutre : régénérer le plateau n'est pas ce qui est
     retenu pour la session, donc pas de rouge (§7.2ter). L'action existait
     déjà comme effet de bord d'un changement de voiture — sans aucun moyen de
     la demander. */
  .oppo-regen {
    background: transparent;
    border: 1px solid var(--line);
    color: var(--muted);
    font-size: 10px;
    letter-spacing: 0.1em;
    text-transform: uppercase;
    padding: 4px 10px;
  }
  .oppo-regen:hover:not(:disabled) {
    background: var(--panel2);
    color: var(--txt2);
  }
  .oppo-regen:disabled {
    color: var(--faint2);
    cursor: not-allowed;
  }
  /* Action secondaire : aucun niveau du barème ne couvre un libellé rouge
     (§7.2ter), et le rouge de cet écran doit rester au bouton de lancement. */
  .oppo-add {
    background: var(--panel2);
    padding: 9px 12px;
    border-top: 1px solid var(--line);
    color: var(--txt2);
    font-size: 11.5px;
    text-align: left;
    width: 100%;
  }
  .oppo-add:hover {
    background: var(--raised);
  }
</style>
