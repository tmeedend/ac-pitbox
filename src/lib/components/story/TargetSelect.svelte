<script lang="ts">
  // A pick among the story's ids (nodes, content, events). A value that is
  // not among them - typed by hand in the file, or pointing to something
  // since removed - stays shown as it is: the validator reports it, and the
  // select must not quietly replace it with the first valid choice.
  import { t } from "$lib/i18n/index.svelte";

  interface Props {
    value: string;
    options: string[];
    /** The label of the empty choice; without it, the empty choice says
     * that nothing is picked yet. */
    emptyLabel?: string;
    /** What to show for an id (an event's display name); the id otherwise. */
    labels?: Record<string, string>;
    onchange: (value: string) => void;
  }
  const { value, options, emptyLabel, labels, onchange }: Props = $props();

  const all = $derived(value && !options.includes(value) ? [value, ...options] : options);
</script>

<select class="input mono" {value} onchange={(e) => onchange((e.currentTarget as HTMLSelectElement).value)}>
  <option value="">{emptyLabel ?? t("story.studio.pick")}</option>
  {#each all as id (id)}
    <option value={id}>{labels?.[id] ?? id}</option>
  {/each}
</select>
