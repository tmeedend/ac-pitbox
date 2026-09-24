<script lang="ts">
  // La ligne qui NOMME un mod : logo, marque, modèle, année — d'un seul tenant.
  //
  // **Un composant et non un style recopié** (chantier « composants partagés »,
  // CLAUDE.md) : la carte de bibliothèque et les deux blocs de la colonne de
  // session doivent montrer exactement la même chose, et le CSS de Svelte étant
  // scopé par composant, deux copies auraient dérivé sans que rien ne le
  // signale.
  //
  // **Une seule taille et une seule couleur pour la marque, le modèle et
  // l'année.** Elles avaient été séparées sur deux lignes, deux tailles et
  // trois gris : ce ne sont pourtant pas des informations moins sûres les unes
  // que les autres, juste des morceaux d'un même nom. Réunies, elles se lisent
  // comme la voiture se nomme — « Nissan Skyline GT-R R34 » — et le logo en
  // tête sert d'ancre pour le balayage. La marque n'apparaît qu'ici : le nom
  // reçu en est déjà privé (`withoutBrand`), sans quoi elle s'écrirait deux
  // fois.
  //
  // L'année est poussée au bord droit de la même ligne : elle suit bien le
  // modèle, mais s'aligne aussi d'une carte à l'autre, donc se compare en
  // colonne — ce qu'un « … R34 1999 » au fil du texte interdirait, en plus de
  // se lire comme la fin du nom du modèle.
  //
  // La taille vient de l'appelant par `--ident-size` (une variable CSS, seul
  // moyen pour un parent d'atteindre l'intérieur d'un composant scopé) : la
  // grille dense la veut petite, la confortable et la colonne de session
  // grande.
  import type { Snippet } from "svelte";
  import { previewSrc } from "$lib/library/library";
  import { isPlaque } from "$lib/library/brandLogos.svelte";
  import Emblem from "$lib/components/ui/Emblem.svelte";

  interface Props {
    /** Nom du modèle, marque déjà retirée par l'appelant. */
    name: string;
    /** Chemin du `ui/badge.png`, tel que la base le range — résolu ici. */
    badge?: string | null;
    brand?: string | null;
    year?: number | null;
    /** Éteint la ligne avec le reste du bloc (carte inutilisable). */
    dim?: boolean;
    /**
     * Occuper la place même sans rien à écrire.
     *
     * Vrai dans la colonne de session, où la marque arrive un aller-retour
     * après le reste (`getModDetail`) : sans réserve, le bloc sauterait d'un
     * cran au moment où elle tombe.
     */
    reserve?: boolean;
    /** Posé après le nom, hors de la boîte qui tronque — le ⚠ « mod non
     * activé » de la colonne de session. */
    after?: Snippet;
  }
  let { name, badge = null, brand = null, year = null, dim = false, reserve = false, after }: Props = $props();

  const empty = $derived(!name && !badge && !brand && !year);

  /** Ce que la ligne écrit, d'un seul tenant — et ce que l'info-bulle répète
   * quand il est coupé. Bâti ici plutôt que relu dans le DOM : l'info-bulle
   * doit dire le nom, pas ce qui se trouve avoir atterri dans l'élément. */
  const label = $derived(brand ? `${brand} ${name}` : name);

  /**
   * Info-bulle sur le nom, **et seulement s'il est coupé**.
   *
   * Un `title` posé systématiquement volerait celui de la carte (« double-clic
   * pour ouvrir la fiche ») sur toute la surface du nom, pour ne rien ajouter
   * dans l'immense majorité des cas où il tient. On ne le pose donc que quand
   * le texte déborde réellement — ce qui dépend de la largeur de colonne autant
   * que du nom, d'où l'observateur de redimensionnement plutôt qu'une mesure
   * unique au montage.
   */
  const clipSync = new WeakMap<Element, () => void>();
  const clipObserver =
    typeof ResizeObserver === "undefined"
      ? null
      : new ResizeObserver((entries) => {
          for (const e of entries) clipSync.get(e.target)?.();
        });

  // Le paramètre sert deux fois : il porte l'info-bulle, et son changement
  // (renommage, bascule « masquer la marque ») rappelle `update` — sans quoi
  // la mesure resterait celle du premier rendu.
  function titleIfClipped(node: HTMLElement, text: string) {
    const sync = (next = text) => {
      text = next;
      // Le +1 absorbe les largeurs fractionnaires : sous zoom d'interface,
      // `scrollWidth` dépasse `clientWidth` d'un demi-pixel sur du texte qui
      // n'est pas coupé, ce qui collerait une info-bulle partout.
      if (node.scrollWidth > node.clientWidth + 1) node.title = text;
      else node.removeAttribute("title");
    };
    sync();
    clipSync.set(node, sync);
    clipObserver?.observe(node);
    return {
      update: sync,
      destroy() {
        clipObserver?.unobserve(node);
        clipSync.delete(node);
      },
    };
  }
</script>

{#if !empty || reserve}
  <div class="line" class:dim>
    <!-- The car's own badge (TAXO§3: the car level, never curated), on the
         light plate when its background is baked in (TAXO§5). -->
    {#if badge}<span class="badge"><Emblem src={previewSrc(badge) ?? ""} plaque={isPlaque(badge)} size={13} /></span>{/if}
    <span class="text" use:titleIfClipped={label}>{label}</span>
    <!-- Hors de la boîte qui tronque : un ⚠ « mod non activé » avalé par une
         ellipse serait exactement l'avertissement qu'on ne voit pas. -->
    {@render after?.()}
    {#if year}<span class="year">{year}</span>{/if}
  </div>
{/if}

<style>
  .line {
    display: flex;
    align-items: center;
    font-size: var(--ident-size, 12.5px);
    /* Réservée même quand tout manque : sans hauteur minimale, un bloc sans
       année ni marque remonte d'un cran et casse l'alignement du haut des
       cartes, qui est ce qu'on lit en balayant une grille. En `em` pour suivre
       la taille que l'appelant impose. */
    min-height: 1.3em;
    /* L'écart sous la vignette, le même pour tous les appelants — c'est
       justement ce genre de valeur qu'une copie fait diverger. */
    margin-top: 8px;
    line-height: 1.3;
    letter-spacing: 0.01em;
    white-space: nowrap;
    overflow: hidden;
  }
  .dim {
    color: var(--muted);
  }
  /* `0 1 auto` et non `1 1 auto` : le nom prend sa largeur naturelle et ne
     rétrécit que s'il le faut, ce qui laisse la place libre à la marge
     automatique de l'année. En `flex: 1`, il mangerait tout et l'année se
     collerait à lui. */
  .text {
    flex: 0 1 auto;
    min-width: 0;
    overflow: hidden;
    text-overflow: ellipsis;
  }
  /* Jamais tronquée : quatre chiffres valent bien les quatre derniers
     caractères d'un nom, qui a l'ellipse pour le dire. */
  .year {
    flex: none;
    margin-left: auto;
    padding-left: 8px;
  }
  .badge {
    flex: none;
    display: inline-flex;
    margin-right: 5px;
  }
</style>
