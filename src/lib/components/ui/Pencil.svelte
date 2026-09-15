<script lang="ts">
  // Bouton crayon : « cette valeur se reprend à la main ».
  //
  // Partagé entre `InlineEdit` (nom, description, auteur) et `NoteBlock`, parce
  // qu'un même dessin doit ouvrir la même sorte de champ. Le premier jet de la
  // note affichait un ✎ **décoratif** dans son bandeau, à côté d'un crayon
  // dessiné, lui, en SVG : deux glyphes différents pour le même geste, et celui
  // de la note ne faisait rien au clic. Un pictogramme qui ressemble à un
  // bouton en est un, ou n'existe pas.
  interface Props {
    /** Infobulle et libellé lu par les lecteurs d'écran. */
    label: string;
    onclick: () => void;
  }
  let { label, onclick }: Props = $props();
</script>

<button class="pencil" type="button" {onclick} title={label} aria-label={label}>
  <!-- Crayon : le trait de la mine et le corps de l'outil. -->
  <svg viewBox="0 0 16 16" aria-hidden="true">
    <path d="M11.3 1.9 L14.1 4.7 L5.3 13.5 L1.8 14.2 L2.5 10.7 Z" fill="none" />
    <path d="M10.1 3.1 L12.9 5.9" fill="none" />
  </svg>
</button>

<style>
  .pencil {
    display: inline-flex;
    align-items: center;
    justify-content: center;
    width: 22px;
    height: 22px;
    padding: 0;
    flex: none;
    background: transparent;
    border: none;
    color: var(--muted2);
    cursor: pointer;
    /* Effacé tant qu'on ne s'y intéresse pas : c'est une action secondaire,
       elle ne doit pas concurrencer la valeur qu'elle modifie. */
    opacity: 0.55;
    transition:
      opacity 0.15s ease,
      color 0.15s ease;
  }
  .pencil:hover,
  .pencil:focus-visible {
    opacity: 1;
    color: var(--rosso-bright);
  }
  .pencil svg {
    width: 13px;
    height: 13px;
    stroke: currentColor;
    stroke-width: 1.4;
    fill: none;
  }
</style>
