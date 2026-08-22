<script lang="ts">
  import { open } from "@tauri-apps/plugin-dialog";
  import type { Check } from "$lib/config";
  import { t } from "$lib/i18n/index.svelte";

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
    {#if optional}<span class="opt">{t("pathfield.optional")}</span>{/if}
  </div>
  <div class="row2">
    <input
      class="input mono"
      type="text"
      {placeholder}
      bind:value
      spellcheck="false"
    />
    <button class="btn" type="button" onclick={browse}>{t("pathfield.browse")}</button>
  </div>
  <!-- Les deux, et dans cet ordre. L'aide et la vérification ne répondent pas
       à la même question : « à quoi sert ce champ ? » d'un côté, « ce que j'ai
       tapé convient-il ? » de l'autre. Elles étaient exclusives (`:else if`),
       donc l'aide disparaissait dès qu'une vérification remontait — c'est-à-dire
       toujours, la détection automatique renseignant les chemins avant même
       que l'écran ne s'affiche. Résultat : une aide écrite, traduite, et que
       personne n'a jamais lue. -->
  {#if hint}
    <div class="hint">{hint}</div>
  {/if}
  {#if check}
    <div class="status {check.ok ? 'ok' : check.level === 'optional' ? 'warn' : 'err'}">
      {t(check.message)}
    </div>
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
