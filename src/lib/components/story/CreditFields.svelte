<script lang="ts">
  // The fields of one line of a permission register (SPEC-story-mode,
  // Create Your Own: Package): who made what the line names, under which
  // licence, whether it may travel, where it comes from and the proof.
  // `onchange` hands the parent the change to apply to its own copy of the
  // line, which it saves.
  import { t } from "$lib/i18n/index.svelte";
  import { setText, type CreditLine, type CreditText } from "$lib/story/package";
  import type { Permission } from "$lib/story/studio";

  interface Props {
    line: CreditLine;
    onchange: (apply: (line: CreditLine) => void) => void;
  }
  const { line, onchange }: Props = $props();

  const PERMISSIONS: Permission[] = ["granted", "not_needed", "requested", "not_asked"];
  const PERMISSION_LABEL: Record<Permission, string> = {
    granted: "story.package.granted",
    not_needed: "story.package.notNeeded",
    requested: "story.package.requested",
    not_asked: "story.package.notAsked",
  };
  const NAMES: [CreditText, string][] = [
    ["author", "story.package.author"],
    ["licence", "story.package.licence"],
  ];
  const ORIGIN: [CreditText, string][] = [
    ["source", "story.package.source"],
    ["proof", "story.package.proof"],
  ];

  function field(e: Event): string {
    return (e.currentTarget as HTMLInputElement).value;
  }
</script>

<div class="row">
  {#each NAMES as [key, label] (key)}
    <label>
      <span class="lbl">{t(label)}</span>
      <input class="input" value={line[key] ?? ""} onchange={(e) => onchange((l) => setText(l, key, field(e)))} />
    </label>
  {/each}
  <label>
    <span class="lbl">{t("story.package.permission")}</span>
    <select class="input" value={line.permission} onchange={(e) => onchange((l) => (l.permission = field(e) as Permission))}>
      {#each PERMISSIONS as p (p)}<option value={p}>{t(PERMISSION_LABEL[p])}</option>{/each}
    </select>
  </label>
</div>
<div class="row">
  {#each ORIGIN as [key, label] (key)}
    <label class="wide">
      <span class="lbl">{t(label)}</span>
      <input class="input" value={line[key] ?? ""} onchange={(e) => onchange((l) => setText(l, key, field(e)))} />
    </label>
  {/each}
</div>

<style>
  .row {
    display: flex;
    align-items: flex-end;
    gap: 10px;
    flex-wrap: wrap;
  }
  label {
    display: flex;
    flex-direction: column;
    gap: 3px;
    min-width: 160px;
  }
  .wide {
    flex: 1;
  }
</style>
