// The rule the Rules tab should open on arrival - set by a tab that shows the
// rules feeding what it edits (Families › Tracks, "Rules that produce it";
// Brands, "Edit in Rules"). Read and cleared by the Rules tab; a state rather
// than a route parameter, like `brandFocus` and every section of the Workshop
// (`nav.section` is the tab).
import { requestSection } from "$lib/shell/nav.svelte";
import type { Section } from "./rulesEdit";

export const rulesFocus = $state<{ section: Section | null; id: string | null }>({ section: null, id: null });

export async function showRule(section: Section, id: string): Promise<void> {
  rulesFocus.section = section;
  rulesFocus.id = id;
  if (!(await requestSection("rules"))) {
    rulesFocus.section = null;
    rulesFocus.id = null;
  }
}
