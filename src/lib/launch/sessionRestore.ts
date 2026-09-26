// Loading a saved session puts its car and its track back in the session pair
// (SESSION§3.5): the car and its livery, the track, its layout and its track
// skins. What cannot be found is added to `warnings`, never thrown: une
// session enregistrée survit à des années de bibliothèque remaniée — l'échec
// partiel est le cas normal, pas l'exception.
//
// Tout passe par le duo de session (`pickSession`) et non par `setup` : le duo
// est la source de vérité (SESSION§3), l'effet de resynchronisation de l'écran
// réécrirait sinon `setup` avec ce qui est resté dans la barre latérale.
import { errorText } from "$lib/errors";
import { t } from "$lib/i18n/index.svelte";
import { listTrackSkinOptions, setTrackSkinActive, syncTrackSkins } from "$lib/inventory/submods";
import type { SkinItem } from "$lib/launch/launch";
import { getModDetail, type ModCard } from "$lib/library/library";
import { getPreferredSkin, setPreferredLayout, setPreferredSkin } from "$lib/preferred";
import { pickSession } from "$lib/shell/nav.svelte";

/** Rétablit la voiture pilotée et son skin. `skinsOf` is the screen's livery
 * cache, shared with the opponents grid. */
export async function restoreCar(
  carId: string,
  skinId: string | null,
  library: ModCard[],
  skinsOf: (carId: string) => Promise<SkinItem[]>,
  warnings: string[],
): Promise<void> {
  if (!carId) return;
  const card = library.find((c) => c.id_interne === carId && c.kind === "Car");
  if (!card) {
    warnings.push(t("launch.loadWarnCarMissing", { id: carId }));
    return;
  }
  const skins = await skinsOf(carId);
  // Sans skin enregistré — le cas de tout preset Content Manager, dont le
  // format n'a pas de champ pour lui (SESSION§3.6) —, c'est la mémoire par
  // voiture qui décide, comme partout ailleurs dans l'app. Prendre `null`
  // pour « aucun skin » déshabillerait la voiture au chargement.
  const wanted = skinId ?? getPreferredSkin(carId)?.id ?? null;
  const skin = wanted ? skins.find((sk) => sk.id === wanted) ?? null : null;
  if (skinId && !skin) warnings.push(t("launch.loadWarnCarSkinMissing", { id: skinId }));
  if (skinId && skin) setPreferredSkin(carId, skin);
  const meta = [card.brand, card.year].filter(Boolean).join(" · ");
  pickSession("Car", {
    id: carId,
    name: card.display_name ?? carId,
    meta,
    preview: skin?.preview ?? card.preview,
    layout: null,
    skin: skin?.id ?? null,
    outline: null,
  });
}

/** Rétablit le circuit, son tracé et ses skins. Même principe que
 * `restoreCar` : tout passe par le duo de session. */
export async function restoreTrack(
  trackId: string,
  layoutId: string | null,
  trackSkins: string[] | undefined,
  library: ModCard[],
  warnings: string[],
): Promise<void> {
  if (!trackId) return;
  const card = library.find((c) => c.id_interne === trackId && c.kind === "Track");
  if (!card) {
    warnings.push(t("launch.loadWarnTrackMissing", { id: trackId }));
    return;
  }
  const detail = await getModDetail(trackId).catch(() => null);
  const layouts = detail?.track?.layouts ?? [];
  const layout = layoutId ? layouts.find((l) => l.id === layoutId) ?? null : null;
  if (layoutId && !layout) warnings.push(t("launch.loadWarnLayoutMissing", { id: layoutId }));
  if (layout) setPreferredLayout(trackId, layout);
  // Avant `pickSession`, pas après : la barre latérale recharge sa liste de
  // skins de circuit quand `nav.sessionTrack` change, donc basculer les
  // skins d'abord lui fait lire l'état déjà à jour. Dans l'autre ordre, elle
  // afficherait les cases de l'état précédent jusqu'au prochain changement
  // de circuit.
  await restoreTrackSkins(trackId, trackSkins, warnings);
  const meta = card.author ?? "";
  pickSession("Track", {
    id: trackId,
    name: card.display_name ?? trackId,
    meta,
    preview: layout?.preview ?? card.preview,
    layout: layout?.id ?? null,
    skin: null,
    outline: layout?.outline ?? card.outline,
  });
}

/** Remet exactement le jeu de skins de circuit de la sauvegarde (§8) :
 * ceux qui manquent sont activés, ceux en trop désactivés — un skin resté
 * actif d'une session précédente changerait sinon l'apparence du circuit
 * sans que rien ne le signale. */
async function restoreTrackSkins(trackId: string, wanted: string[] | undefined, warnings: string[]): Promise<void> {
  if (!wanted) return;
  try {
    await syncTrackSkins(trackId);
    const options = await listTrackSkinOptions(trackId);
    const missing = wanted.filter((name) => !options.some((o) => o.name === name));
    if (missing.length) warnings.push(t("launch.loadWarnTrackSkinsMissing", { names: missing.join(", ") }));
    for (const o of options) {
      const active = wanted.includes(o.name);
      if (o.active !== active) await setTrackSkinActive(trackId, o.name, active);
    }
  } catch (e) {
    warnings.push(t("launch.loadWarnTrackSkinsFailed", { error: errorText(e) }));
  }
}
