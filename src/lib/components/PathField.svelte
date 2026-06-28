<script lang="ts">
  import { open } from "@tauri-apps/plugin-dialog";
  import type { Check } from "$lib/config";

  interface Props {
    label: string;
    value: string | null;
    kind: "file" | "dir";
    placeholder?: string;
    optional?: boolean;
    hint?: string;
    filterName?: string;
    filterExt?: string[];
    check?: Check | null;
  }

  let {
    label,
    value = $bindable(),
    kind,
    placeholder = "",
    optional = false,
    hint = "",
    filterName,
    filterExt,
    check = null,
  }: Props = $props();

  async function browse() {
    const selected = await open({
      directory: kind === "dir",
      multiple: false,
      filters:
        kind === "file" && filterExt
          ? [{ name: filterName ?? "Fichiers", extensions: filterExt }]
          : undefined,
    });
    if (typeof selected === "string") value = selected;
  }
</script>

<div class="field">
  <div class="row1">
    <span class="label">{label}</span>
    {#if optional}<span class="opt">optionnel</span>{/if}
  </div>
  <div class="row2">
    <input
      class="input mono"
      type="text"
      {placeholder}
      bind:value
      spellcheck="false"
    />
    <button class="btn" type="button" onclick={browse}>Parcourir…</button>
  </div>
  {#if check}
    <div class="status {check.ok ? 'ok' : check.level === 'optional' ? 'warn' : 'err'}">
      {check.message}
    </div>
  {:else if hint}
    <div class="hint">{hint}</div>
  {/if}
</div>

<style>
  .field {
    margin-bottom: 16px;
  }
  .row1 {
    display: flex;
    align-items: baseline;
    gap: 8px;
    margin-bottom: 5px;
  }
  .label {
    font-size: 11px;
    font-weight: 600;
    letter-spacing: 0.5px;
    color: var(--txt2);
    text-transform: uppercase;
  }
  .opt {
    font-size: 9.5px;
    letter-spacing: 1px;
    color: var(--faint);
    text-transform: uppercase;
  }
  .row2 {
    display: flex;
    gap: 8px;
  }
  .row2 .input {
    flex: 1;
  }
  .status,
  .hint {
    margin-top: 5px;
    font-size: 11px;
  }
  .hint {
    color: var(--muted);
  }
  .status.ok {
    color: var(--green);
  }
  .status.err {
    color: var(--rosso-bright);
  }
  .status.warn {
    color: var(--yellow);
  }
</style>
