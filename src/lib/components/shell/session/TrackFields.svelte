<script lang="ts">
  // The fields under the session track: its layout and its track skins.
  // Rendered only once a track is picked.
  //
  // Sélecteurs rapides directement depuis le bloc SESSION (évite de passer par
  // la fiche détail pour un changement rapide). Mêmes actions que
  // DetailPage.svelte (mémorise le choix + met à jour le duo de session),
  // réutilisées à l'identique.
  import ImageSelectDropdown from "$lib/components/ui/ImageSelectDropdown.svelte";
  import TrackSkinChecklistDropdown from "./TrackSkinChecklistDropdown.svelte";
  import { nav, pickSession } from "$lib/shell/nav.svelte";
  import { previewSrc, type getModDetail } from "$lib/library/library";
  import { libraryVersion } from "$lib/library/libraryVersion.svelte";
  import { setPreferredLayout } from "$lib/preferred";
  import { syncTrackSkins, listTrackSkinOptions, setTrackSkinActive, type TrackSkinOption } from "$lib/inventory/submods";
  import { t } from "$lib/i18n/index.svelte";

  interface Props {
    /** Fresh details of the session track, `null` while loading. */
    trackDetail: Awaited<ReturnType<typeof getModDetail>>;
  }
  let { trackDetail }: Props = $props();

  let trackSkinOptions = $state<TrackSkinOption[]>([]);
  let trackSkinBusy = $state(false);

  $effect(() => {
    const trackId = nav.sessionTrack?.id ?? null;
    libraryVersion();
    if (!trackId) {
      trackSkinOptions = [];
      return;
    }
    loadTrackSkinOptions(trackId);
  });

  async function loadTrackSkinOptions(trackId: string) {
    await syncTrackSkins(trackId);
    if (nav.sessionTrack?.id !== trackId) return;
    const opts = await listTrackSkinOptions(trackId);
    if (nav.sessionTrack?.id === trackId) trackSkinOptions = opts;
  }

  // Le tracé (outline), pas la photo de fond : plus lisible en petite
  // miniature pour distinguer les layouts d'un même circuit d'un coup d'œil.
  const trackLayoutOptions = $derived(
    (trackDetail?.track?.layouts ?? []).map((l) => ({ id: l.id, name: l.name, image: previewSrc(l.outline) })),
  );
  const trackSkinChecklist = $derived(
    trackSkinOptions.map((o) => ({ name: o.name, image: previewSrc(o.image), active: o.active })),
  );

  function pickTrackLayout(layoutId: string) {
    const track = nav.sessionTrack;
    const d = trackDetail;
    const l = d?.track?.layouts.find((x) => x.id === layoutId);
    if (!track || !d || !l) return;
    setPreferredLayout(d.id_interne, l);
    // `meta` ne porte plus le tracé (SPEC SESSION§1) : il a sa propre ligne
    // juste dessous, il n'y a donc plus rien à y réécrire.
    pickSession("Track", {
      ...track,
      preview: l.preview ?? track.preview,
      layout: l.id,
      outline: l.outline,
    });
  }

  async function toggleTrackSkinFromSlot(name: string, active: boolean) {
    const trackId = nav.sessionTrack?.id;
    if (!trackId || trackSkinBusy) return;
    trackSkinBusy = true;
    try {
      await setTrackSkinActive(trackId, name, active);
      trackSkinOptions = await listTrackSkinOptions(trackId);
    } finally {
      trackSkinBusy = false;
    }
  }
</script>

<ImageSelectDropdown
  label={t("session.fieldLayout")}
  options={trackLayoutOptions}
  selectedId={nav.sessionTrack?.layout ?? null}
  placeholder={t("session.pickLayout")}
  emptyText={t("session.noLayoutsAvailable")}
  staticWhenSingle
  singleNote={t("session.layoutSingle")}
  onselect={pickTrackLayout}
  fit="contain"
/>
<TrackSkinChecklistDropdown
  label={t("session.fieldTrackSkin")}
  options={trackSkinChecklist}
  busy={trackSkinBusy}
  ontoggle={toggleTrackSkinFromSlot}
/>
