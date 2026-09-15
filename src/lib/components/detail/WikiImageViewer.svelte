<script lang="ts">
  // The article's image viewer (SPEC-wikipedia-fiche-detail.md §9).
  //
  // **It is a viewer, not a browser.** It shows an image we are already
  // displaying, larger, in our own DOM — no third-party markup, no navigation,
  // nothing new reaching the webview. That is the whole reason this was cheap
  // to add where opening linked *articles* in-app is not: the hard parts (the
  // file, its author, its licence) are already in hand, and the Commons-only
  // rule of §9 means nothing under fair use can ever get here.
  //
  // **The credit travels with the image.** Author and licence are what makes
  // displaying a Commons file lawful, so they are part of the viewer, not a
  // decoration that a full-screen mode may drop.
  import { openUrl } from "@tauri-apps/plugin-opener";
  import { t } from "$lib/i18n/index.svelte";
  import { largerImage } from "$lib/wikiHtml";
  import type { WikiImage } from "$lib/wiki";

  /** An article image, plus the caption the page printed under it. */
  export interface ViewerImage extends WikiImage {
    caption?: string;
  }

  interface Props {
    /** The article's images, in reading order. */
    images: ViewerImage[];
    /** Which one is shown. */
    index: number;
    onclose: () => void;
    onnavigate: (index: number) => void;
  }

  let { images, index, onclose, onnavigate }: Props = $props();

  const image = $derived(images[index]);
  const hasPrev = $derived(index > 0);
  const hasNext = $derived(index < images.length - 1);

  // **MediaWiki refuses to upscale past the original**, and nothing in a
  // thumbnail URL says how big that original is. So we ask for the large
  // rendering and keep the thumbnail we already have as the answer to `error`
  // — the image the article showed is never worse than a broken frame.
  let downgraded = $state(false);
  const src = $derived(downgraded ? image.url : largerImage(image.url));

  $effect(() => {
    // A new image deserves its own attempt at the large rendering.
    void index;
    downgraded = false;
  });

  function go(step: number) {
    const next = index + step;
    if (next >= 0 && next < images.length) onnavigate(next);
  }

  function onKey(event: KeyboardEvent) {
    if (event.key === "Escape") {
      event.preventDefault();
      onclose();
    } else if (event.key === "ArrowLeft") {
      go(-1);
    } else if (event.key === "ArrowRight") {
      go(1);
    }
  }
</script>

<svelte:window onkeydown={onKey} />

<!-- The backdrop is a real button rather than a div with a click handler: it is
     the primary way out, so it has to be reachable without a mouse. -->
<div class="viewer">
  <button class="backdrop" type="button" aria-label={t("wiki.viewerClose")} onclick={onclose}></button>

  <figure class="frame">
    <img {src} alt={image.file} onerror={() => (downgraded = true)} />

    <figcaption class="below">
      {#if image.caption}
        <!-- **The caption, not the credit.** One says what is being looked at,
             the other who owns it; dropping the first turns a photograph into
             an anonymous picture. -->
        <p class="caption">{image.caption}</p>
      {/if}
      <div class="credit">
        <span class="who">{image.artist} · {image.licence}</span>
        {#if images.length > 1}
          <span class="count">{index + 1} / {images.length}</span>
        {/if}
        {#if image.descriptionUrl}
          <button class="btn link" type="button" onclick={() => openUrl(image.descriptionUrl).catch(() => {})}>
            {t("wiki.viewerSource")}
          </button>
        {/if}
      </div>
    </figcaption>
  </figure>

  {#if hasPrev}
    <button class="step prev" type="button" aria-label={t("wiki.viewerPrev")} onclick={() => go(-1)}>‹</button>
  {/if}
  {#if hasNext}
    <button class="step next" type="button" aria-label={t("wiki.viewerNext")} onclick={() => go(1)}>›</button>
  {/if}
  <button class="close" type="button" aria-label={t("wiki.viewerClose")} onclick={onclose}>×</button>
</div>

<style>
  /* **`fixed` here covers the fiche, not the window — and that is wanted.**
     `DetailPage` declares `container: detail / inline-size`, and a container
     type brings layout containment, which makes that element the containing
     block for its fixed descendants. So the rail, the session column and the
     title bar stay visible and usable behind the viewer. Anyone "fixing" this
     to cover the whole window would also have to re-solve the zoom problem
     that containment avoids for free. */
  .viewer {
    position: fixed;
    inset: 0;
    z-index: 40;
    display: grid;
    place-items: center;
    padding: 20px;
    background: rgba(8, 8, 12, 0.88);
  }

  .backdrop {
    position: absolute;
    inset: 0;
    border: 0;
    padding: 0;
    background: none;
    cursor: zoom-out;
  }

  .frame {
    position: relative;
    display: flex;
    flex-direction: column;
    gap: 8px;
    margin: 0;
    max-width: min(100%, 1400px);
    max-height: 100%;
    pointer-events: none;
  }

  /* No `vh` anywhere: the viewport and this box are not the same rectangle
     (see the containment note above), and the zoom multiplies one and not the
     other. `min-height: 0` is what lets the image shrink inside the flex
     column instead of pushing the credit off the bottom. */
  .frame img {
    min-height: 0;
    max-width: 100%;
    object-fit: contain;
    border-radius: 6px;
    background: var(--panel);
    pointer-events: auto;
  }

  .below {
    display: flex;
    flex-direction: column;
    gap: 4px;
    pointer-events: auto;
  }
  /* La légende garde la couleur du texte : c'est du contenu de l'article, pas
     une mention de service comme le crédit. */
  .caption {
    margin: 0;
    font-size: 13px;
    line-height: 1.45;
    color: var(--txt);
  }
  .credit {
    display: flex;
    align-items: baseline;
    gap: 10px;
    flex-wrap: wrap;
    font-size: 12px;
    color: var(--muted);
  }
  .who {
    min-width: 0;
  }
  .count {
    color: var(--faint);
    font-variant-numeric: tabular-nums;
  }

  .step,
  .close {
    position: absolute;
    border: 1px solid var(--line);
    background: var(--card);
    color: var(--txt);
    border-radius: 6px;
    cursor: pointer;
    line-height: 1;
  }
  .step:hover,
  .close:hover {
    border-color: var(--rosso-border);
    color: var(--rosso-bright);
  }

  .step {
    top: 50%;
    transform: translateY(-50%);
    width: 34px;
    height: 54px;
    font-size: 22px;
  }
  .prev {
    left: 12px;
  }
  .next {
    right: 12px;
  }

  .close {
    top: 12px;
    right: 12px;
    width: 30px;
    height: 30px;
    font-size: 18px;
  }

  /* A narrow fiche has no room for side arrows over the image: the keyboard
     and the counter still say where one is. */
  @container detail (max-width: 620px) {
    .step {
      display: none;
    }
  }
</style>
