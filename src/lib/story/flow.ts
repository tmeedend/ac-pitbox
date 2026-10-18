// The flow screen's logic, pure (SPEC-story-mode, Story flow): what kind a
// node is, where it leads, and the edits the screen makes to a chapter.
// Nothing here decides whether a story is right - that is the engine's
// validator, run after every save. These functions only keep an edit from
// producing a file the engine cannot read.
import type { Chapter, FlowNode } from "./studio";

export type NodeKind = "beat" | "message" | "event" | "branch" | "action" | "side_events" | "end";

/** In the order the "add" bar offers them: the spec's table, then the end. */
export const NODE_KINDS: NodeKind[] = ["beat", "message", "event", "branch", "action", "side_events", "end"];

export function kindOf(node: FlowNode): NodeKind {
  return NODE_KINDS.find((k) => node[k] !== undefined) ?? "beat";
}

/** Whether the node goes on to another: a branch picks its own way, an end
 * stops. Only these two have no `next`. */
export function hasNext(node: FlowNode): boolean {
  const kind = kindOf(node);
  return kind !== "branch" && kind !== "end";
}

export type LinkRole = "next" | "failure" | "case" | "otherwise";

/** An arrow leaving a node. The plain "then the node below" is not one: it
 * is drawn by the order itself. */
export interface Link {
  role: LinkRole;
  target: string;
  /** For a branch case: the conditions that take it. */
  when?: string[];
}

export function linksOf(node: FlowNode): Link[] {
  const out: Link[] = [];
  for (const c of node.branch ?? []) out.push({ role: "case", target: c.goto, when: c.if });
  if (node.otherwise !== undefined) out.push({ role: "otherwise", target: node.otherwise });
  if (node.on_failure) out.push({ role: "failure", target: node.on_failure });
  if (node.next) out.push({ role: "next", target: node.next });
  return out;
}

export function allNodeIds(chapters: (Chapter | null)[]): string[] {
  return chapters.flatMap((c) => c?.nodes.map((n) => n.id) ?? []);
}

/** Node ids are written in files and in links: letters, digits and `_`. */
export function isNodeId(id: string): boolean {
  return /^[A-Za-z0-9_]+$/.test(id);
}

/** `<kind>_<n>`, the first one free in the whole story: ids are unique
 * across chapters. */
export function freshNodeId(kind: NodeKind, taken: Iterable<string>): string {
  const used = new Set(taken);
  for (let n = 1; ; n++) {
    const id = `${kind}_${n}`;
    if (!used.has(id)) return id;
  }
}

export interface NewNodeContext {
  /** The first flow content a message could send, if any. */
  content?: string;
  /** The first event, if any. */
  event?: string;
  /** The node after the insertion point: a branch's default way out. */
  following?: string;
}

/** A node of `kind`, with every key its kind needs so the file reads. What is
 * left empty (an event to pick, a branch target) the validator then points
 * at: an honest error rather than a guessed value. */
export function newNode(kind: NodeKind, id: string, ctx: NewNodeContext = {}): FlowNode {
  switch (kind) {
    case "beat":
      return { id, beat: {} };
    case "message":
      return { id, message: ctx.content ?? "" };
    case "event":
      return { id, event: ctx.event ?? "" };
    case "branch":
      return { id, branch: [], otherwise: ctx.following ?? "" };
    case "action":
      return { id, action: "" };
    case "side_events":
      return { id, side_events: {} };
    case "end":
      return { id, end: "end" };
  }
}

/** `nodes` with `node` inserted after index `after` (at the end for
 * `null`), and where it landed. */
export function insertNode(nodes: FlowNode[], after: number | null, node: FlowNode): [FlowNode[], number] {
  const at = after === null ? nodes.length : after + 1;
  return [[...nodes.slice(0, at), node, ...nodes.slice(at)], at];
}

/** `items` with the one at `index` moved by `delta`, clamped to the ends:
 * a node in its chapter, a step on the relation scale. */
export function moveItem<T>(items: T[], index: number, delta: number): T[] {
  const to = Math.max(0, Math.min(items.length - 1, index + delta));
  if (to === index) return items;
  const out = [...items];
  const [item] = out.splice(index, 1);
  out.splice(to, 0, item);
  return out;
}

/** Renames node `from` to `to` and every link that points to it, in every
 * chapter, in place. Returns the indices of the chapters that changed - each
 * is a file to save. */
export function renameNode(chapters: (Chapter | null)[], from: string, to: string): number[] {
  const changed: number[] = [];
  chapters.forEach((c, i) => {
    if (!c) return;
    let touched = false;
    const swap = <K extends "id" | "next" | "on_failure" | "otherwise">(n: FlowNode, key: K) => {
      if (n[key] === from) {
        n[key] = to;
        touched = true;
      }
    };
    for (const n of c.nodes) {
      swap(n, "id");
      swap(n, "next");
      swap(n, "on_failure");
      swap(n, "otherwise");
      for (const k of n.branch ?? []) {
        if (k.goto === from) {
          k.goto = to;
          touched = true;
        }
      }
    }
    if (touched) changed.push(i);
  });
  return changed;
}

/** The file and number of a chapter added after `files`: one more than the
 * highest `flow/NN-…` prefix, since chapters play in file-name order. */
export function nextChapterFile(files: string[]): { file: string; number: number } {
  const numbers = files.map((f) => Number(/^flow\/(\d+)/.exec(f)?.[1] ?? 0));
  const number = Math.max(0, ...numbers) + 1;
  return { file: `flow/${String(number).padStart(2, "0")}-chapter.yaml`, number };
}

/** `chNN` for chapter `number`, or the first free one after it: a chapter
 * id is unique in the story. */
export function freshChapterId(number: number, chapters: (Chapter | null)[]): string {
  const taken = new Set(chapters.map((c) => c?.id));
  for (let n = number; ; n++) {
    const id = `ch${String(n).padStart(2, "0")}`;
    if (!taken.has(id)) return id;
  }
}

/** A sentence list as typed in a field, one per line, blanks dropped. */
export function parseLines(text: string): string[] {
  return text
    .split("\n")
    .map((l) => l.trim())
    .filter((l) => l.length > 0);
}
