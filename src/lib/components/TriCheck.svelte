<script lang="ts" module>
  /** Neutre (le filtre ne s'applique pas), inclure, exclure. */
  export type TriState = 0 | 1 | -1;
</script>

<script lang="ts">
  // Case à cocher à trois états, pour les filtres de la bibliothèque (§6.3).
  //
  // Une case booléenne ne sait dire qu'une moitié de ce qu'on veut : « favoris
  // uniquement » se cochait, « tout sauf les favoris » ne s'exprimait pas. D'où
  // trois états et non deux — et un libellé **neutre** (« Favoris », pas
  // « Masquer les favoris ») : c'est la couleur qui porte le sens, vert pour ce
  // qu'on garde, rouge pour ce qu'on écarte, comme les jetons de `TokenFilter`.
  //
  // L'infobulle dit l'état courant en toutes lettres. Elle est là parce que
  // vert et rouge ne se distinguent pas pour tout le monde, pas pour expliquer
  // la mécanique du clic.
  interface Props {
    label: string;
    value: TriState;
    /** Ce que veut dire l'état vert, en clair (« favoris uniquement »). */
    titleInclude: string;
    /** Ce que veut dire l'état rouge (« hors favoris »). */
    titleExclude: string;
    /** Ce que veut dire l'état neutre (« sans filtre »). */
    titleNeutral: string;
  }
  let { label, value = $bindable(), titleInclude, titleExclude, titleNeutral }: Props = $props();

  // Shift parcourt le cycle à l'envers : atteindre « exclure » depuis neutre
  // demande sinon deux clics, alors que c'est l'état le plus utile des trois
  // sur « contenu de base ».
  function cycle(back: boolean) {
    const order: TriState[] = back ? [0, -1, 1] : [0, 1, -1];
    value = order[(order.indexOf(value) + 1) % 3];
  }

  const title = $derived(value === 1 ? titleInclude : value === -1 ? titleExclude : titleNeutral);
</script>

<button
  class="tri"
  type="button"
  data-state={value === 1 ? "inc" : value === -1 ? "exc" : "off"}
  aria-pressed={value !== 0}
  aria-label="{label} : {title}"
  {title}
  onclick={(e) => cycle(e.shiftKey)}
>
  <span class="box" aria-hidden="true">{value === 1 ? "✓" : value === -1 ? "✕" : ""}</span>
  <span>{label}</span>
</button>

<style>
  .tri {
    display: inline-flex;
    align-items: center;
    gap: 7px;
    padding: 5px 10px 5px 7px;
    background: var(--raised);
    border: 1px solid var(--line);
    color: var(--muted);
    font-size: 12px;
    cursor: pointer;
  }
  .tri:hover {
    color: var(--txt2);
    border-color: var(--muted2);
  }
  .box {
    width: 15px;
    height: 15px;
    flex: none;
    border: 1px solid var(--muted2);
    display: flex;
    align-items: center;
    justify-content: center;
    font-size: 10px;
    line-height: 1;
    color: transparent;
  }
  /* Vert = ce qu'on garde, rouge = ce qu'on écarte. Les deux teintes viennent
     du design system (`--green`, `--rosso-bright`) : pas de couleur inventée
     pour un seul composant. */
  .tri[data-state="inc"] {
    color: var(--green);
    border-color: var(--green-border);
    background: var(--green-dim);
  }
  .tri[data-state="inc"] .box {
    border-color: var(--green);
    background: var(--green);
    color: var(--bg);
  }
  .tri[data-state="exc"] {
    color: var(--rosso-bright);
    border-color: var(--rosso-border);
    background: var(--rosso-dim);
  }
  .tri[data-state="exc"] .box {
    border-color: var(--rosso-bright);
    background: var(--rosso-bright);
    color: var(--bg);
  }
</style>
