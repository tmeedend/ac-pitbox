<script lang="ts">
  // The country a server declares (an ISO code, `DE`), as the game's flag
  // (SPEC-play-online.md, "Liste des serveurs": "Icônes seules : drapeau").
  // The name, in the user's language, is the tooltip — or written beside the
  // flag where there is room (`named`, the panel). A code the game does not
  // know, or a table not loaded yet, shows the code itself: a guessed flag
  // would be a wrong one.
  import { countryLabel, flagFor, gameCountryByIso2, loadFlags } from "$lib/flags.svelte";

  interface Props {
    code: string;
    named?: boolean;
  }
  let { code, named = false }: Props = $props();

  // Idempotent: the first row asks, the others wait on the same read.
  void loadFlags();

  const nation = $derived(gameCountryByIso2(code));
  const flag = $derived(nation ? flagFor(nation.name) : null);
  const label = $derived(nation ? countryLabel(nation.name) : code);
</script>

{#if flag}
  <span class="country" title={label}>
    <img class="flag" src={flag} alt={label} />{#if named}{label}{/if}
  </span>
{:else}
  <span class="country mono" title={label}>{code}</span>
{/if}

<style>
  .country {
    display: inline-flex;
    align-items: center;
    gap: 5px;
  }
  /* The app's one flag size (FilterBar, the library's country column). */
  .flag {
    width: 16px;
    height: 12px;
    object-fit: cover;
    border: 1px solid var(--line);
  }
</style>
