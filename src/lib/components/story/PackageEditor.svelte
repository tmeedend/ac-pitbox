<script lang="ts">
  // The Package section (SPEC-story-mode, Create Your Own: Package): the
  // story's permission register - one `credits.yaml` per folder that
  // travels - and the build of its package. Each file the player sees or
  // hears needs a line saying who made it, under which licence, and whether
  // it may travel; a file without one offers to start it. The cars and
  // tracks the story races on are listed apart: the game's own stay out of
  // the package, the others travel with a line of `package/manifest.yaml`.
  // The build waits for every line and for a story with no error. Each edit
  // saves its register at once.
  import { open, save } from "@tauri-apps/plugin-dialog";
  import { errorText } from "$lib/errors";
  import { fmtSize } from "$lib/format";
  import { t } from "$lib/i18n/index.svelte";
  import { contentLineFor, contentLineOf, lineFor, packageName, registerGroups, splitFiles } from "$lib/story/package";
  import { buildPackage, publishPackage, type ContentCredit, type Credit, type Holding } from "$lib/story/studio";
  import { registerEntry, saveContentRegister, saveRegister, studio } from "$lib/story/studioState.svelte";
  import CreditFields from "./CreditFields.svelte";
  import StoryChecks from "./StoryChecks.svelte";

  const story = $derived(studio.story!);
  const manifest = $derived(story.manifest.value);
  const groups = $derived(registerGroups(story.package));
  const registerDiagnostics = $derived(story.package.register.diagnostics);
  const storyErrors = $derived(studio.diagnostics.filter((d) => d.severity === "error").length);
  const registerErrors = $derived(registerDiagnostics.filter((d) => d.severity === "error").length);
  const blocked = $derived(storyErrors > 0 || registerErrors > 0 || !manifest);

  let building = $state(false);
  let built = $state<string | null>(null);
  let failed = $state<string | null>(null);

  const contentLines = $derived(story.package.content_register.value);

  function editContent(change: (lines: ContentCredit[]) => void) {
    const lines = story.package.content_register.value;
    if (!lines) return;
    change(lines);
    void saveContentRegister();
  }

  function lineOf(h: Holding, layer?: string): number {
    return contentLineOf(contentLines ?? [], h.kind, h.id, layer);
  }

  /** Takes the layer `layer` of `h` along with the story, or leaves it. */
  function carryLayer(h: Holding, layer: string) {
    const j = lineOf(h, layer);
    editContent((lines) => (j >= 0 ? lines.splice(j, 1) : lines.push(contentLineFor(h.kind, h.id, layer))));
  }

  function edit(file: string, change: (lines: Credit[]) => void) {
    const entry = registerEntry(file);
    if (!entry?.value) return;
    change(entry.value);
    void saveRegister(file);
  }

  function field(e: Event): string {
    return (e.currentTarget as HTMLInputElement).value;
  }

  /** Writes what a host serves into a folder the author picks: the link
   * players paste is that of its `manifest.json`. */
  async function publish() {
    const dir = await open({ directory: true, title: t("story.package.publish") });
    if (typeof dir !== "string") return;
    building = true;
    built = null;
    failed = null;
    try {
      const size = await publishPackage(story.path, dir);
      built = t("story.package.published", { dir, size: fmtSize(size) });
    } catch (e) {
      failed = errorText(e);
    } finally {
      building = false;
    }
  }

  async function build() {
    if (!manifest) return;
    const out = await save({
      title: t("story.package.build"),
      defaultPath: packageName(manifest.id, manifest.version),
      filters: [{ name: t("story.play.packageFile"), extensions: ["pitstory"] }],
    });
    if (!out) return;
    building = true;
    built = null;
    failed = null;
    try {
      const size = await buildPackage(story.path, out);
      built = t("story.package.built", { path: out, size: fmtSize(size) });
    } catch (e) {
      failed = errorText(e);
    } finally {
      building = false;
    }
  }
</script>

<div class="main">
  <header>
    <span class="lbl-screen">{t("story.package.title")}</span>
  </header>
  {#if studio.notice}<p class="warnbox">{studio.notice}</p>{/if}

  <section class="blk">
    <div class="blk-h">
      <span class="blk-t">{t("story.package.buildTitle")}</span>
      {#if manifest}<span class="blk-n">{packageName(manifest.id, manifest.version)}</span>{/if}
    </div>
    <div class="blk-b build">
      <button class="btn" disabled={blocked || building} onclick={build}>{t("story.package.build")}</button>
      <button class="btn" disabled={blocked || building} onclick={publish}>{t("story.package.publish")}</button>
      {#if storyErrors > 0}
        <span class="why">{t("story.package.storyErrors", { count: storyErrors })}</span>
      {:else if registerErrors > 0}
        <span class="why">{t("story.package.registerErrors", { count: registerErrors })}</span>
      {/if}
    </div>
    {#if built}<p class="done">{built}</p>{/if}
    {#if failed}<p class="errbox">{failed}</p>{/if}
  </section>

  {#if story.package.content.length}
    <section class="blk">
      <div class="blk-h">
        <span class="blk-t">{t("story.package.contentTitle")}</span>
        <span class="blk-n mono">{story.package.content_register.file}</span>
      </div>
      <div class="blk-b">
        {#if contentLines === null}
          <p class="errbox">{t("story.package.unreadable")}</p>
        {:else}
          {#each story.package.content as h (`${h.kind}/${h.id}`)}
            {@const i = lineOf(h)}
            <div class="line">
              <div class="row">
                <span class="mono item">{h.id}</span>
                <span class="named">{t(h.kind === "car" ? "story.package.car" : "story.package.track")}</span>
                {#if h.official}
                  <span class="named">{h.dlc ? t("story.package.dlc", { dlc: h.dlc }) : t("story.package.baseGame")}</span>
                {:else if !h.folder}
                  <span class="pill pill-err">{t("story.package.notInLibrary")}</span>
                {:else if i < 0}
                  <button class="btn btn-ghost" onclick={() => editContent((lines) => lines.push(contentLineFor(h.kind, h.id)))}
                    >{t("story.package.addLine")}</button
                  >
                {/if}
                {#if i >= 0}
                  <button
                    class="btn btn-ghost"
                    title={t("story.package.removeLine")}
                    aria-label={t("story.package.removeLine")}
                    onclick={() => editContent((lines) => lines.splice(i, 1))}>✕</button
                  >
                {/if}
              </div>
              {#if i >= 0}
                <CreditFields line={contentLines[i]} onchange={(apply) => editContent((lines) => apply(lines[i]))} />
              {/if}
              {#each h.layers as layer (layer.name)}
                {@const j = lineOf(h, layer.name)}
                <label class="carry">
                  <input type="checkbox" checked={j >= 0} onchange={() => carryLayer(h, layer.name)} />
                  <span>{t("story.package.layer", { name: layer.name })}</span>
                </label>
                {#if j >= 0}
                  <CreditFields line={contentLines[j]} onchange={(apply) => editContent((lines) => apply(lines[j]))} />
                {/if}
              {/each}
            </div>
          {/each}
        {/if}
      </div>
    </section>
  {/if}

  {#each groups as g (g.file)}
    <section class="blk">
      <div class="blk-h">
        <span class="blk-t">{g.skin ? t("story.package.skinRegister", { skin: g.skin }) : t("story.package.mediaRegister")}</span>
        <span class="blk-n mono">{g.file}</span>
      </div>
      <div class="blk-b">
        {#if g.lines === null}
          <p class="errbox">{t("story.package.unreadable")}</p>
        {:else}
          {#each g.lines as line, i (i)}
            <div class="line">
              <div class="row">
                <label class="files">
                  <span class="lbl">{t("story.package.files")}</span>
                  <input
                    class="input mono"
                    value={line.files.join(", ")}
                    onchange={(e) => edit(g.file, (lines) => (lines[i].files = splitFiles(field(e))))}
                  />
                </label>
                <span class="named">{t("story.package.named", { count: g.named[i] ?? 0 })}</span>
                <button
                  class="btn btn-ghost"
                  title={t("story.package.removeLine")}
                  aria-label={t("story.package.removeLine")}
                  onclick={() => edit(g.file, (lines) => lines.splice(i, 1))}>✕</button
                >
              </div>
              <CreditFields {line} onchange={(apply) => edit(g.file, (lines) => apply(lines[i]))} />
            </div>
          {/each}

          {#if g.uncredited.length}
            <div class="lbl-sub missing">{t("story.package.uncredited", { count: g.uncredited.length })}</div>
            <ul class="uncredited">
              {#each g.uncredited as file (file)}
                <li>
                  <span class="mono">{file}</span>
                  <button class="btn btn-ghost" onclick={() => edit(g.file, (lines) => lines.push(lineFor(file)))}
                    >{t("story.package.addLine")}</button
                  >
                </li>
              {/each}
            </ul>
          {:else if g.lines.length === 0}
            <p class="empty">{t("story.package.noLine")}</p>
          {/if}
        {/if}
      </div>
    </section>
  {:else}
    <p class="empty">{t("story.package.nothingToCredit")}</p>
  {/each}

  <StoryChecks section="package" list={registerDiagnostics} />
</div>

<style>
  .main {
    flex: 1;
    min-width: 0;
    overflow: auto;
    padding: 22px 28px;
  }
  header {
    margin-bottom: 14px;
  }
  .blk {
    max-width: 880px;
  }
  .build {
    display: flex;
    align-items: center;
    gap: 12px;
  }
  .why,
  .named,
  .empty,
  .done {
    color: var(--muted);
    font-size: 12px;
  }
  .done {
    margin: 8px 0 0;
  }
  .errbox {
    margin: 8px 0 0;
  }
  .line {
    display: flex;
    flex-direction: column;
    gap: 6px;
    padding: 10px 0;
    border-bottom: 1px solid var(--line);
  }
  .row {
    display: flex;
    align-items: flex-end;
    gap: 10px;
    flex-wrap: wrap;
  }
  label {
    display: flex;
    flex-direction: column;
    gap: 3px;
    min-width: 160px;
  }
  .files {
    flex: 1;
  }
  .named {
    padding-bottom: 7px;
  }
  .carry {
    flex-direction: row;
    align-items: center;
    gap: 8px;
    font-size: 12px;
    margin-top: 4px;
  }
  .item {
    flex: 1;
    font-size: 12px;
    padding-bottom: 7px;
  }
  .missing {
    margin-top: 14px;
  }
  .uncredited {
    list-style: none;
    margin: 6px 0 0;
    padding: 0;
    display: flex;
    flex-direction: column;
    gap: 4px;
  }
  .uncredited li {
    display: flex;
    align-items: center;
    gap: 10px;
    font-size: 12px;
  }
</style>
