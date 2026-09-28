<script lang="ts">
  // The delete confirmation (ESPACE§5.2): two outcomes side by side, never a
  // "purge" checkbox. A checkbox reads like an advanced option and is skipped;
  // two choices make the user read both consequences — and that is where they
  // learn what the showcase is. The showcase is always the default, never
  // remembered: a mistake towards it is undone in one click, a mistake
  // towards the complete deletion loses notes.
  //
  // Mounted once in the shell and driven by `deleteDialog.request`: deleting
  // starts from the fiche and from the library's context menu alike.
  import { t } from "$lib/i18n/index.svelte";
  import { fmtSize } from "$lib/format";
  import { getUiPref, setUiPref } from "$lib/uiPrefs.svelte";
  import { deleteDialog, freedLine, HOW_IT_WORKS_KEY, type PlanEntry } from "$lib/library/showcase.svelte";

  let mode = $state<"showcase" | "complete">("showcase");
  let keepArchive = $state(true);
  let howHidden = $state(true);

  const request = $derived(deleteDialog.request);
  const entries = $derived(request?.entries ?? []);
  const one = $derived(entries.length === 1 ? entries[0] : null);
  /** Mods that still have files: those the showcase would apply to. */
  const freeable = $derived(entries.filter((e) => !e.showcase));
  /** Everything is already in the showcase: a complete deletion is all that
   * is left (ESPACE§5.1). */
  const completeOnly = $derived(freeable.length === 0);
  const size = $derived(freeable.reduce((n, e) => n + e.size_bytes, 0));
  const active = $derived(entries.filter((e) => e.active));
  const unknownSource = $derived(freeable.filter((e) => !e.source_file_name));
  const anyKept = $derived(freeable.some((e) => e.kept_archive));

  // Each opening starts from the showcase, whatever was chosen last time.
  $effect(() => {
    const r = request;
    if (!r) return;
    mode = r.entries.every((e) => e.showcase) ? "complete" : "showcase";
    keepArchive = true;
    void getUiPref(HOW_IT_WORKS_KEY).then((v) => (howHidden = v === "1"));
  });

  function answer(ok: boolean) {
    request?.resolve(ok ? { mode, keepArchive } : null);
  }

  function hideHow() {
    howHidden = true;
    setUiPref(HOW_IT_WORKS_KEY, "1");
  }

  function sourceLine(e: PlanEntry): string {
    if (!e.source_file_name) return t("showcase.noSource");
    return e.source_site
      ? t("showcase.sourceFrom", { file: e.source_file_name, site: siteName(e.source_site) })
      : t("showcase.source", { file: e.source_file_name });
  }

  /** `https://www.overtake.gg/` reads `overtake.gg`. */
  function siteName(site: string): string {
    try {
      return new URL(site).hostname.replace(/^www\./, "");
    } catch {
      return site;
    }
  }

  function onkeydown(e: KeyboardEvent) {
    if (request && e.key === "Escape") answer(false);
  }
</script>

<svelte:window {onkeydown} />

{#if request}
  <div class="backdrop">
    <div class="modal" role="dialog" aria-modal="true">
      <header>
        <h2>
          {#if one}
            {completeOnly ? t("showcase.titleCompleteOne", { name: one.name }) : t("showcase.titleOne", { name: one.name })}
          {:else}
            {completeOnly
              ? t("showcase.titleCompleteMany", { count: entries.length })
              : t("showcase.titleMany", { count: entries.length })}
          {/if}
        </h2>
      </header>

      <div class="body">
        {#if !one}
          <p class="summary">{t("showcase.summaryMany", { count: entries.length, size: fmtSize(size) })}</p>
        {/if}

        {#if !completeOnly}
          <label class="choice" class:on={mode === "showcase"}>
            <input type="radio" name="delete-mode" value="showcase" bind:group={mode} />
            <span class="c-body">
              <span class="c-title">{t("showcase.keep")} <span class="c-reco">({t("showcase.recommended")})</span></span>
              <span class="c-line">{freedLine(size)} {t("showcase.explain")}</span>
              {#if mode === "showcase"}
                {#if one}
                  <span class="c-line c-source">{sourceLine(one)}</span>
                {:else if unknownSource.length}
                  <span class="c-line c-source">
                    {t("showcase.noSourceMany", { count: unknownSource.length })}
                    {unknownSource.map((e) => e.name).join(", ")}
                  </span>
                {/if}
                {#if anyKept}
                  <span class="c-check">
                    <input type="checkbox" bind:checked={keepArchive} />
                    {t("showcase.keepArchive")}
                  </span>
                {/if}
              {/if}
            </span>
          </label>

          {#if !howHidden && mode === "showcase"}
            <div class="how">
              <div class="how-t">{t("showcase.howTitle")}</div>
              <ol>
                <li>{t("showcase.how1")}</li>
                <li>{t("showcase.how2")}</li>
                <li>{t("showcase.how3")}</li>
              </ol>
              <button class="btn btn-ghost how-hide" type="button" onclick={hideHow}>{t("showcase.hideHow")}</button>
            </div>
          {/if}
        {/if}

        <label class="choice" class:on={mode === "complete"}>
          <input type="radio" name="delete-mode" value="complete" bind:group={mode} disabled={completeOnly} />
          <span class="c-body">
            <span class="c-title">{t("showcase.complete")}</span>
            <span class="c-line">
              {#if !completeOnly}{freedLine(size)}{/if}
              {t("showcase.completeLine")}
            </span>
          </span>
        </label>

        {#if active.length}
          <p class="note">
            {one ? t("showcase.willDeactivate") : t("showcase.activeMany", { count: active.length })}
          </p>
        {/if}
        {#if one && one.versions > 1}
          <p class="note">{t("showcase.versions", { count: one.versions })}</p>
        {/if}
        <p class="note muted">{t("showcase.bin")}</p>
      </div>

      <footer>
        <button class="btn" type="button" onclick={() => answer(false)}>{t("common.cancel")}</button>
        <button class="btn btn-primary" type="button" onclick={() => answer(true)}>{t("showcase.confirm")}</button>
      </footer>
    </div>
  </div>
{/if}

<style>
  .backdrop {
    position: fixed;
    inset: 0;
    background: rgb(0 0 0 / 60%);
    display: flex;
    align-items: center;
    justify-content: center;
    z-index: 100;
  }
  .modal {
    width: 520px;
    max-width: 92vw;
    max-height: 85vh;
    display: flex;
    flex-direction: column;
    background: var(--panel);
    border: 1px solid var(--rosso);
  }
  header {
    padding: 12px 16px;
    border-bottom: 1px solid var(--line);
  }
  h2 {
    font-size: 13px;
    letter-spacing: 0.5px;
    text-transform: uppercase;
    color: var(--txt2);
  }
  .body {
    overflow-y: auto;
    padding: 14px 16px;
    display: flex;
    flex-direction: column;
    gap: 10px;
  }
  .summary {
    color: var(--txt2);
    font-size: 12px;
  }
  .choice {
    display: flex;
    gap: 10px;
    padding: 10px 12px;
    background: var(--panel2);
    border: 1px solid var(--line);
    cursor: pointer;
  }
  .choice.on {
    border-color: var(--rosso-border);
  }
  .choice input[type="radio"] {
    flex: none;
    margin-top: 2px;
    accent-color: var(--rosso);
  }
  .c-body {
    display: flex;
    flex-direction: column;
    gap: 4px;
    min-width: 0;
  }
  .c-title {
    color: var(--txt2);
    font-size: 12.5px;
  }
  .c-reco {
    color: var(--muted);
  }
  .c-line {
    color: var(--muted);
    font-size: 11.5px;
    line-height: 1.45;
  }
  .c-source {
    color: var(--txt2);
    overflow-wrap: anywhere;
  }
  .c-check {
    display: flex;
    align-items: center;
    gap: 6px;
    color: var(--txt2);
    font-size: 11.5px;
  }
  .how {
    padding: 8px 12px;
    border-left: 2px solid var(--line);
    color: var(--muted);
    font-size: 11.5px;
  }
  .how-t {
    color: var(--txt2);
    margin-bottom: 4px;
  }
  .how ol {
    margin: 0 0 6px 16px;
    padding: 0;
  }
  .how-hide {
    font-size: 11px;
    padding: 0;
  }
  .note {
    color: var(--txt2);
    font-size: 11.5px;
  }
  .note.muted {
    color: var(--muted);
  }
  footer {
    display: flex;
    justify-content: flex-end;
    gap: 8px;
    padding: 12px 16px;
    border-top: 1px solid var(--line);
  }
</style>
