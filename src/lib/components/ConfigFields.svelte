<script lang="ts">
  import PathField from "./PathField.svelte";
  import Tooltip from "./Tooltip.svelte";
  import type { AppConfig, ConfigValidation } from "$lib/config";
  import { openDeveloperModeSettings } from "$lib/config";
  import { t } from "$lib/i18n/index.svelte";
  import { errorText } from "$lib/errors";

  interface Props {
    config: AppConfig;
    validation: ConfigValidation | null;
  }

  let { config = $bindable(), validation }: Props = $props();
  let devModeError = $state("");

  function openDevModeSettings() {
    devModeError = "";
    openDeveloperModeSettings().catch((e) => (devModeError = errorText(e)));
  }
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

<div class="deploy-block">
  <div class="row1"><span class="label">{t("settings.deployMode")}</span></div>

  <label class="radio-opt">
    <input type="radio" name="deploy_mode" value="hardlink" bind:group={config.prefs.deploy_mode} />
    <span>
      <span class="radio-title">{t("settings.deployHardlink")}</span>
      <span class="radio-hint">{t("settings.deployHardlinkHint")}</span>
    </span>
  </label>

  <label class="radio-opt">
    <input type="radio" name="deploy_mode" value="symlink" bind:group={config.prefs.deploy_mode} />
    <span>
      <span class="radio-title">{t("settings.deploySymlink")}</span>
      <span class="radio-hint">
        {t("settings.deploySymlinkHint")}
        <Tooltip text={t("settings.devModeTooltip")}>
          <button class="devmode-link" type="button" onclick={openDevModeSettings}>
            {t("settings.devModeLinkLabel")}
          </button>
        </Tooltip>
      </span>
    </span>
  </label>

  {#if devModeError}<div class="status err">{devModeError}</div>{/if}

  {#if validation}
    <div class="status {validation.deploy_mode.ok ? 'ok' : 'err'}">{t(validation.deploy_mode.message)}</div>
  {/if}
</div>

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
  .deploy-block {
    margin-bottom: 16px;
  }
  .row1 {
    margin-bottom: 8px;
  }
  .label {
    font-size: 11px;
    font-weight: 600;
    letter-spacing: 0.5px;
    color: var(--txt2);
    text-transform: uppercase;
  }
  .radio-opt {
    display: flex;
    align-items: flex-start;
    gap: 9px;
    padding: 8px 0;
    cursor: pointer;
  }
  .radio-opt input {
    margin-top: 2px;
    accent-color: var(--rosso);
    flex: none;
  }
  .radio-title {
    display: block;
    font-size: 12.5px;
    color: var(--txt);
  }
  .radio-hint {
    display: block;
    font-size: 11px;
    color: var(--muted);
    line-height: 1.5;
    margin-top: 2px;
  }
  .devmode-link {
    background: transparent;
    color: var(--blue);
    font-size: 11px;
    text-decoration: underline;
    padding: 0;
    display: inline;
  }
  .devmode-link:hover {
    color: var(--rosso-bright);
  }
  .status {
    margin-top: 5px;
    font-size: 11px;
  }
  .status.ok {
    color: var(--green);
  }
  .status.err {
    color: var(--rosso-bright);
  }
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
