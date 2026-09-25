<script lang="ts">
  // --- Le type de session EST la navigation (L5§1) ----------------------
  //
  // Le bouton « Paramétrage de la session » et le segmenté « Type de session »
  // de l'écran de réglages ont disparu tous les deux : ils disaient la même
  // chose à deux endroits, et aucun des deux ne disait ce que l'app sait
  // faire. Les quatre types sont désormais une liste, toujours dépliée — c'est
  // la seule chose de cette colonne qui ne se replie pas.
  //
  // Un clic fait deux gestes en un : il choisit le type **et** ouvre ses
  // réglages. Le second est ce qui remplace le bouton supprimé.
  import {
    SESSION_TYPES,
    hasOpponents,
    openOpponentsPage,
    openSetupPage,
    pickSessionType,
    sessionNav,
  } from "$lib/shell/sessionNav.svelte";
  import { nav, requestSection } from "$lib/shell/nav.svelte";
  import type { SessionType } from "$lib/launch/launch";
  import { t } from "$lib/i18n/index.svelte";

  async function goToType(type: SessionType) {
    pickSessionType(type);
    await requestSection("race");
  }
  /** La sous-entrée. Elle sert aussi de retour : depuis la page adversaires,
   * cliquer le type parent ramène aux réglages sans changer de type. */
  async function goToOpponents() {
    openOpponentsPage();
    await requestSection("race");
  }
  function backToSetup(type: SessionType) {
    if (type !== sessionNav.type) {
      void goToType(type);
      return;
    }
    openSetupPage();
    void requestSection("race");
  }
  /** `6 AI · 87% ± 3` (L5§1.2). Ce n'est pas un ornement : sans elle on ne peut
   * plus savoir combien d'adversaires on affronte sans changer de page, alors
   * qu'on peut lancer la session sans y être allé. */
  const opponentsSummary = $derived(
    t("session.opponentsSummary", {
      count: sessionNav.count,
      center: sessionNav.center,
      spread: sessionNav.spread,
    }),
  );
  /**
   * Deux marques, deux choses différentes — et c'est ce qui permet au filet
   * rouge de cohabiter avec celui du rail sans dire la même chose que lui.
   *
   * - **Choisi** (libellé en pleine lumière) : c'est le type qui partira, quel
   *   que soit l'écran qu'on regarde. La colonne répond toujours à « qu'est-ce
   *   que je lance ? », comme la voiture et le circuit au-dessus — et sans
   *   cette marque, la sous-entrée « Adversaires » pendait sous quatre lignes
   *   identiques, sans qu'on voie à laquelle elle appartenait.
   * - **Ouvert** (filet rouge d'attaque) : on est en train de le regarder. Ça,
   *   c'est un repère d'écran actif, et il ne s'allume que sur l'écran de
   *   session.
   */
  const onSessionScreen = $derived(nav.section === "race");
  const typeChosen = (type: SessionType) => sessionNav.type === type;
  const typeSelected = (type: SessionType) =>
    onSessionScreen && sessionNav.type === type && sessionNav.page === "setup";
</script>

<!-- LE TYPE DE SESSION EST LA NAVIGATION (L5§1). Les quatre types
     sont toujours visibles, jamais repliés derrière un sélecteur : ils
     annoncent ce que l'application sait faire, et c'est la seule chose
     de cette colonne qui ne se replie pas quand la hauteur manque.

     La sous-entrée « Adversaires » ne paraît que sous Course et Track
     day, et seulement quand ce type est sélectionné — en Essais et en
     Hotlap, la liste fait quatre lignes. -->
<nav class="types" aria-label={t("nav.session")}>
  {#each SESSION_TYPES as type (type)}
    <button
      class="type"
      class:chosen={typeChosen(type)}
      class:on={typeSelected(type)}
      class:parent={onSessionScreen && sessionNav.type === type && sessionNav.page === "opponents"}
      type="button"
      onclick={() => (sessionNav.page === "opponents" ? backToSetup(type) : void goToType(type))}
      >{t(`launch.type.${type}`)}</button
    >
    {#if sessionNav.type === type && hasOpponents(type)}
      <!-- Indentée, et ce qui la rend lisible comme une descente est le
           filet vertical qui la rattache à son parent : sans lui, deux
           entrées de même gabarit à quelques pixels d'écart se lisent
           comme deux destinations sœurs. -->
      <button
        class="type sub"
        class:on={onSessionScreen && sessionNav.page === "opponents"}
        type="button"
        onclick={() => void goToOpponents()}
      >
        <span class="sub-n">{t("launch.opponentsLabel")}</span>
        <!-- Le résumé passe en rouge et porte un marqueur quand la page
             adversaires porte une alerte (L5§1.3) : une alerte sur une
             page qu'on ne regarde pas ne vaut pas mieux que pas
             d'alerte. -->
        <span class="sub-v" class:alert={sessionNav.alert}
          >{#if sessionNav.alert}<span aria-hidden="true">⚠ </span>{/if}{opponentsSummary}</span
        >
      </button>
    {/if}
  {/each}
</nav>

<style>
  /* --- La liste des types de session (L5§1) ---------------------------
     Même langage que le rail de navigation : le repos est en retrait, l'entrée
     retenue s'éclaircit et prend un filet rouge sur son bord d'attaque —
     niveau 2 du barème (SPEC §7.2ter), jamais un fond plein, qui reste au seul
     bouton de lancement. */
  .types {
    display: flex;
    flex-direction: column;
    margin-bottom: 4px;
  }
  .type {
    position: relative;
    display: flex;
    align-items: baseline;
    gap: 8px;
    /* **Pas de `width: 100%`** : la colonne est un `flex` vertical, ses enfants
       s'étirent déjà d'eux-mêmes. Posée en dur, cette largeur s'ajoutait au
       `margin-left` de la sous-entrée — une marge vit hors de la boîte, même en
       `border-box` — et la colonne débordait de quatorze pixels. `.side` étant
       en `overflow-y: auto`, une règle CSS lui calcule l'autre axe en `auto`
       aussi : d'où une barre de défilement horizontale pour quatorze pixels. */
    padding: 6px 10px;
    background: none;
    border: none;
    color: var(--muted);
    font-size: 12px;
    text-align: left;
  }
  .type:hover {
    color: var(--txt2);
    background: var(--panel2);
  }
  /* Le type qui partira : lisible en pleine lumière, sans rouge — c'est une
     valeur, pas un écran. */
  .type.chosen {
    color: var(--txt);
  }
  .type.on::before {
    content: "";
    position: absolute;
    left: 0;
    top: 3px;
    bottom: 3px;
    width: 2px;
    background: var(--rosso);
  }
  /* Le type qui porte la sous-entrée ouverte : en retrait d'un cran par
     rapport à celle-ci, mais toujours lisible — c'est lui qu'on clique pour
     remonter. */
  .type.parent {
    color: var(--txt2);
  }
  /* **L'indentation seule ne dit pas « descente »** — c'est la faiblesse connue
     de cette structure, et le filet vertical est ce qui la corrige : la
     sous-entrée est visiblement accrochée au type au-dessus d'elle, donc
     cliquer celui-ci se lit comme une remontée. */
  .sub {
    flex-direction: column;
    align-items: stretch;
    gap: 1px;
    margin-left: 14px;
    padding-left: 12px;
    border-left: 1px solid var(--line);
    font-size: 11px;
  }
  .sub.on {
    border-left-color: var(--rosso);
  }
  /* Le filet d'attaque appartient au type, pas à sa sous-entrée : celle-ci a
     déjà le sien, à gauche, et deux traits rouges à 14 px l'un de l'autre se
     liraient comme deux sélections. */
  .sub.on::before {
    display: none;
  }
  .sub-v {
    font-size: 9.5px;
    color: var(--muted);
    font-family: var(--mono);
  }
  /* Une alerte vivant sur la page adversaires (L5§1.3). Niveau 2 du barème : le
     libellé passe en rouge, rien de plein. */
  .sub-v.alert {
    color: var(--rosso-bright);
  }
</style>
