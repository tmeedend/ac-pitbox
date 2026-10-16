<script lang="ts">
  // The picture of a track the library does not hold (SPEC-play-online.md,
  // "Vignette d'un circuit non installé"), in the row and in the panel's
  // banner. The library's own photo is the caller's, drawn before this.
  //
  // In this order: the Commons picture `online/picture.rs` found, credited -
  // in a tooltip here, in clear under the banner -; else a drawn thumbnail:
  // the pictogram of the track's category and the flag of its country, on the
  // neutral background of the cards. Never another track's image.
  import { t } from "$lib/i18n/index.svelte";
  import { flagFor, gameCountryByIso2 } from "$lib/flags.svelte";
  import { previewSrc } from "$lib/library/library";
  import TrackCategoryIcon from "$lib/components/library/TrackCategoryIcon.svelte";
  import { pictureOf } from "$lib/online/trackPictures.svelte";

  interface Props {
    trackId: string;
    /** The track's categories (`trackGlyphCategories`): the first is drawn. */
    categories: string[];
    /** The row's 34 px tile, or the panel's banner. */
    size: "row" | "banner";
  }
  let { trackId, categories, size }: Props = $props();

  const picture = $derived(pictureOf(trackId));
  const src = $derived(picture?.file ? previewSrc(picture.file) : null);
  const flag = $derived(flagFor(gameCountryByIso2(picture?.country)?.name));
  const credit = $derived(
    picture?.artist && picture.licence ? t("online.photoCredit", { artist: picture.artist, licence: picture.licence }) : "",
  );
</script>

{#if src}
  <img class="pic {size}" {src} alt="" title={credit || undefined} loading="lazy" decoding="async" />
{:else}
  <span class="drawn {size}" aria-hidden="true">
    <TrackCategoryIcon tag={categories[0] ?? ""} icon={categories.length ? undefined : ""} size={size === "row" ? 18 : 52} />
    {#if flag}<img class="flag" src={flag} alt="" />{/if}
  </span>
{/if}

<style>
  .pic {
    width: 100%;
    height: 100%;
    object-fit: cover;
    display: block;
  }
  .drawn {
    --tile-bg: var(--card);
    position: relative;
    width: 100%;
    height: 100%;
    display: flex;
    align-items: center;
    justify-content: center;
    background: var(--card);
    color: var(--muted);
  }
  .flag {
    position: absolute;
    object-fit: cover;
    border: 1px solid var(--line);
  }
  .drawn.row .flag {
    right: 3px;
    bottom: 3px;
    width: 12px;
    height: 9px;
  }
  .drawn.banner .flag {
    right: 12px;
    top: 12px;
    width: 30px;
    height: 20px;
  }
</style>
