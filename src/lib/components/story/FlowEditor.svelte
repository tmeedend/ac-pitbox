<script lang="ts">
  // The Story flow screen (SPEC-story-mode, Create Your Own): one chapter's
  // nodes, top to bottom in the order they play, an arrow only where a node
  // jumps elsewhere. A column laid out by the order itself rather than a
  // canvas: nothing to store about where a box sits, and a file that changes
  // only when the story does. The graph shows the same chapter as a map,
  // read-only (`FlowGraph`); a box clicked there opens in the column.
  import { t } from "$lib/i18n/index.svelte";
  import { allNodeIds, freshNodeId, hasNext, insertNode, kindOf, linksOf, newNode, NODE_KINDS } from "$lib/story/flow";
  import type { NodeKind } from "$lib/story/flow";
  import { eventIds, eventLabels } from "$lib/story/events";
  import type { FlowNode } from "$lib/story/studio";
  import { saveChapterAt, selectNode, studio } from "$lib/story/studioState.svelte";
  import { KIND_LABEL, LINK_LABEL } from "$lib/story/labels";
  import Seg from "$lib/components/ui/Seg.svelte";
  import FlowGraph from "./FlowGraph.svelte";
  import NodePanel from "./NodePanel.svelte";
  import StoryChecks from "./StoryChecks.svelte";

  const story = $derived(studio.story!);
  const start = $derived(story.manifest.value?.start.node ?? null);
  const entry = $derived(story.chapters[studio.chapter]);
  const chapter = $derived(entry?.chapter ?? null);
  const isLastChapter = $derived(studio.chapter === story.chapters.length - 1);
  /** Where the chapter's last node goes on: the first node of the next
   * chapter that has one (the engine's fallthrough). */
  const nextChapterStart = $derived(
    story.chapters.slice(studio.chapter + 1).find((c) => c.chapter?.nodes.length)?.chapter?.nodes[0]?.id,
  );
  const selectedIndex = $derived(chapter?.nodes.findIndex((n) => n.id === studio.node) ?? -1);
  const selected = $derived(selectedIndex >= 0 ? chapter!.nodes[selectedIndex] : null);
  /** The column, where the chapter is edited, or its map. */
  let view = $state<"column" | "graph">("column");

  /** A box of the map opens its node in the column - in its own chapter for
   * a way out of this one. */
  function openFromGraph(id: string) {
    view = "column";
    selectNode(id);
  }

  /** Diagnostics by node, for the marks on the cards. */
  const eventNames = $derived(eventLabels(story.events));
  const marks = $derived.by(() => {
    const out = new Map<string, "error" | "warning">();
    for (const d of studio.diagnostics) {
      if (!d.node || out.get(d.node) === "error") continue;
      out.set(d.node, d.severity);
    }
    return out;
  });

  function add(kind: NodeKind) {
    if (!chapter) return;
    const after = selectedIndex >= 0 ? selectedIndex : null;
    const id = freshNodeId(kind, allNodeIds(story.chapters.map((c) => c.chapter)));
    const following = chapter.nodes[after === null ? chapter.nodes.length : after + 1]?.id;
    const node = newNode(kind, id, {
      content: story.content.find((c) => c.placement === "flow")?.id,
      event: eventIds(story.events)[0],
      following,
    });
    const [nodes] = insertNode(chapter.nodes, after, node);
    chapter.nodes = nodes;
    studio.node = id;
    void saveChapterAt(studio.chapter);
  }

  function saveTitle(e: Event) {
    if (!chapter) return;
    const title = (e.currentTarget as HTMLInputElement).value.trim();
    if (title) chapter.title = title;
    else delete chapter.title;
    void saveChapterAt(studio.chapter);
  }

  function summary(node: FlowNode): string {
    switch (kindOf(node)) {
      case "beat":
        return [...(node.beat?.effects ?? []), node.beat?.advance && `+${node.beat.advance}`, node.beat?.clock]
          .filter(Boolean)
          .join(" · ");
      case "message":
        return node.message ?? "";
      case "event":
        return node.event ? (eventNames[node.event] ?? node.event) : "";
      case "action":
        return node.action ?? "";
      case "side_events": {
        const s = node.side_events ?? {};
        return [
          s.open?.length ? `${t("story.studio.opens")} ${s.open.join(", ")}` : "",
          s.close?.length ? `${t("story.studio.closes")} ${s.close.join(", ")}` : "",
        ]
          .filter(Boolean)
          .join(" · ");
      }
      case "end":
        return node.end ?? "";
      case "branch":
        return "";
    }
  }
</script>

<div class="flow">
  <div class="main">
    {#if !entry}
      <p class="empty">{t("story.studio.noChapter")}</p>
    {:else}
      <header>
        {#if chapter}
          <input
            class="chapter-title"
            value={chapter.title ?? ""}
            placeholder={chapter.id}
            onchange={saveTitle}
            aria-label={t("story.studio.chapterName")}
          />
        {/if}
        <span class="file mono">{entry.file}</span>
        <Seg
          items={[
            { value: "column", label: t("story.studio.viewColumn") },
            { value: "graph", label: t("story.studio.viewGraph") },
          ]}
          value={view}
          onselect={(v) => (view = v as "column" | "graph")}
          tone="neutral"
        />
      </header>

      {#if studio.notice}<p class="warnbox">{studio.notice}</p>{/if}
      {#if studio.unsaved[entry.file]}
        <p class="errbox">{t("story.studio.notSaved", { reason: studio.unsaved[entry.file] })}</p>
      {/if}

      {#if !chapter}
        <p class="errbox">{t("story.studio.unreadable", { reason: entry.error ?? "" })}</p>
      {:else if view === "graph"}
        <FlowGraph
          nodes={chapter.nodes}
          after={isLastChapter ? undefined : nextChapterStart}
          {start}
          {marks}
          onopen={openFromGraph}
        />
      {:else}
        <div class="addbar">
          <span class="lbl-key">{t("story.studio.add")}</span>
          {#each NODE_KINDS as kind (kind)}
            <button class="btn" onclick={() => add(kind)}>{t(KIND_LABEL[kind])}</button>
          {/each}
        </div>

        <ol class="nodes">
          <!-- Keyed by position, not id: a file edited by hand can hold the
               same id twice, which the checks report - the list must still
               show it. -->
          {#each chapter.nodes as node, i (i)}
            {@const kind = kindOf(node)}
            {@const text = summary(node)}
            <li>
              <button class="card" class:on={node.id === studio.node} onclick={() => (studio.node = node.id)}>
                <span class="top">
                  <span class="kind mono">{t(KIND_LABEL[kind])}</span>
                  <span class="id mono">{node.id}</span>
                  {#if node.id === start}<span class="pill">{t("story.studio.start")}</span>{/if}
                  {#if marks.get(node.id) === "error"}
                    <span class="pill pill-err">{t("story.studio.hasError")}</span>
                  {:else if marks.get(node.id) === "warning"}
                    <span class="pill pill-warn">{t("story.studio.hasWarning")}</span>
                  {/if}
                </span>
                {#if text}<span class="sum mono">{text}</span>{/if}
              </button>
              {#each linksOf(node, chapter.nodes[i + 1]?.id ?? nextChapterStart) as link, k (k)}
                <button class="link mono" onclick={() => selectNode(link.target)}>
                  <span class="role">{t(LINK_LABEL[link.role])}</span>
                  {#if link.label}
                    <span class="when">{link.label}</span>
                  {:else if link.when?.length}
                    <span class="when">{link.when.join(" · ")}</span>
                  {:else if link.role === "case"}
                    <!-- A case without conditions is always taken. -->
                    <span class="when">{t("story.studio.always")}</span>
                  {/if}
                  <span class="arrow">→</span>
                  <span class="target">{link.target || "—"}</span>
                </button>
              {/each}
              {#if hasNext(node) && !node.next && i === chapter.nodes.length - 1 && !isLastChapter}
                <span class="link mono quiet">{t("story.studio.toNextChapter")}</span>
              {/if}
            </li>
          {/each}
        </ol>
      {/if}
    {/if}

    <StoryChecks section="flow" />
  </div>

  {#if selected && chapter && view === "column"}
    {#key selected.id}
      <NodePanel index={selectedIndex} />
    {/key}
  {/if}
</div>

<style>
  .flow {
    flex: 1;
    min-width: 0;
    display: flex;
    min-height: 0;
  }
  .main {
    flex: 1;
    min-width: 0;
    overflow: auto;
    padding: 22px 28px;
  }
  header {
    display: flex;
    align-items: baseline;
    gap: 14px;
    margin-bottom: 14px;
  }
  .chapter-title {
    flex: 1;
    min-width: 0;
    background: transparent;
    border: 1px solid transparent;
    color: var(--txt);
    font-size: 18px;
    font-weight: 600;
    padding: 2px 4px;
    margin-left: -5px;
  }
  .chapter-title:hover,
  .chapter-title:focus {
    border-color: var(--line);
    outline: none;
  }
  .file {
    color: var(--muted);
    font-size: 11px;
  }
  .warnbox,
  .errbox {
    margin: 0 0 12px;
  }
  .empty {
    color: var(--muted);
    font-size: 13px;
  }
  .addbar {
    display: flex;
    flex-wrap: wrap;
    align-items: center;
    gap: 6px;
    margin-bottom: 16px;
  }
  .addbar .lbl-key {
    margin-right: 4px;
  }
  .addbar .btn {
    padding: 5px 10px;
    font-size: 11.5px;
  }
  .nodes {
    list-style: none;
    margin: 0;
    padding: 0;
    display: flex;
    flex-direction: column;
    gap: 10px;
    max-width: 720px;
  }
  .card {
    display: flex;
    flex-direction: column;
    gap: 5px;
    width: 100%;
    text-align: left;
    padding: 9px 12px;
    background: var(--card);
    border: 1px solid var(--line);
  }
  .card:hover {
    border-color: var(--faint);
  }
  .card.on {
    border-color: var(--rosso);
  }
  .top {
    display: flex;
    align-items: center;
    gap: 10px;
  }
  .kind {
    font-size: 10px;
    letter-spacing: 1.2px;
    text-transform: uppercase;
    color: var(--muted);
    min-width: 92px;
  }
  .id {
    font-size: 13px;
    color: var(--txt);
  }
  .sum {
    font-size: 11.5px;
    color: var(--txt2);
    padding-left: 102px;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }
  .link {
    display: flex;
    gap: 8px;
    align-items: baseline;
    width: 100%;
    text-align: left;
    background: transparent;
    padding: 4px 12px 0 114px;
    font-size: 11.5px;
    color: var(--muted);
  }
  button.link:hover .target {
    color: var(--txt);
    text-decoration: underline;
  }
  .link .when {
    color: var(--txt2);
  }
  .link .target {
    color: var(--txt2);
  }
  .link.quiet {
    color: var(--faint);
  }
</style>
