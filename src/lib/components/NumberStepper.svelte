<script lang="ts">
  // Champ numérique avec flèches ▲▼ intégrées au thème (remplace les flèches
  // natives du navigateur, blanches et hors charte sur cette app). Les
  // flèches natives sont masquées via `appearance` ; ces boutons pilotent la
  // même valeur avec les mêmes bornes.
  interface Props {
    value: number;
    min?: number;
    max?: number;
    step?: number;
    /** "field" (~32px, champs de formulaire standard) | "compact" (~20px, listes denses). */
    variant?: "field" | "compact";
    width?: number;
    disabled?: boolean;
    title?: string;
    class?: string;
    /** Valeur qui signifie « rien » : le champ s'affiche **vide** quand il la
     * porte, et vider le champ la rétablit (§6.2, fourchette d'année de la
     * bibliothèque — « pas de borne de ce côté »).
     *
     * Une sentinelle plutôt qu'un `null` : `value` reste un `number` pour tous
     * les autres appelants, qui n'ont aucune raison de devenir nullables.
     * Elle échappe volontairement aux bornes (voir `clamp`), sinon `min`
     * la ramènerait aussitôt dans la plage — c'est exactement le bug qu'on
     * corrige ici : vider « année min » écrivait 1950. */
    emptyValue?: number;
    /** Valeur sur laquelle atterrissent ▲ **et** ▼ au premier appui depuis un
     * champ vide (§6.2bis) — le même repère quel que soit le sens, comme
     * taper directement cette valeur. Sans elle, ▲ retombait sur `min` même
     * pour un champ dont le point de départ naturel est ailleurs (année max
     * de la bibliothèque : l'année courante, pas 1950) et ▼ n'avait aucune
     * destination définie, d'où sa désactivation forcée tant que le champ
     * était vide — corrigée ici, une fois qu'il en a une. */
    emptyStart?: number;
    onchange?: (value: number) => void;
  }
  let {
    value = $bindable(),
    min,
    max,
    step = 1,
    variant = "field",
    width,
    disabled = false,
    title,
    class: cls = "",
    emptyValue,
    emptyStart,
    onchange,
  }: Props = $props();

  let inputEl: HTMLInputElement | null = null;

  const isEmpty = (v: number) => emptyValue != null && v === emptyValue;
  const display = (v: number) => (isEmpty(v) ? "" : String(v));

  function clamp(v: number): number {
    if (isEmpty(v)) return v;
    let n = v;
    if (min != null) n = Math.max(min, n);
    if (max != null) n = Math.min(max, n);
    return n;
  }
  function commit(v: number) {
    const c = clamp(v);
    value = c;
    // Le champ peut afficher autre chose que ce qu'on retient : saisie hors
    // bornes, ou champ vidé alors que la valeur ne bouge pas. Svelte ne
    // réécrit l'attribut que si `value` a changé — sans cette
    // resynchronisation, le texte tapé restait à l'écran en contradiction avec
    // l'état réel (bug constaté : vider une deuxième fois laissait le champ
    // vide alors que le filtre valait toujours 1950).
    if (inputEl && inputEl.value !== display(c)) inputEl.value = display(c);
    onchange?.(c);
  }
  function onInput(e: Event) {
    const raw = (e.currentTarget as HTMLInputElement).value.trim();
    // Champ vidé : la sentinelle si l'appelant en a une, sinon on s'en tient
    // à la valeur courante (`Number("")` vaut 0, qui passait le test
    // `!Number.isNaN` et écrasait silencieusement la valeur).
    if (raw === "") {
      commit(emptyValue ?? value);
      return;
    }
    const n = Number(raw);
    if (!Number.isNaN(n)) commit(n);
  }
  // Depuis « vide », les deux flèches atterrissent au même repère
  // (`emptyStart`, ou `min` à défaut) — seul le sens diffère ensuite, une
  // fois que la valeur n'est plus vide.
  const inc = () => commit(isEmpty(value) ? (emptyStart ?? min ?? value + step) : value + step);
  const dec = () => commit(isEmpty(value) ? (emptyStart ?? min ?? value - step) : value - step);
</script>

<div
  class="nstep {variant} {cls}"
  class:disabled
  style={width != null ? `--nstep-w:${width}px` : undefined}
>
  <input
    bind:this={inputEl}
    class="nstep-input mono"
    type="number"
    {min}
    {max}
    {step}
    {disabled}
    {title}
    value={display(value)}
    onchange={onInput}
  />
  <div class="nstep-arrows">
    <button
      type="button"
      class="nstep-btn"
      tabindex="-1"
      disabled={disabled || (!isEmpty(value) && max != null && value >= max)}
      onclick={inc}>▲</button
    >
    <button
      type="button"
      class="nstep-btn"
      tabindex="-1"
      disabled={disabled || (!isEmpty(value) && min != null && value <= min)}
      onclick={dec}>▼</button
    >
  </div>
</div>

<style>
  .nstep {
    display: inline-flex;
    width: var(--nstep-w, 70px);
    border: 1px solid var(--line);
    background: var(--bg);
  }
  .nstep.disabled {
    opacity: 0.5;
  }
  .nstep-input {
    flex: 1;
    min-width: 0;
    background: transparent;
    border: none;
    color: var(--txt);
    text-align: center;
    font-size: 13px;
    /* Masque les flèches natives (blanches, hors charte) : remplacées par .nstep-arrows. */
    appearance: textfield;
    -moz-appearance: textfield;
  }
  .nstep-input::-webkit-outer-spin-button,
  .nstep-input::-webkit-inner-spin-button {
    appearance: none;
    -webkit-appearance: none;
    margin: 0;
  }
  .nstep-input:focus {
    outline: none;
  }
  .nstep.field {
    height: 32px;
  }
  .nstep.field .nstep-input {
    font-size: 13px;
  }
  .nstep.compact {
    height: 20px;
  }
  .nstep.compact .nstep-input {
    font-size: 9px;
  }
  .nstep-arrows {
    display: flex;
    flex-direction: column;
    flex: none;
    width: 16px;
    border-left: 1px solid var(--line);
  }
  .nstep-btn {
    flex: 1;
    min-height: 0;
    display: flex;
    align-items: center;
    justify-content: center;
    background: var(--raised);
    border: none;
    color: var(--muted);
    line-height: 1;
    padding: 0;
  }
  .nstep.field .nstep-btn {
    font-size: 7px;
  }
  .nstep.compact .nstep-btn {
    font-size: 5.5px;
  }
  .nstep-btn:not(:first-child) {
    border-top: 1px solid var(--line);
  }
  .nstep-btn:not(:disabled):hover {
    background: var(--rosso-dim);
    color: var(--rosso-bright);
  }
  .nstep-btn:disabled {
    opacity: 0.35;
    cursor: default;
  }
</style>
