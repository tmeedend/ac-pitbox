<script lang="ts">
  // Un réglage dans un bloc : son intitulé, sa commande, son explication —
  // et surtout **l'écart avec le réglage précédent**, posé une fois ici.
  //
  // Pourquoi un composant pour si peu : le CSS Svelte est scopé, donc chaque
  // écran qui empile des réglages recopiait ses propres `.field { margin-top }`
  // et ses propres `.hint`. Il suffit alors d'oublier la marge sur un champ
  // pour qu'il se colle au précédent — c'est exactement ce qui est arrivé à
  // « afficher le pilote au volant », posé sous une case à cocher sans rien
  // entre les deux. Le sélecteur `+` ci-dessous répond pour tout le monde :
  // deux champs qui se suivent s'écartent, le premier ne prend rien.
  //
  // L'intitulé est facultatif : une case à cocher porte le sien à sa droite, et
  // en poser un second au-dessus dirait deux fois la même chose.
  import type { Snippet } from "svelte";

  interface Props {
    /** Intitulé posé au-dessus de la commande. Omis pour une case à cocher. */
    label?: string;
    /** Phrase d'explication sous la commande. */
    hint?: string;
    /** La commande elle-même : `<select>`, `<Slider>`, groupe de radios… */
    children: Snippet;
  }

  const { label, hint, children }: Props = $props();
</script>

<div class="field">
  {#if label}<span class="blk-sub">{label}</span>{/if}
  {@render children()}
  {#if hint}<p class="hint">{hint}</p>{/if}
</div>

<style>
  /* Rien sur le premier champ : c'est le bloc qui pose son propre rembourrage,
     et une marge en tête l'y ajouterait une seconde fois. C'est donc le champ
     **suivant** qui prend l'écart.
     `:global` sur la moitié droite, et il le faut : les deux `.field` sont
     deux *instances* de ce composant, ce que l'analyse de portée de Svelte ne
     voit pas — sans lui la règle est signalée inutilisée et retirée du
     bundle. */
  .field + :global(.field) {
    margin-top: 20px;
  }
  .hint {
    margin: 6px 0 0;
    font-size: 11.5px;
    color: var(--muted);
    line-height: 1.5;
  }
</style>
