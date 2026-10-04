<script lang="ts">
  // A readiness level in words and colour (SPEC-play-online.md, "Liste des
  // serveurs"), the same in the list's state column and in a server's panel.
  // Written in three places, the colours had already parted: the panel kept
  // « Bloqué » grey after the table turned it yellow.
  //
  // Colour only: the caller's wrapper decides size, case and placement, which
  // differ between a table cell and a car card — the text inherits them.
  import { levelText } from "$lib/online/labels";
  import type { Level } from "$lib/online/online";

  interface Props {
    level: Level;
    /** The DLC a blocked car needs: named rather than the bare "Blocked". */
    dlc?: string | null;
    title?: string;
  }
  let { level, dlc = null, title }: Props = $props();
</script>

<span class="level {level}" {title}>{level === "blocked" && dlc ? dlc : levelText(level)}</span>

<style>
  .level {
    color: var(--muted);
  }
  /* Ready is the normal case and stays quiet; colour is for what asks
     something: blue one click, orange something to fetch, yellow blocked —
     red is kept for what the session retains (SPEC §7.2ter). */
  .ready {
    color: var(--faint);
  }
  .oneClick {
    color: var(--blue);
  }
  .download {
    color: var(--orange);
  }
  .blocked {
    color: var(--yellow);
  }
</style>
