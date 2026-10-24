// The read-only graph of a chapter, pure (SPEC-story-mode, In-person
// dialogues: seeing the shape of a branching story). Laid out from the flow
// each time it is shown - nothing about it is stored:
//
// - each node sits on a row, its distance from the chapter's start along
//   the longest way in, so that a node comes below everything leading to it;
// - a way back up (a loop) does not count for the rows: it is drawn as a
//   return, beside the boxes;
// - an arrow that skips rows goes through a waypoint on each row it crosses,
//   which takes a place on that row like a box: branches stand side by side
//   and never cross a box on their way to where they join;
// - within a row, each node is placed below the middle of what leads to it,
//   so that a chain stays straight;
// - a way out of the chapter (to a node of another chapter, the next chapter
//   included) ends on a small box naming where it goes.
import { hasNext, kindOf, linksOf, type LinkRole, type NodeKind } from "./flow";
import type { FlowNode } from "./studio";

/** A box's size and the space between boxes, in CSS pixels. */
export const BOX = { w: 168, h: 50, gapX: 32, gapY: 40 };

export interface GraphBox {
  /** The node's id; for a way out, the id of the node it goes to. */
  id: string;
  /** `exit`: a way out of the chapter. */
  kind: NodeKind | "exit";
  row: number;
  /** Top-left corner. */
  x: number;
  y: number;
}

export interface GraphEdge {
  from: string;
  /** A box's key (`boxKey`). */
  to: string;
  /** `then`: on to the node written after, which the column leaves unsaid. */
  role: LinkRole | "then";
  /** A reply exit's label. */
  label?: string;
  /** A branch case's conditions. */
  when?: string[];
  /** A way back up: drawn beside the boxes. */
  back: boolean;
  /** The points it goes through, from the bottom of its source to the top of
   * its target (or, for a way back, the right sides of both). */
  points: { x: number; y: number }[];
}

export interface FlowGraph {
  boxes: GraphBox[];
  edges: GraphEdge[];
  width: number;
  height: number;
}

/** The key of the box a link ends on: the node, or the way out to it. */
export function boxKey(target: string, inChapter: boolean): string {
  return inChapter ? target : `exit:${target}`;
}

interface RawEdge {
  from: string;
  to: string;
  role: LinkRole | "then";
  label?: string;
  when?: string[];
}

/** The graph of `nodes`, a chapter in file order; `after` is where its last
 * node goes on (the next chapter's first node), if anywhere. */
export function layoutFlow(nodes: FlowNode[], after?: string): FlowGraph {
  // A file edited by hand can hold an id twice: the first one stands.
  const seen = new Set<string>();
  const own = nodes.filter((n) => n.id && !seen.has(n.id) && seen.add(n.id));
  const inChapter = new Set(own.map((n) => n.id));

  const raw: RawEdge[] = [];
  const kinds = new Map<string, GraphBox["kind"]>();
  for (const n of own) kinds.set(n.id, kindOf(n));
  own.forEach((n, i) => {
    // The node written after, the twice-written ids left out.
    const following = own[i + 1]?.id ?? after;
    const links: RawEdge[] = linksOf(n, following).map((l) => ({
      from: n.id,
      to: l.target,
      role: l.role,
      label: l.label,
      when: l.when,
    }));
    if (hasNext(n) && !n.next && following) links.push({ from: n.id, to: following, role: "then" });
    for (const l of links) {
      if (!l.to) continue;
      const key = boxKey(l.to, inChapter.has(l.to));
      if (!inChapter.has(l.to)) kinds.set(key, "exit");
      raw.push({ ...l, to: key });
    }
  });

  // Ways back up: an edge to a node still on the walk's stack.
  const out = new Map<string, RawEdge[]>();
  for (const e of raw) out.set(e.from, [...(out.get(e.from) ?? []), e]);
  const order = [...kinds.keys()];
  const discovered = new Map<string, number>();
  const onStack = new Set<string>();
  const back = new Set<RawEdge>();
  const walk = (id: string) => {
    discovered.set(id, discovered.size);
    onStack.add(id);
    for (const e of out.get(id) ?? []) {
      if (onStack.has(e.to)) back.add(e);
      else if (!discovered.has(e.to)) walk(e.to);
    }
    onStack.delete(id);
  };
  for (const id of order) if (!discovered.has(id)) walk(id);

  // Rows: the longest way in, over the edges that go down.
  const forward = raw.filter((e) => !back.has(e));
  const row = new Map<string, number>(order.map((id) => [id, 0]));
  const inDegree = new Map<string, number>(order.map((id) => [id, 0]));
  for (const e of forward) inDegree.set(e.to, inDegree.get(e.to)! + 1);
  const ready = order.filter((id) => inDegree.get(id) === 0);
  while (ready.length) {
    const id = ready.shift()!;
    for (const e of forward.filter((f) => f.from === id)) {
      row.set(e.to, Math.max(row.get(e.to)!, row.get(id)! + 1));
      inDegree.set(e.to, inDegree.get(e.to)! - 1);
      if (inDegree.get(e.to) === 0) ready.push(e.to);
    }
  }

  // Waypoints: an edge skipping rows is cut into steps of one row.
  type Step = { from: string; to: string };
  const steps: Step[] = [];
  const chains = new Map<RawEdge, string[]>();
  const rank = new Map(row);
  const firstSeen = new Map(discovered);
  for (const e of forward) {
    const [a, b] = [row.get(e.from)!, row.get(e.to)!];
    const chain = [e.from];
    for (let r = a + 1; r < b; r++) {
      const way = `way:${chains.size}:${r}`;
      rank.set(way, r);
      firstSeen.set(way, (discovered.get(e.from) ?? 0) + 0.5);
      chain.push(way);
    }
    chain.push(e.to);
    chains.set(e, chain);
    for (let k = 1; k < chain.length; k++) steps.push({ from: chain[k - 1], to: chain[k] });
  }

  // Order within each row: as discovered, then below the middle of what
  // leads in, a pass down and a pass up.
  const rows: string[][] = [];
  for (const [id, r] of rank) (rows[r] ??= []).push(id);
  for (const r of rows) r?.sort((p, q) => firstSeen.get(p)! - firstSeen.get(q)!);
  const slot = () => new Map(rows.flatMap((r) => r?.map((id, i) => [id, i] as const) ?? []));
  const reorder = (r: string[], near: (id: string) => string[]) => {
    const at = slot();
    const centre = (id: string) => {
      const ns = near(id).map((n) => at.get(n)!);
      return ns.length ? ns.reduce((s, v) => s + v, 0) / ns.length : at.get(id)!;
    };
    r.sort((p, q) => centre(p) - centre(q));
  };
  const preds = (id: string) => steps.filter((s) => s.to === id).map((s) => s.from);
  const succs = (id: string) => steps.filter((s) => s.from === id).map((s) => s.to);
  for (const r of rows.slice(1)) if (r) reorder(r, preds);
  for (const r of rows.slice(0, -1).reverse()) if (r) reorder(r, succs);
  for (const r of rows.slice(1)) if (r) reorder(r, preds);

  // Places: below the middle of what leads in, never over the box before.
  const step = BOX.w + BOX.gapX;
  const x = new Map<string, number>();
  rows.forEach((r, i) => {
    let next = 0;
    for (const id of r ?? []) {
      const ins = preds(id).map((p) => x.get(p)!);
      const wanted = i > 0 && ins.length ? ins.reduce((s, v) => s + v, 0) / ins.length : 0;
      const at = Math.max(wanted, next);
      x.set(id, at);
      next = at + step;
    }
  });
  const y = (id: string) => rank.get(id)! * (BOX.h + BOX.gapY);

  const boxes: GraphBox[] = order.map((id) => ({ id: id.replace(/^exit:/, ""), kind: kinds.get(id)!, row: rank.get(id)!, x: x.get(id)!, y: y(id) }));
  const centre = (id: string) => x.get(id)! + BOX.w / 2;
  const edges: GraphEdge[] = raw.map((e) => {
    const isBack = back.has(e);
    const points = isBack
      ? [
          { x: x.get(e.from)! + BOX.w, y: y(e.from) + BOX.h / 2 },
          { x: x.get(e.to)! + BOX.w, y: y(e.to) + BOX.h / 2 },
        ]
      : chains.get(e)!.map((id, k, all) => ({
          x: centre(id),
          y: k === 0 ? y(id) + BOX.h : k === all.length - 1 ? y(id) : y(id) + BOX.h / 2,
        }));
    return { from: e.from, to: e.to, role: e.role, label: e.label, when: e.when, back: isBack, points };
  });
  const width = Math.max(0, ...[...x.values()].map((v) => v + BOX.w));
  const height = Math.max(0, ...[...rank.values()].map((r) => r * (BOX.h + BOX.gapY) + BOX.h));
  return { boxes, edges, width, height };
}
