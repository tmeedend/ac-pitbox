<script lang="ts">
  // Onglets de premier niveau, partagés par TOUS les écrans qui en ont : fiche
  // détail, Réglages, Add-ons voiture/circuit, Règles de tags.
  //
  // Avant ce composant, trois écrans avaient chacun leur `.tabs` local — trois
  // tailles de police, trois façons de marquer l'onglet actif, trois fonds
  // différents. Le CSS Svelte étant scopé par composant, chaque copie dérivait
  // de son côté sans que personne ne le voie : c'est exactement le mécanisme
  // qui a produit 53 signatures visuelles pour 68 libellés (§chantier
  // libellés). Un seul composant = une seule apparence, par construction.
  //
  // Il s'inscrit auprès de `screenActions` tant qu'il est monté : c'est ce qui
  // permet aux boutons « onglet précédent/suivant » de la manette de le
  // parcourir sans que la manette ait à connaître l'écran affiché.
  import type { Snippet } from "svelte";
  import { registerTabStrip } from "$lib/screenActions";

  export interface TabItem {
    id: string;
    label: string;
    /** Décompte affiché à droite du libellé. Absent = pas de décompte. */
    count?: number;
    /** Onglet visible mais inatteignable : ni cliquable, ni parcouru à la
     * manette. Sert à montrer une catégorie vide sans la faire disparaître —
     * une bande d'onglets qui change de taille selon le contenu se relit
     * entièrement à chaque visite. */
    disabled?: boolean;
  }

  interface Props {
    tabs: TabItem[];
    active: string;
    onselect: (id: string) => void;
    /** Bande pleine largeur posée sur le fond des cartes, pour un écran qui
     * occupe tout le cadre (fiche détail). Par défaut la bande est
     * transparente et suit la marge de l'écran qui la contient. */
    flush?: boolean;
    /** Contenu poussé à droite de la bande, sur la même ligne que les onglets
     * (l'état du mod sur la fiche détail). Rendu ici plutôt que posé en
     * absolu par l'appelant : la bande garde son fond et son trait sur toute
     * la largeur, et l'alignement vertical est celui des onglets par
     * construction. */
    trailing?: Snippet;
  }
  let { tabs, active, onselect, flush = false, trailing }: Props = $props();

  // Boucle plutôt que butée : avec deux onglets, une butée rendrait l'un des
  // deux boutons de la manette inerte la moitié du temps. Les onglets
  // désactivés sont sautés — la manette ne montre pas qu'un onglet est grisé,
  // elle le traverserait sans que rien ne change à l'écran.
  $effect(() =>
    registerTabStrip((delta) => {
      const reachable = tabs.filter((tab) => !tab.disabled);
      const i = reachable.findIndex((tab) => tab.id === active);
      if (i === -1 || reachable.length < 2) return;
      onselect(reachable[(i + delta + reachable.length) % reachable.length].id);
    }),
  );
</script>

<nav class="tabs" class:flush>
  {#each tabs as tab (tab.id)}
    <button
      class:on={active === tab.id}
      type="button"
      disabled={tab.disabled}
      onclick={() => onselect(tab.id)}
    >
      {tab.label}{#if tab.count !== undefined}<span class="n">{tab.count}</span>{/if}
    </button>
  {/each}
  {#if trailing}
    <span class="trailing">{@render trailing()}</span>
  {/if}
</nav>

<style>
  .tabs {
    display: flex;
    gap: 1px;
    border-bottom: 1px solid var(--line);
    /* L'écart au contenu appartient à la bande, pas à chaque écran : c'est
       une valeur de plus qui divergeait d'un écran à l'autre. */
    margin-bottom: 20px;
  }
  /* Variante pleine largeur (fiche détail) : même fond que les cartes de
     contenu en dessous — `--line` ou `--panel2` y créaient une bande
     visiblement plus claire/plus sombre au-dessus du contenu (deux retours
     utilisateur distincts, l'un sur le conteneur, l'autre sur les boutons).
     Le contenu y porte déjà sa propre marge (`.tab-body`). */
  .tabs.flush {
    background: var(--card);
    padding: 0 18px;
    margin-bottom: 0;
  }
  .tabs button {
    padding: 10px 16px;
    background: transparent;
    color: var(--muted);
    font-size: 11px;
    letter-spacing: 0.5px;
    border-bottom: 2px solid transparent;
  }
  .tabs button.on {
    color: var(--txt);
    border-bottom-color: var(--rosso);
  }
  .tabs button:hover:not(.on):not(:disabled) {
    color: var(--txt2);
  }
  .tabs button:disabled {
    color: var(--faint);
    cursor: default;
  }
  /* Décompte : plus discret que le libellé, jamais au point de disparaître. */
  .tabs button .n {
    color: var(--muted2);
    font-family: var(--mono);
    margin-left: 6px;
  }
  .tabs button.on .n {
    color: var(--muted);
  }
  .trailing {
    margin-left: auto;
    display: flex;
    align-items: center;
    gap: 10px;
    font-size: 11px;
    /* Aligné sur le libellé des onglets, pas sur leur boîte : ceux-ci portent
       un liseré bas de 2px que rien ici ne doit compenser. */
    padding: 0 4px 2px;
  }
</style>
