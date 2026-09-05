<script lang="ts">
  // Ligne d'identification d'une voiture : logo et marque à gauche, année
  // poussée au bord droit.
  //
  // **Un composant et non un style recopié** (chantier « composants partagés »,
  // CLAUDE.md) : la carte de bibliothèque et le bloc voiture de la colonne de
  // session doivent montrer exactement la même chose, et le CSS de Svelte étant
  // scopé par composant, deux copies auraient dérivé sans que rien ne le
  // signale.
  //
  // L'année au bord droit plutôt qu'accolée à la marque : elle s'aligne alors
  // d'une carte à l'autre et se compare en colonne, ce qu'un « Nissan · 1999 »
  // au fil du texte interdisait. C'est cet alignement qui remplace le
  // séparateur, d'où l'absence de « · ».
  //
  // Une seule couleur pour la marque, l'année et le modèle qui suit : ce ne
  // sont pas des informations moins sûres que le nom, juste d'une autre
  // nature. La hiérarchie tient à la taille et à la place, pas à l'extinction.
  import { previewSrc } from "$lib/library";

  interface Props {
    /** Chemin du `ui/badge.png`, tel que la base le range — résolu ici. */
    badge?: string | null;
    brand?: string | null;
    year?: number | null;
    /** Éteint la ligne avec le reste du bloc (carte inutilisable). */
    dim?: boolean;
    /**
     * Occuper la place même sans rien à écrire.
     *
     * Faux pour une carte de bibliothèque : un circuit n'a ni marque ni année,
     * et la ligne y laisserait une bande vide. Vrai dans la colonne de session,
     * où la voiture est seule et où la marque arrive un aller-retour après le
     * reste (`getModDetail`) — sans réserve, le nom sauterait d'un cran au
     * moment où elle tombe.
     */
    reserve?: boolean;
  }
  let { badge = null, brand = null, year = null, dim = false, reserve = false }: Props = $props();

  const empty = $derived(!badge && !brand && !year);
</script>

{#if !empty || reserve}
  <div class="sub" class:dim>
    {#if badge}<img class="badge" src={previewSrc(badge)} alt="" loading="lazy" />{/if}
    {#if brand}<span class="brand">{brand}</span>{/if}
    {#if year}<span class="year">{year}</span>{/if}
  </div>
{/if}

<style>
  .sub {
    display: flex;
    align-items: center;
    /* Réservée même quand l'année manque : sans hauteur minimale, une voiture
       sans millésime remonte son nom d'une ligne et casse l'alignement du haut
       des blocs, qui est ce qu'on lit en balayant une grille. */
    min-height: 15px;
    /* L'écart sous la vignette, le même pour les deux appelants — c'est
       justement ce genre de valeur qu'une copie fait diverger. */
    margin-top: 8px;
    font-size: 10.5px;
    letter-spacing: 0.02em;
    white-space: nowrap;
    overflow: hidden;
  }
  .dim {
    color: var(--muted);
  }
  .brand {
    overflow: hidden;
    text-overflow: ellipsis;
  }
  .year {
    margin-left: auto;
    padding-left: 8px;
  }
  .badge {
    flex: none;
    width: 13px;
    height: 13px;
    object-fit: contain;
    margin-right: 4px;
  }
</style>
