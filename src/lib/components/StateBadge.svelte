<script lang="ts">
  // État d'un mod : actif, inactif, ou contenu de base (§6.1).
  //
  // Une seule définition pour la colonne « État » du tableau de bibliothèque
  // et pour la fiche détail : c'est la même information, elle doit se lire
  // pareil aux deux endroits. Le tableau affichait un tiret pour « inactif »
  // — une absence, là où l'utilisateur cherche un état — et rien ne
  // distinguait le contenu de base d'un mod actif.
  //
  // La couleur porte la distinction, le libellé porte l'état :
  //   vert   = mod actif           orange = mod inactif
  //   bleu   = contenu de base Kunos (toujours présent dans le jeu, il ne
  //            s'active ni ne se désactive — d'où une couleur à lui plutôt
  //            que le vert des mods qu'on a soi-même déployés, et un
  //            libellé « De base » plutôt que « Actif » : c'est vrai
  //            techniquement (`c.active` vaut aussi vrai pour lui, c'est
  //            d'ailleurs pour ça que le filtre « Actif » remonte le
  //            contenu de base sans rien y changer ici), mais ce n'est pas
  //            l'information que cette pastille doit donner)
  import { t } from "$lib/i18n/index.svelte";

  let { active, stock }: { active: boolean; stock: boolean } = $props();
</script>

<span
  class="state"
  class:stock
  class:off={!stock && !active}
  title={stock ? t("library.stockTooltip") : undefined}
>
  <span class="dot"></span>{stock ? t("common.stockState") : active ? t("common.active") : t("common.inactive")}
</span>

<style>
  .state {
    display: inline-flex;
    align-items: center;
    gap: 5px;
    white-space: nowrap;
    color: var(--txt2);
  }
  .dot {
    flex: none;
    width: 7px;
    height: 7px;
    border-radius: 50%;
    background: var(--green);
  }
  .off .dot {
    background: var(--orange);
  }
  .stock .dot {
    background: var(--blue);
  }
  .off {
    color: var(--muted);
  }
</style>
