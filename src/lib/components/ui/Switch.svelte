<script lang="ts">
  // The on/off switch of a rule or an entry of the catalogue: a plain track
  // and knob, grey off, light on - no red (nothing it switches belongs to the
  // session, §7.2ter). Written three times before this brick, in Rules,
  // Brands and Families › Tracks.
  interface Props {
    on: boolean;
    /** What is switched, for the screen reader (the switch has no text). */
    label?: string;
    /** Points the switch at a visible label instead of `label`. */
    labelledby?: string;
    disabled?: boolean;
    onclick: () => void;
  }
  let { on, label, labelledby, disabled = false, onclick }: Props = $props();
</script>

<button
  type="button"
  class="switch"
  role="switch"
  aria-checked={on}
  aria-label={labelledby ? undefined : label}
  aria-labelledby={labelledby}
  {disabled}
  {onclick}><span></span></button
>

<style>
  .switch {
    flex: none;
    width: 26px;
    height: 14px;
    padding: 0;
    border: 1px solid var(--faint);
    border-radius: 8px;
    background: var(--panel2);
    position: relative;
    cursor: pointer;
  }
  .switch span {
    position: absolute;
    top: 2px;
    left: 2px;
    width: 8px;
    height: 8px;
    border-radius: 50%;
    background: var(--muted2);
    transition: left 0.12s;
  }
  .switch[aria-checked="true"] {
    border-color: var(--muted);
    background: var(--mat);
  }
  .switch[aria-checked="true"] span {
    left: 14px;
    background: var(--txt);
  }
  .switch:disabled {
    cursor: default;
  }
  .switch:focus-visible {
    outline: 2px solid var(--rosso);
    outline-offset: 2px;
  }
</style>
