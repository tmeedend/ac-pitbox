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
    onchange,
  }: Props = $props();

  function clamp(v: number): number {
    let n = v;
    if (min != null) n = Math.max(min, n);
    if (max != null) n = Math.min(max, n);
    return n;
  }
  function commit(v: number) {
    const c = clamp(v);
    value = c;
    onchange?.(c);
  }
  function onInput(e: Event) {
    const raw = Number((e.currentTarget as HTMLInputElement).value);
    if (!Number.isNaN(raw)) commit(raw);
  }
  const inc = () => commit(value + step);
  const dec = () => commit(value - step);
</script>

<div
  class="nstep {variant} {cls}"
  class:disabled
  style={width != null ? `--nstep-w:${width}px` : undefined}
>
  <input class="nstep-input mono" type="number" {min} {max} {step} {disabled} {title} value={value} onchange={onInput} />
  <div class="nstep-arrows">
    <button type="button" class="nstep-btn" tabindex="-1" disabled={disabled || (max != null && value >= max)} onclick={inc}>▲</button>
    <button type="button" class="nstep-btn" tabindex="-1" disabled={disabled || (min != null && value <= min)} onclick={dec}>▼</button>
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
