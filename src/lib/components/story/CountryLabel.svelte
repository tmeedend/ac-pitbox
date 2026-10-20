<script lang="ts">
  // A country as the grid shows it: its flag and its name in the user's
  // language, or a dimmed `fallback` when there is none. Matched as
  // everywhere in the app (`$lib/flags.svelte`): a livery's `USA` or `UK`
  // finds its flag through the aliases.
  import { countryLabel, flagFor } from "$lib/flags.svelte";

  interface Props {
    name: string | null;
    /** Shown dimmed when `name` is empty. */
    fallback: string;
  }
  const { name, fallback }: Props = $props();

  const flag = $derived(flagFor(name));
</script>

<span class="country">
  {#if flag}<img class="flag" src={flag} alt="" />{:else}<span class="flag none"></span>{/if}
  <span class="name" class:auto={!name}>{name ? countryLabel(name) : fallback}</span>
</span>

<style>
  .country {
    display: flex;
    align-items: center;
    gap: 8px;
    min-width: 0;
  }
  /* 4:3 like the game's PNGs, with a rule: many flags have white on an edge. */
  .flag {
    flex: none;
    width: 16px;
    height: 12px;
    object-fit: cover;
    border: 1px solid var(--line);
  }
  .flag.none {
    border-style: dashed;
  }
  .name {
    min-width: 0;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
    font-size: 12px;
    color: var(--txt);
  }
  .name.auto {
    color: var(--faint);
  }
</style>
