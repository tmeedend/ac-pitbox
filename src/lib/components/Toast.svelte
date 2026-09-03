<script lang="ts">
  // One notification of the bottom-right stack (`ToastStack`), shared by
  // everything that lands there: import progress, import reports, new
  // controller detected.
  //
  // Before this component the import overlay carried its own `position: fixed`
  // card and the controller notice was a full-width banner at the top of the
  // screen — two shapes for the same thing, and two cards fixed at the very
  // same corner silently covering each other (a second import hid the first
  // one's report behind it). Positioning belongs to the stack, the frame
  // belongs here; nothing lays itself out on its own any more.
  import type { Snippet } from "svelte";
  import { t } from "$lib/i18n/index.svelte";

  interface Props {
    title: string;
    /** Blue = information, yellow = warning (red is taken by
     * category/session/destructive — couleurs sémantiques, §chantier
     * libellés). */
    tone?: "default" | "info" | "warn";
    /** Small emoji ahead of the title. */
    icon?: string;
    /** Single-line title, ellipsised. For a title that is a moving value (an
     * archive name) rather than a sentence — a sentence must stay readable. */
    truncate?: boolean;
    /** Collapsed = header only. `undefined` means the toast has no body to
     * fold, so no toggle is offered. */
    collapsed?: boolean;
    ontoggle?: () => void;
    /** Renders the ✕. Absent = the toast cannot be dismissed by hand. */
    onclose?: () => void;
    /** Wording of the ✕ when "close" is not what it means ("Later"). */
    closeLabel?: string;
    /** Buttons of the header, left of the ✕. */
    actions?: Snippet;
    children?: Snippet;
  }
  const {
    title,
    tone = "default",
    icon,
    truncate = false,
    collapsed,
    ontoggle,
    onclose,
    closeLabel,
    actions,
    children,
  }: Props = $props();

  const foldable = $derived(ontoggle != null && children != null);
</script>

<div class="toast" class:info={tone === "info"} class:warn={tone === "warn"}>
  <div class="t-head">
    {#if icon}<span class="t-ic">{icon}</span>{/if}
    <!-- The whole title is the toggle, not just a chevron: a collapsed toast is
         a one-line bar, and aiming at a 12px arrow to get the report back is a
         needless miss. -->
    {#if foldable}
      <button
        class="t-title t-toggle"
        class:trunc={truncate}
        type="button"
        onclick={ontoggle}
        aria-expanded={!collapsed}
        title={collapsed ? t("toast.expand") : t("toast.collapse")}
      >
        <span class="t-chev" class:folded={collapsed} aria-hidden="true">▾</span>
        {title}
      </button>
    {:else}
      <span class="t-title" class:trunc={truncate}>{title}</span>
    {/if}
    {#if actions}<span class="t-actions">{@render actions()}</span>{/if}
    {#if onclose}
      <button
        class="t-close"
        type="button"
        onclick={onclose}
        title={closeLabel ?? t("common.close")}
        aria-label={closeLabel ?? t("common.close")}
      >✕</button>
    {/if}
  </div>
  {#if children && !collapsed}
    <div class="t-body">{@render children()}</div>
  {/if}
</div>

<style>
  .toast {
    width: 380px;
    max-width: calc(100vw - 44px);
    background: var(--panel);
    border: 1px solid var(--line);
    box-shadow: 0 10px 28px rgba(0, 0, 0, 0.45);
    font-size: 12px;
    display: flex;
    flex-direction: column;
    /* The body scrolls, the header must not be pushed out of the card. */
    min-height: 0;
  }
  .toast.info {
    border-color: var(--blue-border);
  }
  /* Mêmes teintes que `.warnbox` de `global.css` — le jaune d'alerte n'a pas
     de jeton dédié, et en inventer un ici en ferait un deuxième. */
  .toast.warn {
    border-color: #4a4426;
  }
  .t-head {
    display: flex;
    align-items: center;
    gap: 8px;
    padding: 9px 10px 9px 12px;
    background: var(--panel2);
    flex: none;
  }
  /* Header and body share one background when there is nothing to separate. */
  .toast.info .t-head {
    background: var(--blue-dim);
  }
  .toast.warn .t-head {
    background: #1a1708;
    color: var(--yellow);
  }
  .t-ic {
    flex: none;
    font-size: 14px;
  }
  .t-title {
    flex: 1;
    min-width: 0;
    font-weight: 600;
    color: var(--txt);
    text-align: left;
  }
  .t-title.trunc {
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }
  .t-toggle {
    background: none;
    border: none;
    padding: 0;
    font: inherit;
    font-weight: 600;
    cursor: pointer;
  }
  .t-toggle:hover,
  .t-toggle:focus-visible {
    color: var(--rosso-bright);
  }
  .t-chev {
    display: inline-block;
    margin-right: 4px;
    color: var(--muted);
    transition: transform 0.15s;
  }
  .t-chev.folded {
    transform: rotate(-90deg);
  }
  .t-actions {
    flex: none;
    display: flex;
    gap: 6px;
  }
  .t-close {
    flex: none;
    background: transparent;
    border: none;
    color: var(--muted);
    font-size: 11px;
    padding: 4px 6px;
    cursor: pointer;
  }
  .t-close:hover,
  .t-close:focus-visible {
    color: var(--txt);
  }
  .t-body {
    padding: 8px 12px 10px;
    /* Capped here rather than on the card: a forty-mod report scrolls inside
       its own body while the header stays put. */
    max-height: 50vh;
    overflow-y: auto;
    border-top: 1px solid var(--line);
  }
</style>
