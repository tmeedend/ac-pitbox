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
  import { registerTabStrip } from "$lib/screenActions";

  export interface TabItem {
    id: string;
    label: string;
  }

  interface Props {
    tabs: TabItem[];
    active: string;
    onselect: (id: string) => void;
    /** Bande pleine largeur posée sur le fond des cartes, pour un écran qui
     * occupe tout le cadre (fiche détail). Par défaut la bande est
     * transparente et suit la marge de l'écran qui la contient. */
    flush?: boolean;
  }
  let { tabs, active, onselect, flush = false }: Props = $props();

  // Boucle plutôt que butée : avec deux onglets, une butée rendrait l'un des
  // deux boutons de la manette inerte la moitié du temps.
  $effect(() =>
    registerTabStrip((delta) => {
      const i = tabs.findIndex((tab) => tab.id === active);
      if (i === -1 || tabs.length < 2) return;
      onselect(tabs[(i + delta + tabs.length) % tabs.length].id);
    }),
  );
</script>

<nav class="tabs" class:flush>
  {#each tabs as tab (tab.id)}
    <button class:on={active === tab.id} type="button" onclick={() => onselect(tab.id)}>
      {tab.label}
    </button>
  {/each}
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
  .tabs button:hover:not(.on) {
    color: var(--txt2);
  }
</style>
