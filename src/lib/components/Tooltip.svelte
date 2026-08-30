<script lang="ts">
  // Info-bulle réutilisable : plusieurs écrans avaient fini par improviser leur
  // propre explication (attribut `title` natif, trop court pour un texte à
  // plusieurs lignes, ou tout simplement absent). CSS pur (`:hover`/`:focus-within`,
  // pas de JS) : marche au survol comme au clavier (Tab), et ne peut pas rester
  // « coincée » ouverte comme le ferait un état posé par un clic. Le focus
  // clavier vient du contenu (`children`) lui-même — toujours un élément
  // interactif (bouton, lien…), jamais du wrapper, qui reste neutre pour
  // l'accessibilité (`:focus-within` se déclenche dès que l'enfant est
  // focus, pas besoin d'un tabindex ici en plus).
  import type { Snippet } from "svelte";

  interface Props {
    /** Texte de la bulle — `\n` produit un retour à la ligne (`white-space: pre-line`). */
    text: string;
    /** `"center"` (défaut) centre la bulle sous/sur le déclencheur — déborde
     * si le déclencheur est près d'un bord étroit (ex. barre latérale).
     * `"left"` aligne le bord gauche de la bulle sur celui du déclencheur.
     * `"right"` aligne son bord droit dessus (la bulle grandit vers la
     * gauche) — pour un déclencheur près du bord droit (ex. dernières
     * colonnes d'un tableau, rognées par le panneau de droite sinon, bug
     * réel constaté sur la colonne « Date de publication »). */
    align?: "center" | "left" | "right";
    /** `"top"` (défaut) affiche la bulle au-dessus du déclencheur. `"bottom"` —
     * en-tête de tableau `position: sticky` collé en haut d'un conteneur
     * `overflow-y: auto` (§6.2) : une bulle au-dessus déborderait hors de la
     * zone visible du scroll et serait purement et simplement rognée, jamais
     * affichée malgré `opacity`/`visibility` (bug réel constaté — ⓘ des
     * en-têtes de colonnes dates). */
    side?: "top" | "bottom";
    children: Snippet;
  }
  let { text, align = "center", side = "top", children }: Props = $props();
</script>

<span class="tt-wrap">
  {@render children()}
  <span
    class="tt-bubble"
    class:align-left={align === "left"}
    class:align-right={align === "right"}
    class:side-bottom={side === "bottom"}
    role="tooltip"
  >{text}</span>
</span>

<style>
  .tt-wrap {
    position: relative;
    display: inline-flex;
    align-items: center;
  }
  .tt-bubble {
    position: absolute;
    bottom: calc(100% + 7px);
    left: 50%;
    transform: translateX(-50%);
    width: max-content;
    max-width: 280px;
    background: var(--panel);
    border: 1px solid var(--rosso-border);
    color: var(--txt2);
    font-size: 11px;
    /* Composant générique : ne doit jamais hériter la typo du déclencheur
       (bug réel constaté — ⓘ d'en-tête de tableau, où `<th>` impose gras +
       majuscules + interlettrage par défaut du navigateur/du CSS de la
       page). Toujours réinitialisés explicitement, quel que soit l'endroit
       où la bulle est posée. */
    font-weight: 400;
    text-transform: none;
    letter-spacing: normal;
    line-height: 1.5;
    padding: 8px 10px;
    white-space: pre-line;
    box-shadow: 0 6px 18px rgba(0, 0, 0, 0.5);
    z-index: 200;
    opacity: 0;
    visibility: hidden;
    pointer-events: none;
    transition: opacity 0.12s;
  }
  .tt-wrap:hover .tt-bubble,
  .tt-wrap:focus-within .tt-bubble {
    opacity: 1;
    visibility: visible;
  }
  /* Bord gauche aligné sur le déclencheur plutôt que centré : évite le
     débordement/rognage contre un bord étroit (ex. barre latérale, 328px).
     `max-width` réduit en plus, sinon la bulle grandit vers la droite et se
     fait quand même rogner par le contenu central — quitte à passer sur
     plus de lignes dans un espace aussi étroit. */
  .tt-bubble.align-left {
    left: 0;
    transform: none;
    max-width: 170px;
  }
  /* Symétrique d'align-left : la bulle grandit vers la gauche depuis le bord
     droit du déclencheur, jamais vers la droite où elle serait rognée. */
  .tt-bubble.align-right {
    left: auto;
    right: 0;
    transform: none;
    max-width: 220px;
  }
  .tt-bubble.side-bottom {
    bottom: auto;
    top: calc(100% + 7px);
  }
</style>
