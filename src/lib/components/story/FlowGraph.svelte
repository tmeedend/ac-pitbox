<script lang="ts">
  // A chapter as a map, read-only (SPEC-story-mode, In-person dialogues:
  // seeing the shape of a branching story): branches side by side, where
  // they join, the endings, the ways out to other chapters. Laid out by
  // `layoutFlow` each time it is shown; a click on a box opens that node in
  // the column.
  import { t } from "$lib/i18n/index.svelte";
  import { BOX, layoutFlow, type GraphEdge } from "$lib/story/flowGraph";
  import { KIND_LABEL, LINK_LABEL } from "$lib/story/labels";
  import type { FlowNode } from "$lib/story/studio";

  interface Props {
    nodes: FlowNode[];
    /** Where the chapter's last node goes on: the next chapter's first node. */
    after?: string;
    /** The story's start node. */
    start: string | null;
    /** Diagnostics by node. */
    marks: Map<string, "error" | "warning">;
    onopen: (id: string) => void;
  }
  const { nodes, after, start, marks, onopen }: Props = $props();

  const graph = $derived(layoutFlow(nodes, after));
  /** Room for the ways back, right of the boxes. */
  const MARGIN = 40;

  /** A smooth line through the edge's points; a way back bulges right. */
  function path(e: GraphEdge): string {
    const [first, ...rest] = e.points;
    if (e.back) {
      const last = rest[rest.length - 1];
      const out = Math.max(first.x, last.x) + MARGIN / 1.5;
      return `M ${first.x} ${first.y} C ${out} ${first.y}, ${out} ${last.y}, ${last.x} ${last.y}`;
    }
    let d = `M ${first.x} ${first.y}`;
    let prev = first;
    for (const p of rest) {
      const mid = (prev.y + p.y) / 2;
      d += ` C ${prev.x} ${mid}, ${p.x} ${mid}, ${p.x} ${p.y}`;
      prev = p;
    }
    return d;
  }

  /** What an edge says: a reply's label, a case's conditions, a failure or
   * the other way of a branch. The plain "then" says nothing. */
  function caption(e: GraphEdge): string {
    if (e.label) return e.label;
    if (e.role === "case") return e.when?.length ? e.when.join(" · ") : t("story.studio.always");
    if (e.role === "otherwise" || e.role === "failure") return t(LINK_LABEL[e.role]);
    return "";
  }

  /** Where an edge's caption sits: just under its source. */
  function captionAt(e: GraphEdge): { x: number; y: number } {
    const [a, b] = e.points;
    return { x: (a.x + b.x) / 2 + 4, y: a.y + (b.y - a.y) * 0.35 };
  }
</script>

<div class="graph" style:width="{graph.width + MARGIN}px" style:height="{graph.height + 8}px">
  <svg width={graph.width + MARGIN} height={graph.height + 8} aria-hidden="true">
    <defs>
      <marker id="flow-arrow" viewBox="0 0 8 8" refX="7" refY="4" markerWidth="7" markerHeight="7" orient="auto">
        <path d="M 0 0 L 8 4 L 0 8 z" class="head" />
      </marker>
    </defs>
    {#each graph.edges as e, i (i)}
      <path d={path(e)} class="edge {e.role}" class:back={e.back} marker-end="url(#flow-arrow)" />
      {@const text = caption(e)}
      {#if text}
        {@const at = captionAt(e)}
        <text x={at.x} y={at.y} class="caption">{text.length > 28 ? `${text.slice(0, 27)}…` : text}</text>
      {/if}
    {/each}
  </svg>
  {#each graph.boxes as b (b.kind + b.id)}
    <button
      class="box {b.kind}"
      class:start={b.id === start}
      style:left="{b.x}px"
      style:top="{b.y}px"
      style:width="{BOX.w}px"
      style:height="{BOX.h}px"
      title={b.id}
      onclick={() => onopen(b.id)}
    >
      {#if b.kind === "exit"}
        <span class="id mono">→ {b.id}</span>
      {:else}
        <span class="top">
          <span class="kind mono">{t(KIND_LABEL[b.kind])}</span>
          {#if marks.get(b.id) === "error"}
            <span class="dot err" aria-label={t("story.studio.hasError")}></span>
          {:else if marks.get(b.id) === "warning"}
            <span class="dot warn" aria-label={t("story.studio.hasWarning")}></span>
          {/if}
        </span>
        <span class="id mono">{b.id}</span>
      {/if}
    </button>
  {/each}
</div>

<style>
  .graph {
    position: relative;
  }
  svg {
    position: absolute;
    inset: 0;
    overflow: visible;
  }
  .edge {
    fill: none;
    stroke: var(--faint);
    stroke-width: 1.2;
  }
  .edge.failure {
    stroke: var(--yellow);
  }
  .edge.back {
    stroke-dasharray: 4 3;
  }
  .head {
    fill: var(--faint);
  }
  .caption {
    fill: var(--muted);
    font-size: 10px;
    font-family: var(--mono);
  }
  .box {
    position: absolute;
    display: flex;
    flex-direction: column;
    justify-content: center;
    gap: 3px;
    padding: 6px 10px;
    text-align: left;
    background: var(--card);
    border: 1px solid var(--line);
    overflow: hidden;
  }
  .box:hover {
    border-color: var(--faint);
  }
  .box.start {
    border-color: var(--txt2);
  }
  .box.end {
    border-width: 2px;
  }
  .box.exit {
    background: transparent;
    border-style: dashed;
  }
  .top {
    display: flex;
    align-items: center;
    justify-content: space-between;
  }
  .kind {
    font-size: 9.5px;
    letter-spacing: 1.1px;
    text-transform: uppercase;
    color: var(--muted);
  }
  .id {
    font-size: 12px;
    color: var(--txt);
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }
  .exit .id {
    color: var(--txt2);
  }
  .dot {
    width: 7px;
    height: 7px;
    border-radius: 50%;
  }
  .dot.err {
    background: var(--rosso);
  }
  .dot.warn {
    background: var(--yellow);
  }
</style>
