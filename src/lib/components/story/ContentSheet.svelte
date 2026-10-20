<script lang="ts">
  // One content item (SPEC-story-mode, Content types; Dialogue text): its
  // header - type, sender, placement, and for a dialogue the characters
  // present and the scene - and its text, in Ink. Each edit saves the whole
  // `.story` file at once; the engine checks the text after every save, line
  // by line (StoryChecks). Its id is not renamed here: renaming a content
  // item does not follow its citations yet.
  import { t } from "$lib/i18n/index.svelte";
  import { CONTENT_TYPES, placementOf, setPlacement, setType, type Placement } from "$lib/story/content";
  import { parseLines } from "$lib/story/flow";
  import { CONTENT_TYPE_LABEL } from "$lib/story/labels";
  import type { ContentHeader } from "$lib/story/studio";
  import { removeContent, saveContentFile, studio } from "$lib/story/studioState.svelte";
  import Field from "$lib/components/ui/Field.svelte";
  import NumberStepper from "$lib/components/ui/NumberStepper.svelte";
  import Seg from "$lib/components/ui/Seg.svelte";
  import FileStatus from "./FileStatus.svelte";

  interface Props {
    file: string;
    index: number;
  }
  const { file, index }: Props = $props();

  const story = $derived(studio.story!);
  const entry = $derived(story.contentFiles.find((f) => f.file === file)!);
  const doc = $derived(entry.value!.items[index]);
  const h = $derived(doc.header);
  const placement = $derived(placementOf(h));
  const characters = $derived(story.characters.value?.characters.map((c) => c.id) ?? []);
  const names = $derived(
    Object.fromEntries((story.characters.value?.characters ?? []).map((c) => [c.id, c.name ?? c.id])),
  );

  let removeProblem = $state<string | null>(null);

  function save() {
    void saveContentFile(file);
  }

  function value(e: Event): string {
    return (e.currentTarget as HTMLInputElement | HTMLTextAreaElement | HTMLSelectElement).value;
  }

  /** Sets or removes an optional header key: an empty field writes nothing. */
  function setOpt<K extends "from" | "clock" | "scene" | "delay" | "pool">(key: K, v: string) {
    if (v.trim()) h[key] = v.trim();
    else delete h[key];
    save();
  }

  function setWhen(e: Event) {
    const lines = parseLines(value(e));
    // Reactive content keeps `when` even empty: it is what makes it reactive.
    if (lines.length || placement === "reactive") h.when = lines;
    else delete h.when;
    save();
  }

  function togglePresent(id: string, on: boolean) {
    const present = (h.characters ?? []).filter((c) => c !== id);
    if (on) present.push(id);
    if (present.length) h.characters = present;
    else delete h.characters;
    save();
  }

  async function remove() {
    removeProblem = await removeContent(h.id);
  }

  const placements = $derived([
    { value: "flow", label: t("story.content.placementFlow") },
    { value: "reactive", label: t("story.content.placementReactive") },
    { value: "pool", label: t("story.content.placementPool") },
  ]);
  const repeats = $derived([
    { value: "once", label: t("story.content.repeatOnce") },
    { value: "each_time", label: t("story.content.repeatEachTime") },
  ]);
</script>

<div class="sheet">
  <FileStatus {entry} />
  <header>
    <span class="mono id">{h.id}</span>
    <span class="file mono">{file}</span>
    <button class="btn btn-ghost" onclick={remove}>{t("story.content.delete")}</button>
  </header>
  {#if removeProblem}<p class="errbox">{removeProblem}</p>{/if}

  <section class="blk">
    <div class="blk-b">
      <div class="pair">
        <Field label={t("story.content.type")}>
          <select
            class="input"
            value={h.type}
            onchange={(e) => {
              setType(h, value(e) as ContentHeader["type"]);
              save();
            }}
          >
            {#each CONTENT_TYPES as kind (kind)}
              <option value={kind}>{t(CONTENT_TYPE_LABEL[kind])}</option>
            {/each}
          </select>
        </Field>
        <Field label={t("story.content.from")}>
          <select class="input" value={h.from ?? ""} onchange={(e) => setOpt("from", value(e))}>
            <option value="">—</option>
            {#if h.from && !characters.includes(h.from)}<option value={h.from}>{h.from}</option>{/if}
            {#each characters as id (id)}
              <option value={id}>{names[id]}</option>
            {/each}
          </select>
        </Field>
        <Field label={t("story.content.clock")}>
          <input class="input" type="time" value={h.clock ?? ""} onchange={(e) => setOpt("clock", value(e))} />
        </Field>
      </div>

      {#if h.type === "dialogue"}
        <Field label={t("story.content.present")}>
          <div class="present">
            {#each characters as id (id)}
              <label class="check">
                <input
                  type="checkbox"
                  checked={h.characters?.includes(id) ?? false}
                  onchange={(e) => togglePresent(id, (e.currentTarget as HTMLInputElement).checked)}
                />
                {names[id]}
              </label>
            {/each}
          </div>
        </Field>
        <Field label={t("story.content.scene")}>
          <input class="input mono" value={h.scene ?? ""} onchange={(e) => setOpt("scene", value(e))} />
        </Field>
      {/if}

      <Field label={t("story.content.placement")}>
        <Seg
          items={placements}
          value={placement}
          onselect={(p) => {
            setPlacement(h, p as Placement);
            save();
          }}
        />
      </Field>
      {#if placement === "pool"}
        <Field label={t("story.content.pool")}>
          <input class="input mono" value={h.pool ?? ""} onchange={(e) => setOpt("pool", value(e))} />
        </Field>
      {/if}
      {#if placement !== "flow"}
        <Field label={t("story.content.when")}>
          <textarea
            class="input mono"
            rows="2"
            value={(h.when ?? []).join("\n")}
            placeholder="reputation >= 2"
            onchange={setWhen}
          ></textarea>
        </Field>
      {/if}
      {#if placement === "reactive"}
        <div class="pair">
          <Field label={t("story.content.delay")}>
            <input
              class="input mono"
              value={h.delay ?? ""}
              placeholder="3 days"
              onchange={(e) => setOpt("delay", value(e))}
            />
          </Field>
          <Field label={t("story.content.priority")}>
            <NumberStepper
              value={h.priority ?? 0}
              width={80}
              onchange={(v) => {
                if (v) h.priority = v;
                else delete h.priority;
                save();
              }}
            />
          </Field>
          <Field label={t("story.content.repeat")}>
            <Seg
              items={repeats}
              value={h.repeat ?? "once"}
              onselect={(r) => {
                if (r === "each_time") h.repeat = "each_time";
                else delete h.repeat;
                save();
              }}
            />
          </Field>
        </div>
      {/if}
    </div>
  </section>

  <section class="blk">
    <div class="blk-h"><span class="blk-t">{t("story.content.text")}</span></div>
    <div class="blk-b">
      <textarea
        class="input mono script"
        rows="16"
        spellcheck="false"
        value={doc.body}
        aria-label={t("story.content.text")}
        onchange={(e) => {
          doc.body = value(e);
          save();
        }}
      ></textarea>
    </div>
  </section>
</div>

<style>
  .sheet {
    flex: 1;
    min-width: 0;
    max-width: 760px;
  }
  header {
    display: flex;
    align-items: baseline;
    gap: 12px;
    margin-bottom: 12px;
  }
  .id {
    font-size: 15px;
    color: var(--txt);
  }
  .file {
    flex: 1;
    color: var(--muted);
    font-size: 11px;
  }
  .pair {
    display: flex;
    gap: 16px;
    flex-wrap: wrap;
  }
  .present {
    display: flex;
    flex-wrap: wrap;
    gap: 6px 16px;
  }
  .check {
    display: flex;
    align-items: center;
    gap: 6px;
    font-size: 13px;
  }
  .script {
    width: 100%;
    resize: vertical;
    line-height: 1.5;
    tab-size: 4;
  }
  .errbox {
    margin: 0 0 12px;
  }
</style>
