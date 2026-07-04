<script lang="ts">
  import PathField from "./PathField.svelte";
  import type { AppConfig, ConfigValidation } from "$lib/config";
  import { t } from "$lib/i18n/index.svelte";

  interface Props {
    config: AppConfig;
    validation: ConfigValidation | null;
  }

  let { config = $bindable(), validation }: Props = $props();
</script>

<PathField
  label={t("pathfield.acInstall")}
  kind="dir"
  bind:value={config.ac_install_path}
  placeholder="…\steamapps\common\assettocorsa"
  hint={t("pathfield.acInstallHint")}
  check={validation?.ac_install}
/>

<!-- Sous-vérifications dérivées du dossier AC -->
{#if validation && (!validation.content_dir.ok || !validation.content_writable.ok)}
  <div class="subchecks">
    {#if !validation.content_dir.ok}
      <div class="sub err">{t(validation.content_dir.message)}</div>
    {/if}
    {#if validation.content_dir.ok && !validation.content_writable.ok}
      <div class="sub err">{t(validation.content_writable.message)}</div>
    {/if}
  </div>
{/if}

<PathField
  label={t("pathfield.library")}
  kind="dir"
  bind:value={config.library_path}
  placeholder="ex. D:\AC-Library"
  hint={t("pathfield.libraryHint")}
  check={validation?.library}
/>

<PathField
  label={t("pathfield.contentManager")}
  kind="file"
  filterName={t("pathfield.executable")}
  filterExt={["exe"]}
  bind:value={config.content_manager_exe}
  placeholder="…\Content Manager.exe"
  check={validation?.content_manager}
/>

<PathField
  label="7-Zip"
  kind="file"
  filterName={t("pathfield.executable")}
  filterExt={["exe"]}
  bind:value={config.sevenzip_exe}
  placeholder="…\7-Zip\7z.exe"
  check={validation?.sevenzip}
/>

<div class="optional-block">
  <div class="opt-head">{t("pathfield.optionalBlockHint")}</div>
  <PathField
    label="QuickBMS"
    kind="file"
    optional
    filterName={t("pathfield.executable")}
    filterExt={["exe"]}
    bind:value={config.quickbms_exe}
    placeholder="…\quickbms.exe"
    check={validation?.quickbms}
  />
  <PathField
    label={t("pathfield.acdScript")}
    kind="file"
    optional
    filterName={t("pathfield.bmsScript")}
    filterExt={["bms"]}
    bind:value={config.acd_bms_script}
    placeholder="…\acd.bms"
  />
</div>

<style>
  .subchecks {
    margin: -10px 0 16px;
    padding-left: 10px;
    border-left: 2px solid var(--rosso-border);
  }
  .sub {
    font-size: 11px;
    margin: 2px 0;
  }
  .sub.err {
    color: var(--rosso-bright);
  }
  .optional-block {
    margin-top: 22px;
    padding-top: 16px;
    border-top: 1px solid var(--line);
  }
  .opt-head {
    font-size: 10.5px;
    letter-spacing: 0.5px;
    color: var(--muted);
    margin-bottom: 14px;
  }
</style>
