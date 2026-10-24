<script lang="ts">
  // The phone (SPEC-story-mode, Content types, templates and frames; Skins):
  // a device with its status bar and the header of the app shown, around one
  // template (`children`). Shared by the Content screen's preview and Story
  // Mode: it knows nothing of the editor.
  //
  // Its look comes from the screen above (`phoneDress`): the story's picture
  // of the device with the screen placed in it, the status bar the Beats set,
  // the wallpaper on the home screen, and the open app's skin. Whatever is
  // missing keeps Pit Box's own theme - the design system's colours, no
  // story art - so a phone story works with no picture at all.
  //
  // The body picture is drawn over the screen, which is cut out of it, so
  // the picture's own corners round the screen's. Its rectangle is in the
  // picture's pixels: it is placed once the picture says its size.
  import type { Snippet } from "svelte";
  import { initials } from "$lib/story/player/phone";
  import { phoneDress, skinFont } from "$lib/story/player/phoneDress";
  import { skinDress } from "$lib/story/player/skins";
  import { mediaUrl, storyFileUrl } from "$lib/story/player/speakers";

  interface Props {
    /** `HH:MM`. */
    clock: string;
    /** The date as the story shows it; empty for none. */
    date: string;
    /** Who the app speaks with: a name. */
    title: string;
    /** Their picture, if they have one. */
    avatar: string | null;
    children: Snippet;
  }
  const { clock, date, title, avatar, children }: Props = $props();

  const dressOf = phoneDress();
  const dress = $derived(dressOf?.() ?? null);
  const look = $derived(dress?.look ?? null);
  const folder = $derived(dress?.folder ?? null);
  const body = $derived(look?.body ?? null);
  const bodyUrl = $derived(mediaUrl(folder, body?.file));
  const wallpaper = $derived(dress?.home ? mediaUrl(folder, look?.wallpaper) : null);
  const status = $derived(look?.status ?? null);
  const statusBar = $derived(look?.status_bar ?? "full");
  const shownDate = $derived(status?.date ?? date);

  /** The family the skin's font is under, once loaded. */
  let font = $state<string | null>(null);
  $effect(() => {
    const [f, path] = [folder, dress?.skin?.font];
    font = null;
    if (!path) return;
    let current = true;
    void skinFont(f, path).then((family) => {
      if (current) font = family;
    });
    return () => {
      current = false;
    };
  });
  const skin = $derived(skinDress(dress?.skin ?? null, (p) => storyFileUrl(folder, p), font));

  /** The picture's size, once loaded: what places the screen in it. */
  let natural = $state<{ w: number; h: number } | null>(null);
  /** The picture did not load: Pit Box's phone instead, never no phone. */
  let broken = $state(false);
  $effect(() => {
    void bodyUrl;
    natural = null;
    broken = false;
  });

  /** The screen's place in the picture, as percentages of it. */
  const place = $derived.by(() => {
    if (!body || !natural) return "";
    const s = body.screen;
    const r = s.radius ?? 0;
    const pc = (n: number, of: number) => `${(n / of) * 100}%`;
    return [
      `left: ${pc(s.x, natural.w)}`,
      `top: ${pc(s.y, natural.h)}`,
      `width: ${pc(s.width, natural.w)}`,
      `height: ${pc(s.height, natural.h)}`,
      `border-radius: ${pc(r, s.width)} / ${pc(r, s.height)}`,
    ].join("; ");
  });
  const wall = $derived(wallpaper ? `background-image: url("${wallpaper}")` : "");
  const style = $derived([place, wall, skin.style].filter(Boolean).join("; "));
</script>

{#snippet screen(inBody: boolean)}
  <div class="phone {skin.classes}" class:in-body={inBody} class:home={wallpaper !== null} {style}>
    {#if statusBar !== "none"}
      <div class="status">
        <span class="clock">{clock}</span>
        {#if statusBar === "full"}
          {#if shownDate}<span class="date">{shownDate}</span>{/if}
          {#if status}
            <span class="indicators" aria-hidden="true">
              {#if status.airplane}
                <span class="plane">✈</span>
              {:else}
                <span class="signal">
                  {#each [1, 2, 3, 4] as bar (bar)}<i class:on={bar <= status.signal} style:height="{bar * 2 + 2}px"></i>{/each}
                </span>
                {#if status.wifi}
                  <svg class="wifi" viewBox="0 0 12 9"
                    ><path d="M1 3.5a7 7 0 0 1 10 0M3 5.5a4 4 0 0 1 6 0" fill="none" stroke="currentColor" stroke-width="1.4" /><circle
                      cx="6"
                      cy="7.6"
                      r="1"
                      fill="currentColor"
                    /></svg
                  >
                {/if}
              {/if}
              <span class="battery" class:low={status.battery <= 15}><i style:width="{status.battery}%"></i></span>
            </span>
          {/if}
        {/if}
      </div>
    {/if}
    <div class="head">
      {#if avatar}
        <img class="avatar" src={avatar} alt="" />
      {:else}
        <span class="avatar initials">{initials(title)}</span>
      {/if}
      <span class="title">{title}</span>
    </div>
    <div class="screen">
      {@render children()}
    </div>
  </div>
{/snippet}

{#if bodyUrl && !broken}
  <div class="device" style:aspect-ratio={natural ? `${natural.w} / ${natural.h}` : undefined}>
    {#if natural}{@render screen(true)}{/if}
    <img
      class="body"
      src={bodyUrl}
      alt=""
      onload={(e) => {
        const img = e.currentTarget as HTMLImageElement;
        natural = { w: img.naturalWidth, h: img.naturalHeight };
      }}
      onerror={() => (broken = true)}
    />
  </div>
{:else}
  {@render screen(false)}
{/if}

<style>
  .phone {
    width: 300px;
    height: 600px;
    display: flex;
    flex-direction: column;
    border-radius: 34px;
    border: 8px solid var(--mat);
    background: var(--panel2);
    overflow: hidden;
    box-shadow: 0 0 0 1px var(--mat-line);
    /* A skin's slots scale with the phone's width (`skins.ts`). */
    container-type: inline-size;
  }
  .phone.skinned {
    font-family: var(--skin-font, inherit);
  }
  /* In the story's picture of the device: placed on its screen, the picture
     itself drawing the edge. */
  .device {
    position: relative;
    height: 640px;
    min-width: 120px;
  }
  .phone.in-body {
    position: absolute;
    border: none;
    box-shadow: none;
  }
  .body {
    position: absolute;
    inset: 0;
    width: 100%;
    height: 100%;
    pointer-events: none;
  }
  .phone.home {
    background-size: cover;
    background-position: center;
  }
  .status {
    display: flex;
    align-items: center;
    gap: 8px;
    padding: 8px 18px 4px;
    font-size: 11px;
    color: var(--txt2);
  }
  .clock {
    font-weight: 600;
  }
  .date {
    color: var(--muted);
  }
  .indicators {
    margin-left: auto;
    display: flex;
    align-items: center;
    gap: 6px;
  }
  .signal {
    display: flex;
    align-items: flex-end;
    gap: 1.5px;
    height: 10px;
  }
  .signal i {
    width: 2.5px;
    background: var(--muted);
    opacity: 0.4;
  }
  .signal i.on {
    background: var(--txt2);
    opacity: 1;
  }
  .wifi {
    width: 12px;
    height: 9px;
  }
  .battery {
    position: relative;
    width: 20px;
    height: 9px;
    border: 1px solid var(--txt2);
    border-radius: 2px;
    padding: 1px;
    display: flex;
  }
  .battery i {
    display: block;
    background: var(--txt2);
    border-radius: 1px;
  }
  .battery.low i {
    background: var(--rosso);
  }
  .head {
    display: flex;
    align-items: center;
    gap: 10px;
    padding: 8px 14px;
    border-bottom: 1px solid var(--line);
  }
  .avatar {
    width: 30px;
    height: 30px;
    border-radius: 50%;
    object-fit: cover;
    flex-shrink: 0;
  }
  .initials {
    display: flex;
    align-items: center;
    justify-content: center;
    background: var(--mat-hi);
    color: var(--txt2);
    font-size: 11px;
    font-weight: 600;
  }
  .title {
    color: var(--txt);
    font-size: 13px;
    font-weight: 600;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }
  .screen {
    flex: 1;
    min-height: 0;
    display: flex;
    flex-direction: column;
  }

  /* A skin's slots (`skins.ts`): an image laid as a 9-slice over the piece
     it dresses, which drops its own background and border. Written once
     here for every template's pieces, by their class names in the views. */
  .phone.has-background {
    background-image: var(--slot-background);
    background-size: cover;
  }
  .phone.has-header .head {
    border: 0 solid transparent;
    border-image-source: var(--slot-header);
    border-image-slice: var(--slot-header-slice);
    border-image-width: var(--slot-header-width);
  }
  .phone.has-bubble_in :global(.bubble.them),
  .phone.has-bubble_out :global(.bubble.me),
  .phone.has-reply :global(.reply),
  .phone.has-row :global(.row),
  .phone.has-row :global(.statement li),
  .phone.has-row :global(.agenda li),
  .phone.has-post :global(.post) {
    background: none;
    border: 0 solid transparent;
    border-radius: 0;
  }
  .phone.has-bubble_in :global(.bubble.them) {
    border-image: var(--slot-bubble_in) var(--slot-bubble_in-slice) / var(--slot-bubble_in-width);
  }
  .phone.has-bubble_out :global(.bubble.me) {
    border-image: var(--slot-bubble_out) var(--slot-bubble_out-slice) / var(--slot-bubble_out-width);
  }
  .phone.has-reply :global(.reply) {
    border-image: var(--slot-reply) var(--slot-reply-slice) / var(--slot-reply-width);
  }
  .phone.has-row :global(.row),
  .phone.has-row :global(.statement li),
  .phone.has-row :global(.agenda li) {
    border-image: var(--slot-row) var(--slot-row-slice) / var(--slot-row-width);
  }
  .phone.has-post :global(.post) {
    border-image: var(--slot-post) var(--slot-post-slice) / var(--slot-post-width);
  }
</style>
