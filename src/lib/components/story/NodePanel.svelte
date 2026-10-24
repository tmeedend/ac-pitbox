<script lang="ts">
  // The selected node's fields (SPEC-story-mode, Create Your Own: "the
  // selected node's fields in a side panel"). Each field writes the chapter
  // when it is left; conditions and effects are typed as the same short
  // sentences as in the files, one per line, and the engine says on saving
  // whether it reads them.
  import { confirm } from "@tauri-apps/plugin-dialog";
  import { t } from "$lib/i18n/index.svelte";
  import {
    allNodeIds,
    exitLabels,
    hasNext,
    isNodeId,
    kindOf,
    moveItem,
    parseLines,
    renameNode,
    setReplyExit,
  } from "$lib/story/flow";
  import { eventIds, eventLabels, newEvent } from "$lib/story/events";
  import {
    addContent,
    addEvent,
    saveChapterAt,
    selectContent,
    selectEvent,
    selectScene,
    studio,
  } from "$lib/story/studioState.svelte";
  import { storyNames, variantIds } from "$lib/story/scenes";
  import { contentIds, newContent } from "$lib/story/content";
  import { nameFrom, parseList } from "$lib/story/world";
  import { KIND_LABEL } from "$lib/story/labels";
  import { frameLabel, framesWith } from "$lib/story/frames";
  import { setOptional } from "$lib/story/fields";
  import type { FlowNode } from "$lib/story/studio";
  import Field from "$lib/components/ui/Field.svelte";
  import TargetSelect from "./TargetSelect.svelte";
  import BeatPhone from "./BeatPhone.svelte";

  interface Props {
    /** Index of the node in the shown chapter. */
    index: number;
  }
  const { index }: Props = $props();

  const story = $derived(studio.story!);
  const chapter = $derived(story.chapters[studio.chapter].chapter!);
  const node = $derived(chapter.nodes[index]);
  const kind = $derived(kindOf(node));
  const nodeIds = $derived(allNodeIds(story.chapters.map((c) => c.chapter)).filter((id) => id !== node.id));
  const flowContent = $derived(story.content.filter((c) => c.placement === "flow").map((c) => c.id));
  const isStart = $derived(node.id === story.manifest.value?.start.node);
  const events = $derived(eventIds(story.events));
  const eventNames = $derived(eventLabels(story.events));
  const places = $derived(story.scenes.value?.scenes ?? []);
  const placeIds = $derived(places.map((p) => p.id));
  const placeNames = $derived(Object.fromEntries(places.map((p) => [p.id, p.name ?? p.id])));
  const chapters = $derived(story.chapters.map((c) => c.chapter));
  /** What a Beat names the story's weather and mood with: the names already
   * in use, and `none`, which clears them. */
  const weathers = $derived(["none", ...storyNames(story.scenes.value, chapters, "weather")]);
  const moods = $derived(["none", ...storyNames(story.scenes.value, chapters, "mood")]);
  /** The apps a Beat can unlock: those of the story's own phones. */
  const appIds = $derived([...new Set((story.frames.value?.frames ?? []).flatMap((f) => f.apps.map((a) => a.id)))]);

  let idError = $state<string | null>(null);

  function save() {
    void saveChapterAt(studio.chapter);
  }

  function value(e: Event): string {
    return (e.currentTarget as HTMLInputElement | HTMLTextAreaElement | HTMLSelectElement).value;
  }

  /** Sets or removes an optional key: an empty field writes nothing. */
  function setOpt<T extends object, K extends keyof T>(target: T, key: K, v: string) {
    setOptional(target, key, v);
    save();
  }

  function rename(e: Event) {
    const to = value(e).trim();
    const from = node.id;
    if (to === from) return (idError = null);
    if (!isNodeId(to)) return (idError = t("story.studio.idInvalid"));
    if (allNodeIds(story.chapters.map((c) => c.chapter)).includes(to)) return (idError = t("story.studio.idTaken"));
    idError = null;
    const changed = renameNode(
      story.chapters.map((c) => c.chapter),
      from,
      to,
    );
    studio.node = to;
    for (const i of changed) void saveChapterAt(i);
  }

  function move(delta: number) {
    chapter.nodes = moveItem(chapter.nodes, index, delta);
    save();
  }

  async function remove() {
    const ok = await confirm(t("story.studio.deleteConfirm", { id: node.id }), {
      title: t("story.studio.deleteNode"),
      kind: "warning",
    });
    if (!ok) return;
    chapter.nodes = chapter.nodes.filter((_, i) => i !== index);
    studio.node = null;
    save();
  }

  function setLines(target: { effects?: string[] }, e: Event) {
    const lines = parseLines(value(e));
    if (lines.length) target.effects = lines;
    else delete target.effects;
    save();
  }

  /** A new event for this node, linked to it and shown in the Events
   * section. Its name is numbered among the events the story has. */
  async function createEvent(n: FlowNode) {
    const name = t("story.events.newName", { n: events.length + 1 });
    const event = newEvent(nameFrom(name, events, "event"), name);
    n.event = event.id;
    save();
    await addEvent(event);
  }

  async function createContent(n: FlowNode) {
    const ids = contentIds(story.contentFiles);
    const doc = newContent(nameFrom(t("story.content.newName", { n: ids.length + 1 }), ids, "content"));
    n.message = doc.header.id;
    save();
    await addContent(doc);
  }

  function addCase(n: FlowNode) {
    n.branch = [...(n.branch ?? []), { if: [], goto: "" }];
    save();
  }

  function removeCase(n: FlowNode, i: number) {
    n.branch = (n.branch ?? []).filter((_, k) => k !== i);
    save();
  }

  function setSide(n: FlowNode, key: "open" | "close", e: Event) {
    const side = (n.side_events ??= {});
    const lines = parseLines(value(e));
    if (lines.length) side[key] = lines;
    else delete side[key];
    save();
  }
</script>

<aside class="panel">
  <div class="lbl">{t("story.studio.selected")} · {t(KIND_LABEL[kind])}</div>

  <Field label={t("story.studio.nodeId")}>
    <input
      class="input mono"
      value={node.id}
      disabled={isStart}
      title={isStart ? t("story.studio.startIdLocked") : undefined}
      onchange={rename}
    />
  </Field>
  {#if idError}<p class="errbox">{idError}</p>{/if}

  {#if kind === "beat" && node.beat}
    {@const beat = node.beat}
    <Field label={t("story.studio.effects")}>
      <textarea class="input mono" rows="3" value={(beat.effects ?? []).join("\n")} onchange={(e) => setLines(beat, e)}
      ></textarea>
    </Field>
    <Field label={t("story.studio.advance")}>
      <input
        class="input mono"
        value={beat.advance ?? ""}
        placeholder="1 month"
        onchange={(e) => setOpt(beat, "advance", value(e))}
      />
    </Field>
    <Field label={t("story.studio.clock")}>
      <input
        class="input mono"
        value={beat.clock ?? ""}
        placeholder="09:00"
        onchange={(e) => setOpt(beat, "clock", value(e))}
      />
    </Field>
    <Field label={t("story.studio.frame")}>
      <select class="input" value={beat.frame ?? ""} onchange={(e) => setOpt(beat, "frame", value(e))}>
        <option value="">{t("story.studio.frameUnchanged")}</option>
        {#each framesWith(beat.frame, story.frames.value) as f (f)}
          <option value={f}>{frameLabel(f, story.frames.value)}</option>
        {/each}
      </select>
    </Field>
    <Field label={t("story.scenes.place")}>
      <TargetSelect
        value={beat.scene ?? ""}
        options={placeIds}
        labels={placeNames}
        emptyLabel={t("story.scenes.unchanged")}
        onchange={(v) => setOpt(beat, "scene", v)}
      />
      {#if beat.scene && placeIds.includes(beat.scene)}
        <div class="links">
          <button class="btn btn-ghost" onclick={() => selectScene(beat.scene!)}>{t("story.events.open")}</button>
        </div>
      {/if}
    </Field>
    <Field label={t("story.scenes.forcedVariant")}>
      <TargetSelect
        value={beat.variant ?? ""}
        options={variantIds(story.scenes.value, beat.scene)}
        emptyLabel={t("story.scenes.noForcedVariant")}
        onchange={(v) => setOpt(beat, "variant", v)}
      />
    </Field>
    <div class="pair">
      <Field label={t("story.scenes.weather")}>
        <input
          class="input mono"
          list="beat-weathers"
          value={beat.weather ?? ""}
          placeholder={t("story.scenes.unchanged")}
          onchange={(e) => setOpt(beat, "weather", value(e))}
        />
      </Field>
      <Field label={t("story.scenes.mood")}>
        <input
          class="input mono"
          list="beat-moods"
          value={beat.mood ?? ""}
          placeholder={t("story.scenes.unchanged")}
          onchange={(e) => setOpt(beat, "mood", value(e))}
        />
      </Field>
    </div>
    <Field label={t("story.studio.unlock")}>
      <input
        class="input mono"
        value={(beat.unlock ?? []).join(", ")}
        placeholder={appIds.join(", ")}
        onchange={(e) => {
          const ids = parseList(value(e));
          if (ids.length) beat.unlock = ids;
          else delete beat.unlock;
          save();
        }}
      />
    </Field>
    <BeatPhone {beat} onchange={save} />
    <datalist id="beat-weathers">{#each weathers as w (w)}<option value={w}></option>{/each}</datalist>
    <datalist id="beat-moods">{#each moods as m (m)}<option value={m}></option>{/each}</datalist>
  {:else if kind === "message"}
    <Field label={t("story.studio.content")}>
      <TargetSelect
        value={node.message ?? ""}
        options={flowContent}
        onchange={(v) => {
          node.message = v;
          save();
        }}
      />
      <div class="links">
        {#if node.message && flowContent.includes(node.message)}
          <button class="btn btn-ghost" onclick={() => selectContent(node.message!)}>{t("story.events.open")}</button>
        {/if}
        <button class="btn btn-ghost" onclick={() => createContent(node)}>+ {t("story.content.create")}</button>
      </div>
    </Field>
    {@const sent = story.content.find((c) => c.id === node.message)}
    {@const exits = exitLabels(sent?.labels ?? [], node.on_reply)}
    <!-- A node with reply exits, or playing a dialogue, waits whatever the
         box says (story_engine::engine, Message): the box shows it, locked. -->
    {@const waits = Object.keys(node.on_reply ?? {}).length > 0 || sent?.kind === "dialogue"}
    <Field>
      <label class="check">
        <input
          type="checkbox"
          checked={waits || (node.wait_reply ?? false)}
          disabled={waits}
          onchange={(e) => {
            if ((e.currentTarget as HTMLInputElement).checked) node.wait_reply = true;
            else delete node.wait_reply;
            save();
          }}
        />
        {t("story.studio.waitReply")}
      </label>
    </Field>
    {#if exits.length}
      <Field label={t("story.studio.replyExits")}>
        {#each exits as label (label)}
          <div class="goto">
            <span class="lbl-key mono">{label}</span>
            <TargetSelect
              value={node.on_reply?.[label] ?? ""}
              options={nodeIds}
              emptyLabel={t("story.studio.replyGoesOn")}
              onchange={(v) => {
                setReplyExit(node, label, v);
                save();
              }}
            />
          </div>
        {/each}
      </Field>
    {/if}
  {:else if kind === "event"}
    <Field label={t("story.studio.event")}>
      <TargetSelect
        value={node.event ?? ""}
        options={events}
        labels={eventNames}
        onchange={(v) => {
          node.event = v;
          save();
        }}
      />
      <div class="links">
        {#if node.event && events.includes(node.event)}
          <button class="btn btn-ghost" onclick={() => selectEvent(node.event!)}>{t("story.events.open")}</button>
        {/if}
        <button class="btn btn-ghost" onclick={() => createEvent(node)}>+ {t("story.events.create")}</button>
      </div>
    </Field>
    <Field label={t("story.studio.onFailure")}>
      <TargetSelect
        value={node.on_failure ?? ""}
        options={nodeIds}
        emptyLabel={t("story.studio.onFailureSetting")}
        onchange={(v) => setOpt(node, "on_failure", v)}
      />
    </Field>
  {:else if kind === "branch"}
    {#each node.branch ?? [] as c, i (i)}
      <div class="case">
        <div class="blk-sub">{t("story.studio.case", { n: i + 1 })}</div>
        <textarea
          class="input mono"
          rows="2"
          value={c.if.join("\n")}
          placeholder="money >= 100"
          onchange={(e) => {
            c.if = parseLines(value(e));
            save();
          }}
        ></textarea>
        <div class="goto">
          <span class="lbl-key">{t("story.studio.goto")}</span>
          <TargetSelect
            value={c.goto}
            options={nodeIds}
            onchange={(v) => {
              c.goto = v;
              save();
            }}
          />
          <button class="btn btn-ghost" onclick={() => removeCase(node, i)}>{t("story.studio.removeCase")}</button>
        </div>
      </div>
    {/each}
    <button class="btn" onclick={() => addCase(node)}>+ {t("story.studio.addCase")}</button>
    <Field label={t("story.studio.otherwise")}>
      <TargetSelect
        value={node.otherwise ?? ""}
        options={nodeIds}
        onchange={(v) => {
          node.otherwise = v;
          save();
        }}
      />
    </Field>
  {:else if kind === "action"}
    <Field label={t("story.studio.action")}>
      <input
        class="input mono"
        value={node.action ?? ""}
        onchange={(e) => {
          node.action = value(e).trim();
          save();
        }}
      />
    </Field>
  {:else if kind === "side_events"}
    <Field label={t("story.studio.sideOpen")}>
      <textarea
        class="input mono"
        rows="2"
        value={(node.side_events?.open ?? []).join("\n")}
        onchange={(e) => setSide(node, "open", e)}
      ></textarea>
    </Field>
    <Field label={t("story.studio.sideClose")}>
      <textarea
        class="input mono"
        rows="2"
        value={(node.side_events?.close ?? []).join("\n")}
        onchange={(e) => setSide(node, "close", e)}
      ></textarea>
    </Field>
  {:else if kind === "end"}
    <Field label={t("story.studio.ending")}>
      <input
        class="input mono"
        value={node.end ?? ""}
        onchange={(e) => {
          node.end = value(e).trim();
          save();
        }}
      />
    </Field>
  {/if}

  {#if hasNext(node)}
    <Field label={t("story.studio.next")}>
      <TargetSelect
        value={node.next ?? ""}
        options={nodeIds}
        emptyLabel={t("story.studio.nextInOrder")}
        onchange={(v) => setOpt(node, "next", v)}
      />
    </Field>
  {/if}

  <div class="actions">
    <button class="btn" disabled={index === 0} onclick={() => move(-1)} aria-label={t("story.studio.moveUp")}
      >↑</button
    >
    <button
      class="btn"
      disabled={index === chapter.nodes.length - 1}
      onclick={() => move(1)}
      aria-label={t("story.studio.moveDown")}>↓</button
    >
    <span class="spacer"></span>
    <button class="btn btn-ghost" onclick={remove}>{t("story.studio.deleteNode")}</button>
  </div>
</aside>

<style>
  .panel {
    width: 320px;
    flex-shrink: 0;
    border-left: 1px solid var(--line);
    background: var(--panel);
    padding: 18px 16px;
    overflow: auto;
  }
  textarea.input {
    resize: vertical;
    display: block;
  }
  .errbox {
    margin: 8px 0 0;
  }
  .check {
    display: flex;
    align-items: center;
    gap: 8px;
    font-size: 12.5px;
    color: var(--txt2);
  }
  .links {
    display: flex;
    gap: 6px;
    margin-top: 6px;
  }
  /* Weather and mood side by side: two short names. */
  .pair {
    display: grid;
    grid-template-columns: 1fr 1fr;
    gap: 10px;
  }
  .case {
    margin: 14px 0 10px;
    padding-bottom: 12px;
    border-bottom: 1px solid var(--line);
  }
  .goto {
    display: flex;
    align-items: center;
    gap: 8px;
    margin-top: 8px;
  }
  .goto .lbl-key {
    white-space: nowrap;
  }
  .actions {
    display: flex;
    align-items: center;
    gap: 6px;
    margin-top: 22px;
  }
  .spacer {
    flex: 1;
  }
</style>
