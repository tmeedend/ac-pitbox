// Searching a pool of library cards — the question both the library screen and
// the opponent picker ask, word for word: "which of these mods match the
// filters and the search box?".
//
// It lives here rather than in `Library.svelte` because the picker needs the
// same answer, and a second copy would drift the day one of the two grows a
// field (the pack name and the user's note were both added to the haystack
// after the fact — a copy made before that would still be missing them).
//
// **Nothing here may import `filters.ts`**, and the split is not cosmetic:
// that module pulls in the i18n store to label its choices, i18n is a
// `.svelte.ts`, and Vitest compiles this project without the Svelte plugin —
// so one import would take the whole file out of reach of its own tests. The
// filter plumbing that does need those labels lives there instead
// (`buildCardIndex`); what is left here is the part worth testing.

import type { ModCard } from "./library";

/** All three tag origins merged — they are equivalent for filtering; only the
 * detail sheet tells them apart. */
export function modTags(c: Pick<ModCard, "tags_from_mod" | "tags_from_rule" | "tags_manual">): string[] {
  return [...c.tags_from_mod, ...c.tags_from_rule, ...c.tags_manual];
}

/**
 * The search box, on one card.
 *
 * **One term per word, AND between them, each a plain "contains"** — not one
 * glued substring. Real bug: "GT-M Evo" did not bring up "GT-M Adonis Evo".
 * Terms need be neither adjacent nor in order.
 *
 * The haystack includes the pack (§4.4), so searching its name brings up all
 * of its cars, and the user's own note (SESSION§5) — a note one cannot find again
 * is a write-only note.
 */
export function matchesQuery(c: ModCard, query: string): boolean {
  const terms = query.toLowerCase().split(/\s+/).filter(Boolean);
  if (!terms.length) return true;
  const hay =
    `${c.display_name ?? ""} ${c.brand ?? ""} ${c.id_interne} ${c.category ?? ""} ${c.source_pack ?? ""} ${c.notes_user ?? ""} ${modTags(c).join(" ")}`.toLowerCase();
  return terms.every((term) => hay.includes(term));
}
