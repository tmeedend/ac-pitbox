<script lang="ts">
  // État d'un mod : actif, inactif, contenu de base, ou non géré (§6.1).
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
  //   gris   = mod installé hors Pit Box (§12bis.1bis) : présent dans le jeu,
  //            donc chargé, mais l'app ne le gère pas — ni activation, ni
  //            couche, ni écriture. Gris et pas jaune : sur une install déjà
  //            moddée il y en a des centaines, et elles fonctionnent.
  import { t } from "$lib/i18n/index.svelte";

  // `unmanaged` l'emporte sur `stock` : les deux sont vrais ensemble côté base
  // (un mod non géré vit dans content/ comme le contenu de jeu, §12bis.1bis),
  // et c'est bien « non géré » qu'il faut lire dans ce cas.
  let { active, stock, unmanaged = false }: { active: boolean; stock: boolean; unmanaged?: boolean } = $props();
</script>

<span
  class="state"
  class:stock={stock && !unmanaged}
  class:unmanaged
  class:off={!stock && !active}
  title={unmanaged ? t("library.unmanagedTooltip") : stock ? t("library.stockTooltip") : undefined}
>
  <span class="dot"></span>{unmanaged
    ? t("common.unmanagedState")
    : stock
      ? t("common.stockState")
      : active
        ? t("common.active")
        : t("common.inactive")}
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
  /* Gris : « hors périmètre, l'app n'y touche pas » — ni le vert de ce qu'on a
     déployé, ni le bleu du contenu de jeu, ni l'alerte du jaune : une install
     moddée qui marche n'est pas un problème à régler. */
  .unmanaged .dot {
    background: var(--muted);
  }
  .off {
    color: var(--muted);
  }
</style>
