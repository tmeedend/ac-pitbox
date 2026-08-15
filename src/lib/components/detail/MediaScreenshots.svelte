<script lang="ts">
  // Onglet Médias — sous-vue Screenshots (§6.1). Rattachement automatique par
  // nom de fichier (voir media.rs) ; « Associer un fichier » couvre le repli
  // manuel quand une capture n'a pas été retrouvée automatiquement. Le bouton
  // corbeille (et la touche Suppr, sur la vignette focalisée comme dans la
  // visionneuse) envoie le fichier à la corbeille Windows — récupérable, donc
  // sans confirmation.
  import {
    listMediaScreenshots,
    linkMediaManually,
    openMediaFolder,
    trashMediaFile,
    type ScreenshotFile,
  } from "$lib/media";
  import { previewSrc } from "$lib/library";
  import { loadThumbnails } from "$lib/thumbnails";
  import { open } from "@tauri-apps/plugin-dialog";
  import { errorText } from "$lib/errors";
  import { t } from "$lib/i18n/index.svelte";
  import Lightbox, { type LightboxItem } from "../Lightbox.svelte";

  let {
    modId,
    onerror,
  }: {
    modId: string;
    onerror: (message: string) => void;
  } = $props();

  let files = $state<ScreenshotFile[]>([]);
  let linking = $state(false);
  let trashing = $state(false);
  let lightboxIndex = $state<number | null>(null);
  // Miniatures mises en cache (§6.1) : la galerie affiche ça, jamais l'image
  // pleine résolution — seule la visionneuse plein écran (Lightbox) charge
  // l'original.
  let thumbs = $state<Record<string, string>>({});

  // La garde sur `modId` évite qu'une réponse tardive d'une fiche précédente
  // n'écrase la liste de la fiche courante (même principe que ResourcesBlock).
  $effect(() => {
    const current = modId;
    files = [];
    thumbs = {};
    lightboxIndex = null;
    listMediaScreenshots(current).then((f) => {
      if (current === modId) files = f;
    });
  });

  $effect(() => {
    const current = modId;
    loadThumbnails(
      files.map((f) => f.path),
      () => current !== modId,
      (path, src) => (thumbs = { ...thumbs, [path]: src }),
    );
  });

  function fmtDate(iso: string | null): string {
    if (!iso) return "—";
    const d = new Date(iso);
    return Number.isNaN(d.getTime()) ? "—" : d.toLocaleString();
  }

  const lightboxItems = $derived<LightboxItem[]>(
    files.map((f) => ({
      src: previewSrc(f.path) ?? "",
      caption: [f.matched_counterpart, fmtDate(f.modified_at)].filter(Boolean).join(" · "),
    })),
  );

  // Retrait local plutôt que rechargement de la liste : la visionneuse reste
  // ouverte sur la suivante (son index inchangé la désigne déjà), et les
  // miniatures voisines ne repartent pas dans un cycle de chargement. Le
  // backend a retiré le rattachement manuel du fichier, un rechargement
  // ultérieur donnera la même liste.
  //
  // Le verrou `trashing` évite qu'une rafale de Suppr ne relance la même
  // suppression avant que la première n'ait retiré l'entrée.
  async function trashAt(i: number) {
    const f = files[i];
    if (trashing || !f) return;
    trashing = true;
    try {
      await trashMediaFile(f.path);
      files = files.filter((x) => x.path !== f.path);
    } catch (e) {
      onerror(errorText(e));
    } finally {
      trashing = false;
    }
  }

  async function openFolder() {
    try {
      await openMediaFolder("SCREENSHOT");
    } catch (e) {
      onerror(errorText(e));
    }
  }

  async function linkManually() {
    if (linking) return;
    const picked = await open({
      multiple: false,
      title: t("detail.mediaLinkPickTitle"),
      filters: [{ name: "Images", extensions: ["jpg", "jpeg", "png"] }],
    });
    if (!picked || typeof picked !== "string") return;
    linking = true;
    try {
      await linkMediaManually(modId, "SCREENSHOT", picked);
      files = await listMediaScreenshots(modId);
    } catch (e) {
      onerror(errorText(e));
    } finally {
      linking = false;
    }
  }
</script>

<section class="blk">
  <header class="blk-h">
    <span class="blk-t">{t("detail.mediaScreenshotsTitle")}</span>
    <span class="blk-n">{files.length}</span>
  </header>
  <div class="blk-b">
    {#if files.length}
      <div class="gallery">
        {#each files as f, i (f.path)}
          {@const src = thumbs[f.path]}
          <!-- Le bouton corbeille est frère de la vignette, pas dedans : un
               bouton dans un bouton n'est pas du HTML valide. -->
          <div class="shot-wrap">
            <button
              class="shot"
              type="button"
              onclick={() => (lightboxIndex = i)}
              onkeydown={(e) => {
                // La vignette garde le focus DOM pendant que la visionneuse
                // est ouverte, et celle-ci écoute Suppr sur `window` : sans
                // cette garde, une seule pression supprimerait deux images.
                if (e.key === "Delete" && lightboxIndex === null) {
                  e.preventDefault();
                  trashAt(i);
                }
              }}
              title={t("lightbox.open")}
            >
              {#if src}<img src={src} alt={f.file_name} loading="lazy" />{/if}
              {#if f.matched_counterpart}<span class="shot-tag mono">{f.matched_counterpart}</span>{/if}
              <span class="shot-date mono">{fmtDate(f.modified_at)}</span>
            </button>
            <button
              class="shot-del"
              type="button"
              title={t("detail.mediaTrash")}
              disabled={trashing}
              onclick={() => trashAt(i)}
            >
              🗑
            </button>
          </div>
        {/each}
      </div>
    {:else}
      <p class="empty">{t("detail.noScreenshots")}</p>
    {/if}
    <div class="actions">
      <button class="btn-ghost" type="button" onclick={openFolder}>{t("detail.openMediaFolder")}</button>
      <button class="btn-ghost" type="button" onclick={linkManually} disabled={linking}>
        {t("detail.mediaLinkManually")}
      </button>
    </div>
  </div>
</section>

{#if lightboxIndex !== null}
  <Lightbox
    items={lightboxItems}
    startIndex={lightboxIndex}
    onclose={() => (lightboxIndex = null)}
    ondelete={trashAt}
  />
{/if}

<style>
  .gallery {
    display: grid;
    grid-template-columns: repeat(auto-fill, minmax(150px, 1fr));
    gap: 10px;
    margin-bottom: 14px;
  }
  .shot-wrap {
    position: relative;
  }
  .shot {
    position: relative;
    aspect-ratio: 16 / 10;
    background: var(--bg);
    border: 1px solid var(--line);
    overflow: hidden;
    padding: 0;
    display: block;
    width: 100%;
  }
  .shot:hover {
    border-color: var(--rosso-border);
  }
  .shot img {
    width: 100%;
    height: 100%;
    object-fit: cover;
  }
  .shot-tag {
    position: absolute;
    top: 5px;
    left: 5px;
    background: rgba(8, 8, 12, 0.75);
    color: var(--green);
    font-size: 8px;
    letter-spacing: 0.5px;
    padding: 2px 6px;
  }
  /* Révélé au survol (et au focus clavier) : une corbeille visible en
     permanence sur chaque vignette d'une grille en 150px sature la galerie
     de rouge alors que l'action reste marginale. */
  .shot-del {
    position: absolute;
    top: 4px;
    right: 4px;
    background: rgba(8, 8, 12, 0.8);
    border: 1px solid var(--line);
    color: var(--txt2);
    font-size: 11px;
    line-height: 1;
    padding: 4px 6px;
    opacity: 0;
  }
  .shot-wrap:hover .shot-del,
  .shot-wrap:focus-within .shot-del {
    opacity: 1;
  }
  .shot-del:hover {
    border-color: var(--rosso-border);
    color: var(--rosso-bright);
  }
  .shot-date {
    position: absolute;
    bottom: 5px;
    left: 5px;
    background: rgba(8, 8, 12, 0.75);
    color: var(--txt2);
    font-size: 8px;
    padding: 2px 6px;
  }
  .empty {
    color: var(--muted);
    font-size: 12px;
    margin-bottom: 14px;
  }
  .actions {
    display: flex;
    gap: 8px;
  }
</style>
