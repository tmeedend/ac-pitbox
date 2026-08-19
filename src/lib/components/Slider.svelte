<script lang="ts">
  // The one slider of the app. Label, current value, track — same shape
  // everywhere, from session settings to the 3D preview panel.
  //
  // There were four of them, all hand-rolled: the session ones (damage, fuel,
  // tyre wear, time of day) drew a square red thumb on a 3px track, while
  // Settings > Music and Settings > 3D preview left the browser's default
  // round thumb with `accent-color`. Same control, two looks — the exact
  // mechanism the "shared components" chantier is about: scoped CSS lets every
  // copy drift on its own and nothing flags it.
  //
  // Gamepad-wise there is nothing to declare: `needsEntry` in `gamepadNav.ts`
  // keys on the input type, so every range in the app — this one included —
  // is a field the cursor enters before left/right changes anything.

  interface Props {
    label: string;
    value: number;
    min: number;
    max: number;
    step?: number;
    /** Value as shown, already formatted with its unit ("100 %", "3,5 s").
     * Defaults to the raw number — a slider without a readable value is a
     * guess, not a setting. */
    display?: string;
    oninput: (value: number) => void;
    /** Explanation under the track (Settings). Absent in a panel where the
     * result is right there under the eye. */
    hint?: string;
    /** Tighter spacing, for a panel laid over the thing being adjusted. */
    compact?: boolean;
  }
  const { label, value, min, max, step = 1, display, oninput, hint, compact = false }: Props = $props();

  // Filled part of the track, as a percentage of the range — computed here
  // rather than passed in. Every call site used to work it out by hand
  // (`fuel_rate / 2` for a 0-200 range), which is one open-coded division per
  // slider and a wrong fill the day a bound moves.
  const fill = $derived(max > min ? ((value - min) / (max - min)) * 100 : 0);
</script>

<div class="slider" class:compact>
  <label>
    <span class="head">
      <span class="name lbl-key">{label}</span>
      <span class="value mono">{display ?? value}</span>
    </span>
    <input
      type="range"
      {min}
      {max}
      {step}
      {value}
      style:--f="{fill}%"
      oninput={(e) => oninput(Number(e.currentTarget.value))}
    />
  </label>
  {#if hint && !compact}<p class="hint">{hint}</p>{/if}
</div>

<style>
  .slider {
    min-width: 0;
  }
  label {
    display: block;
  }
  .head {
    display: flex;
    align-items: center;
    gap: 8px;
    margin-bottom: 4px;
  }
  /* Colour/size/tracking come from `.lbl-key` (global): only what it does not
     cover stays here. */
  .name {
    text-transform: uppercase;
  }
  .value {
    margin-left: auto;
    font-size: 11px;
    color: var(--txt);
  }
  /* The browser's native thumb is round, at odds with the app's rectangular
     theme — redrawn as a square, which is what the session sliders already
     did and what everything else now inherits. */
  input[type="range"] {
    width: 100%;
    height: 20px;
    margin: 0;
    appearance: none;
    background: transparent;
  }
  .compact input[type="range"] {
    height: 16px;
  }
  input[type="range"]::-webkit-slider-runnable-track {
    height: 3px;
    background: linear-gradient(
      to right,
      var(--rosso) 0%,
      var(--rosso) var(--f, 0%),
      var(--line) var(--f, 0%),
      var(--line) 100%
    );
  }
  input[type="range"]::-webkit-slider-thumb {
    appearance: none;
    width: 10px;
    height: 20px;
    border-radius: 2px;
    background: var(--rosso);
    border: 2px solid var(--panel);
    cursor: pointer;
    margin-top: -8.5px;
  }
  .compact input[type="range"]::-webkit-slider-thumb {
    height: 16px;
    margin-top: -6.5px;
  }
  .hint {
    margin-top: 6px;
    font-size: 11px;
    color: var(--faint);
  }
</style>
