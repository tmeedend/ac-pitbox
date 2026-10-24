<script lang="ts">
  // One content item (SPEC-story-mode, Content types; Dialogue text): its
  // header - type, sender, placement, for a dialogue the characters present
  // and the scene, for an illustration its picture, for a document its
  // layout, for what plays on the scene its skin - and its text, in Ink. Each edit saves the whole
  // `.story` file at once; the engine checks the text after every save, line
  // by line (StoryChecks). Its id is not renamed here: renaming a content
  // item does not follow its citations yet. The text shows as a form or as
  // Ink; the form is the tree the engine reads, and only the engine turns
  // one into the other (`readScript`, `writeScript`).
  import { untrack } from "svelte";
  import { errorText } from "$lib/errors";
  import { t } from "$lib/i18n/index.svelte";
  import {
    CONTENT_TYPES,
    canPlayOnScene,
    onScene,
    placementOf,
    sceneTemplate,
    setPlacement,
    setType,
    type Placement,
  } from "$lib/story/content";
  import { mediaUrl } from "$lib/story/player/speakers";
  import { mediaImages } from "$lib/story/world";
  import { setOptional } from "$lib/story/fields";
  import { parseLines } from "$lib/story/flow";
  import { CONTENT_TYPE_LABEL } from "$lib/story/labels";
  import type { FormContext } from "$lib/story/script";
  import { readScript, writeScript, type ContentHeader, type ScriptRead, type Transition } from "$lib/story/studio";
  import { removeContent, saveContentFile, studio } from "$lib/story/studioState.svelte";
  import Field from "$lib/components/ui/Field.svelte";
  import ImageSelectDropdown from "$lib/components/ui/ImageSelectDropdown.svelte";
  import NumberStepper from "$lib/components/ui/NumberStepper.svelte";
  import Seg from "$lib/components/ui/Seg.svelte";
  import FileStatus from "./FileStatus.svelte";
  import ScriptItems from "./ScriptItems.svelte";
  import TargetSelect from "./TargetSelect.svelte";

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
  let view = $state<"form" | "script">("form");
  /** The text read for the form: its tree, or why it cannot be one. */
  let read = $state<ScriptRead | null>(null);
  let formProblem = $state<string | null>(null);
  const ctx = $derived<FormContext | null>(
    read
      ? {
          characters: (story.characters.value?.characters ?? []).map((c) => ({
            id: c.id,
            name: c.name ?? c.id,
            expressions: Object.keys(c.portraits ?? {}),
          })),
          dialogue: h.type === "dialogue",
          from: h.from,
          root: read.script.items,
        }
      : null,
  );

  /** Reads the text for the form, when the form is shown: on opening, on
   * coming back from the Script view, and when the story is read again (an
   * undo, a restore, a file changed elsewhere) - a tree kept from before
   * would write the old text back at the next edit. The form's own saves do
   * not read the story again. */
  $effect(() => {
    const shownFor = story;
    if (view !== "form" || !shownFor) return;
    const body = untrack(() => doc.body);
    void readScript(body).then((r) => (read = r));
  });

  /** The form changed: the engine writes the tree as Ink, which is saved. */
  async function formChanged() {
    if (!read) return;
    try {
      doc.body = await writeScript($state.snapshot(read.script));
      formProblem = null;
      save();
    } catch (e) {
      formProblem = errorText(e);
    }
  }

  function save() {
    void saveContentFile(file);
  }

  function value(e: Event): string {
    return (e.currentTarget as HTMLInputElement | HTMLTextAreaElement | HTMLSelectElement).value;
  }

  /** Sets or removes an optional header key: an empty field writes nothing. */
  /** The pictures an illustration can show: the images of `media/`. */
  const pictures = $derived(
    mediaImages(story.media).map((p) => ({ id: p, name: p, image: mediaUrl(story.path, p) ?? "" })),
  );

  function setOpt<K extends "from" | "clock" | "scene" | "picture" | "skin" | "delay" | "pool">(key: K, v: string) {
    setOptional(h, key, v);
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
  const layouts = $derived([
    { value: "letter", label: t("story.content.layoutLetter") },
    { value: "newspaper", label: t("story.content.layoutNewspaper") },
  ]);
  const transitions = $derived([
    { value: "fade", label: t("story.content.transitionFade") },
    { value: "cut", label: t("story.content.transitionCut") },
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
          <TargetSelect
            value={h.scene ?? ""}
            options={(story.scenes.value?.scenes ?? []).map((p) => p.id)}
            labels={Object.fromEntries((story.scenes.value?.scenes ?? []).map((p) => [p.id, p.name ?? p.id]))}
            emptyLabel={t("story.content.sceneCurrent")}
            onchange={(v) => setOpt("scene", v)}
          />
        </Field>
      {/if}
      {#if h.type === "document"}
        <Field label={t("story.content.layout")}>
          <Seg
            items={layouts}
            value={h.layout ?? "letter"}
            onselect={(v) => {
              // A letter is what a document is when nothing is said: not
              // written. A skin for the other layout goes with it.
              const before = sceneTemplate(h);
              if (v === "letter") delete h.layout;
              else h.layout = v as "newspaper";
              if (sceneTemplate(h) !== before) delete h.skin;
              save();
            }}
          />
        </Field>
      {/if}
      {#if h.type === "illustration" || h.type === "document" || h.type === "video"}
        <Field label={t("story.content.picture")}>
          <ImageSelectDropdown
            options={pictures}
            selectedId={h.picture ?? null}
            placeholder={t("story.content.pickPicture")}
            emptyText={t("story.world.noImagesShort")}
            onselect={(p) => setOpt("picture", p)}
          />
          {#if h.picture && !story.media.includes(h.picture)}
            <span class="missing" title={`media/${h.picture}`}>{t("story.content.pictureMissing")}</span>
          {/if}
        </Field>
      {/if}

      {#if canPlayOnScene(h.type)}
        {@const dresses = sceneTemplate(h)}
        <Field label={t("story.content.skin")}>
          <TargetSelect
            value={h.skin ?? ""}
            options={story.skins.filter((s) => s.template === dresses).map((s) => s.id)}
            emptyLabel={t("story.content.noSkin")}
            onchange={(v) => setOpt("skin", v)}
          />
        </Field>
      {/if}

      {#if onScene(h.type)}
        <Field label={t("story.content.transition")}>
          <Seg
            items={transitions}
            value={h.transition ?? "fade"}
            onselect={(v) => {
              // A fade is what an item does when nothing is said: not written.
              if (v === "fade") delete h.transition;
              else h.transition = v as Transition;
              save();
            }}
          />
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
    <div class="blk-h">
      <span class="blk-t">{t("story.content.text")}</span>
      <span class="views">
        <Seg
          items={[
            { value: "form", label: t("story.content.viewForm") },
            { value: "script", label: t("story.content.viewScript") },
          ]}
          value={view}
          size="mini"
          onselect={(v) => (view = v as "form" | "script")}
        />
      </span>
    </div>
    <div class="blk-b">
      {#if view === "script"}
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
      {:else if read && read.errors.length}
        <p class="warnbox">{t("story.content.formUnreadable")}</p>
        <ul class="errors mono">
          {#each read.errors as e, i (i)}<li>{t("story.content.lineError", { line: e.line, message: e.message })}</li>{/each}
        </ul>
      {:else if read && ctx}
        {#if formProblem}<p class="errbox">{t("story.studio.notSaved", { reason: formProblem })}</p>{/if}
        <ScriptItems items={read.script.items} {ctx} onchange={formChanged} />
      {/if}
    </div>
  </section>
</div>

<style>
  .missing {
    color: var(--yellow);
    font-size: 12px;
  }
  .sheet {
    flex: 1;
    min-width: 420px;
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
  .views {
    margin-left: auto;
  }
  .errors {
    margin: 8px 0 0;
    padding-left: 18px;
    font-size: 11.5px;
    color: var(--muted);
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
