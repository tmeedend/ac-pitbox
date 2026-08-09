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
     * `"left"` aligne le bord gauche de la bulle sur celui du déclencheur. */
    align?: "center" | "left";
    children: Snippet;
  }
  let { text, align = "center", children }: Props = $props();
</script>

<span class="tt-wrap">
  {@render children()}
  <span class="tt-bubble" class:align-left={align === "left"} role="tooltip">{text}</span>
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
     débordement/rognage contre un bord étroit (ex. barre latérale, 222px).
     `max-width` réduit en plus, sinon la bulle grandit vers la droite et se
     fait quand même rogner par le contenu central — quitte à passer sur
     plus de lignes dans un espace aussi étroit. */
  .tt-bubble.align-left {
    left: 0;
    transform: none;
    max-width: 170px;
  }
</style>
