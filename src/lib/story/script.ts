// The Form view's logic, pure (SPEC-story-mode, Dialogue text): the items it
// adds, the labels it gives, the conditions as one field. The text itself is
// read and written by the engine alone (`readScript`, `writeScript`): nothing
// here parses Ink.
import type { ScriptItem } from "./studio";

export type ScriptItemKind = "line" | "effect" | "choices" | "end";

/** Every choice label in `items`, choice bodies included. */
export function allLabels(items: ScriptItem[]): string[] {
  return items.flatMap((i) =>
    i.kind === "choices" ? i.choices.flatMap((c) => [c.label, ...allLabels(c.body)]) : [],
  );
}

/** `reply_<n>`, the first one free in the whole conversation: a label is
 * unique in its content item, since it is the "reply chosen" fact. */
export function freshLabel(items: ScriptItem[]): string {
  const used = new Set(allLabels(items));
  for (let n = 1; ; n++) {
    const label = `reply_${n}`;
    if (!used.has(label)) return label;
  }
}

/** A new item of `kind`. Its text starts empty, for the checks to point at,
 * rather than with words the author did not write. `root` is the whole
 * conversation, for a new choice's label. */
export function newItem(kind: ScriptItemKind, root: ScriptItem[]): ScriptItem {
  switch (kind) {
    case "line":
      return { kind: "line", text: "" };
    case "effect":
      return { kind: "effect", effect: "" };
    case "choices":
      return { kind: "choices", choices: [{ label: freshLabel(root), text: "", body: [] }] };
    case "end":
      return { kind: "end" };
  }
}

/** Conditions as one field: sentences joined by `and`, as Ink writes them. */
export function conditionsText(when: string[] | undefined): string {
  return (when ?? []).join(" and ");
}

/** The sentences of a condition field; none for a blank one. */
export function parseConditions(text: string): string[] {
  return text
    .split(/\s+and\s+/)
    .map((s) => s.trim())
    .filter((s) => s.length > 0);
}

/** What every level of the Form view needs to know about its item. */
export interface FormContext {
  /** The story's characters, with their portraits' expressions. */
  characters: { id: string; name: string; expressions: string[] }[];
  dialogue: boolean;
  /** The item's sender: who says a line without a speaker. */
  from?: string;
  /** The whole conversation, for a new choice's label. */
  root: ScriptItem[];
}
