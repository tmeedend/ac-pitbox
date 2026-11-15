<script lang="ts">
  // The car's grid thumbnails, first block of Media and documents
  // (docs/SPEC-grille.md GRILLE§5.4, GRILLE§7.1): one per preset in use, in the
  // livery of the thumbnails, each under the name of its preset and of the
  // views that use it — and first, the image the mod (or the game) ships for
  // that livery, to compare. Where one is redone — by Pit Box, or by the game
  // for a protected car — without going through the grid.
  import { untrack } from "svelte";
  import { convertFileSrc } from "@tauri-apps/api/core";
  import { t } from "$lib/i18n/index.svelte";
  import Lightbox, { type LightboxItem } from "$lib/components/ui/Lightbox.svelte";
  import { gridThumbnail, type GridThumb } from "$lib/gridthumbs/gridThumbs";
  import { gridThumb, regenerateGridThumb, requestGridThumb } from "$lib/gridthumbs/gridThumbs.svelte";
  import {
    THUMB_SLOTS,
    gridThumbsOn,
    livePresets,
    presetFor,
    presetName,
    renderTemplate,
  } from "$lib/gridthumbs/gridThumbPrefs.svelte";
  import { inGameProgress, requestInGame } from "$lib/gridthumbs/inGame.svelte";

  interface Props {
    carId: string;
    name: string;
    /** The livery of the thumbnails, `null` for the default one. */
    skin: string | null;
    /** The image the mod ships for that livery, resolved. */
    photo: string | null;
    /** What that image is: the mod's, or the game's. */
    photoLabel: string;
    skinName: string | null;
  }
  let { carId, name, skin, photo, photoLabel, skinName }: Props = $props();
  const presets = $derived(
    livePresets().map((preset) => ({
      preset,
      template: renderTemplate(preset),
      // The views that show it: the same preset can serve several.
      slots: THUMB_SLOTS.filter((slot) => presetFor(slot).id === preset.id).map((slot) =>
        t("settings.gridThumbsFor_" + slot),
      ),
    })),
  );
  /** The images as the grid knows them: they change under us when a render
   * or the game delivers one. */
  const images = $derived(presets.map((p) => gridThumb(p.template, carId, skin)));

  /** What the disk says of each — failed, rendered by the game — read again
   * whenever an image changes. */
  let known = $state<GridThumb[]>([]);
  $effect(() => {
    // Every dependency read before the first `await` (CLAUDE.md, `$effect`).
    const id = carId;
    const livery = skin;
    const list = presets;
    void images;
    untrack(() => {
      for (const p of list) requestGridThumb(p.template, id, livery, name);
    });
    void Promise.all(list.map((p) => gridThumbnail(id, livery, p.template)))
      .then((found) => {
        if (id === carId) known = found;
      })
      .catch((e) => console.error("vignettes de la fiche", e));
  });

  const isProtected = $derived(known.some((k) => k.fromGame || k.failed === "errors.previewProtected"));
  const running = $derived(inGameProgress().running);

  function regenerate() {
    for (const p of presets) void regenerateGridThumb(p.template, carId, skin, name);
  }

  /** Every image of the block, the mod's first, as they show: a slot without
   * an image yet is left out of the viewer. */
  const shown = $derived<{ src: string | null; caption: string }[]>([
    { src: photo, caption: photoLabel },
    ...presets.map((p, i) => ({
      src: images[i] ?? (known[i]?.path ? convertFileSrc(known[i].path) : null),
      caption: presetName(p.preset),
    })),
  ]);
  const lightboxItems = $derived<LightboxItem[]>(
    shown.flatMap((s) => (s.src ? [{ src: s.src, caption: s.caption }] : [])),
  );
  let lightboxIndex = $state<number | null>(null);

  /** Opens the viewer on the image of slot `slot` of [`shown`]. */
  function open(slot: number) {
    lightboxIndex = shown.slice(0, slot).filter((s) => s.src).length;
  }
</script>

{#snippet frame(slot: number, src: string | null)}
  {#if src}
    <button class="frame open" type="button" title={t("lightbox.open")} onclick={() => open(slot)}>
      <img {src} alt="" />
    </button>
  {:else}
    <div class="frame"></div>
  {/if}
{/snippet}

{#if gridThumbsOn() && presets.length > 0}
  <section class="blk">
    <header class="blk-h">
      <span class="blk-t">{t("detail.mediaThumbnailsTitle")}</span>
      <span class="blk-n">{images.filter(Boolean).length}</span>
    </header>
    <div class="blk-b">
      <div class="thumbs">
        <figure class="original">
          {@render frame(0, photo)}
          <figcaption>
            <span class="preset">{photoLabel}</span>
            {#if skinName}<span class="slots">{skinName}</span>{/if}
          </figcaption>
        </figure>
        {#each presets as p, i (p.preset.id)}
          <figure style:--mat-hi={p.preset.mat.hi} style:--mat-lo={p.preset.mat.lo}>
            {@render frame(i + 1, shown[i + 1]?.src ?? null)}
            <figcaption>
              <span class="preset">{presetName(p.preset)}</span>
              {#if p.slots.length}<span class="slots">{p.slots.join(" · ")}</span>{/if}
            </figcaption>
          </figure>
        {/each}
      </div>
      <!-- One way to redo them, the one that works for this car: Pit Box cannot
           read a protected model, so for it "Regenerate" only failed, and the
           game was the only way. Hidden until the disk has said which. -->
      {#if known.length > 0}
        <div class="actions">
          {#if isProtected}
            <button
              class="btn"
              type="button"
              title={t("detail.mediaThumbnailsProtectedHint")}
              disabled={running}
              onclick={() => requestInGame([{ id: carId, skin, name, redo: true }])}
            >
              {t("detail.mediaThumbnailsRegenerateInGame")}
            </button>
          {:else}
            <button class="btn" type="button" onclick={regenerate}>{t("detail.mediaThumbnailsRegenerate")}</button>
          {/if}
        </div>
      {/if}
    </div>
  </section>
{/if}

{#if lightboxIndex !== null}
  <Lightbox items={lightboxItems} startIndex={lightboxIndex} onclose={() => (lightboxIndex = null)} />
{/if}

<style>
  .thumbs {
    display: grid;
    grid-template-columns: repeat(auto-fill, minmax(150px, 1fr));
    gap: 10px;
  }
  figure {
    margin: 0;
    min-width: 0;
  }
  /* The preset's mat behind the image, as on a library card (GRILLE§2.2): a
     cut-out thumbnail is meant to sit on it. Sized like a screenshot of the
     gallery below (`MediaScreenshots`): without `overflow: hidden` a button
     lets its 1024-pixel image set its height, and the thumbnails filled the
     page. */
  .frame {
    position: relative;
    aspect-ratio: 16 / 9;
    overflow: hidden;
    display: block;
    width: 100%;
    padding: 0;
    background: radial-gradient(ellipse at 50% 44%, var(--mat-hi) 0%, var(--mat-lo) 76%);
    border: 1px solid var(--line);
  }
  .open:hover {
    border-color: var(--rosso-border);
  }
  /* The mod's own image carries its own background. */
  .original .frame {
    background: var(--panel2);
  }
  .frame img {
    position: absolute;
    inset: 0;
    display: block;
    width: 100%;
    height: 100%;
    object-fit: contain;
  }
  figcaption {
    display: flex;
    flex-direction: column;
    gap: 1px;
    margin-top: 5px;
  }
  .preset {
    font-size: 12px;
    color: var(--txt);
  }
  .slots {
    font-size: 11px;
    color: var(--muted);
  }
  .actions {
    display: flex;
    gap: 10px;
    margin-top: 12px;
  }
</style>
