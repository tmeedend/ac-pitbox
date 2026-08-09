<script lang="ts">
  // Onglet Médias — sous-vue Backgrounds (§6.1), circuits uniquement. Images
  // d'ambiance officielles téléchargées par CSP, filtrées sur le layout
  // actuellement sélectionné sur la fiche (repli automatique côté backend sur
  // les backgrounds génériques du circuit si ce layout n'en a pas — voir
  // media::list_backgrounds). Servent aussi de repli pour le fond photo de
  // l'écran de réglages (§6.2/§9.3).
  import { listMediaBackgrounds, type BackgroundFile } from "$lib/media";
  import { previewSrc } from "$lib/library";
  import { getThumbnail } from "$lib/thumbnails";
  import { errorText } from "$lib/errors";
  import { t } from "$lib/i18n/index.svelte";
  import Lightbox, { type LightboxItem } from "../Lightbox.svelte";

  let {
    modId,
    layoutId,
    onerror,
  }: {
    modId: string;
    layoutId: string | null;
    onerror: (message: string) => void;
  } = $props();

  let files = $state<BackgroundFile[]>([]);
  let lightboxIndex = $state<number | null>(null);
  let thumbs = $state<Record<string, string>>({});

  $effect(() => {
    const current = modId;
    const layout = layoutId;
    files = [];
    thumbs = {};
    lightboxIndex = null;
    listMediaBackgrounds(current, layout)
      .then((f) => {
        if (current === modId && layout === layoutId) files = f;
      })
      .catch((e) => onerror(errorText(e)));
  });

  $effect(() => {
    for (const f of files) {
      if (f.path in thumbs) continue;
      getThumbnail(f.path)
        .then((src) => (thumbs = { ...thumbs, [f.path]: src }))
        .catch(() => {});
    }
  });

  const lightboxItems = $derived<LightboxItem[]>(
    files.map((f) => ({
      src: previewSrc(f.path) ?? "",
      caption: f.layout_id ?? undefined,
    })),
  );
</script>

<section class="blk">
  <header class="blk-h">
    <span class="blk-t">{t("detail.mediaBackgroundsTitle")}</span>
    <span class="blk-n">{files.length}</span>
  </header>
  <div class="blk-b">
    {#if files.length}
      <div class="bg-row">
        {#each files as f, i (f.path)}
          {@const src = thumbs[f.path]}
          <button class="bg-card" type="button" onclick={() => (lightboxIndex = i)} title={t("lightbox.open")}>
            {#if src}<img src={src} alt="" loading="lazy" />{/if}
          </button>
        {/each}
      </div>
      <p class="note">{t("detail.mediaBackgroundsNote")}</p>
    {:else}
      <p class="empty">{t("detail.noBackgrounds")}</p>
    {/if}
  </div>
</section>

{#if lightboxIndex !== null}
  <Lightbox items={lightboxItems} startIndex={lightboxIndex} onclose={() => (lightboxIndex = null)} />
{/if}

<style>
  .bg-row {
    display: grid;
    grid-template-columns: repeat(auto-fill, minmax(180px, 1fr));
    gap: 10px;
  }
  .bg-card {
    aspect-ratio: 16 / 9;
    background: var(--bg);
    border: 1px solid var(--line);
    overflow: hidden;
    padding: 0;
    display: block;
    width: 100%;
  }
  .bg-card:hover {
    border-color: var(--rosso-border);
  }
  .bg-card img {
    width: 100%;
    height: 100%;
    object-fit: cover;
  }
  .note {
    color: var(--blue);
    font-family: var(--mono);
    font-size: 10.5px;
    line-height: 1.5;
    margin-top: 12px;
  }
  .empty {
    color: var(--muted);
    font-size: 12px;
  }
</style>
