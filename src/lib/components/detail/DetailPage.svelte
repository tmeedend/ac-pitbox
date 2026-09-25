<script lang="ts">
  // Full-page detail sheet of a car or a track (§6.3): header, three tabs, and
  // the sheets that open over it (a layer, an attached mod).
  //
  // This component owns the data and the gestures — loading, reloading when
  // the library or the deployed content changes, what gets remembered and
  // pushed to the session. The cards it lays out (`DetailHero`, `PickerCard`,
  // `EngineSoundBlock`, `TrackSkinsBlock`, `DescriptionCard`…) only draw what
  // they are given: most of them are unmounted on every tab switch, and state
  // kept there would be reloaded, or lost, each time.
  import { editBrand } from "$lib/workshop/brandFocus.svelte";
  import { isPlaque } from "$lib/library/brandLogos.svelte";
  import {
    activateMod,
    deactivateMod,
    getModDetail,
    listLibrary,
    listModResources,
    listModExtras,
    openModFolder,
    previewSrc,
    setFavorite,
    setManualTags,
    setModField,
    type ModCard,
    type ModDetail,
    type ModKind,
    type LayoutItem,
    type LayerRow,
    layerLayoutOrigins,
    type LayoutOrigin,
  } from "$lib/library/library";
  import { listMediaScreenshots, listMediaReplays, listMediaBackgrounds } from "$lib/detail/media";
  import { listModSkins, openNativeShowroom, type SkinItem } from "$lib/launch/launch";
  import Tabs from "$lib/components/ui/Tabs.svelte";
  import { getWikiPanel, setWikiLang, wikiLang, type WikiPanel } from "$lib/wiki/wiki";
  import FicheHeader from "./FicheHeader.svelte";
  import { setEntityNote } from "$lib/detail/userMeta";
  import LayerDetail from "./LayerDetail.svelte";
  import { listAttached, type InventoryRow } from "$lib/inventory/inventory";
  import { listOtherMods, type OtherModRow } from "$lib/inventory/others";
  import { tick, untrack } from "svelte";
  import { focusGamepadElement, isGamepadDriving } from "$lib/shell/gamepadNav";
  import {
    exportMod,
    deletePack,
    deleteBrokenMod,
    reinstallFromArchive,
    deleteModVersion,
    profilesUsingVersion,
    type ExportReport,
  } from "$lib/workshop/maintenance";
  import {
    listSubMods,
    activateSound,
    restoreSound,
    syncTrackSkins,
    listActiveTrackSkins,
    setTrackSkinActive,
    type SubModRow,
  } from "$lib/inventory/submods";
  import { open, confirm } from "@tauri-apps/plugin-dialog";
  import { nav, pickSession, requestSection } from "$lib/shell/nav.svelte";
  import { libraryVersion } from "$lib/library/libraryVersion.svelte";
  import { getPreferredSkin, setPreferredSkin, getPreferredLayout, setPreferredLayout } from "$lib/preferred";
  import { t } from "$lib/i18n/index.svelte";
  import { trackLength } from "$lib/detail/trackLength";
  import LayersBlock from "./LayersBlock.svelte";
  import ResourcesBlock from "./ResourcesBlock.svelte";
  import DecisionsBlock from "./DecisionsBlock.svelte";
  import ExtrasBlock from "./ExtrasBlock.svelte";
  import HistoryBlock from "./HistoryBlock.svelte";
  import UpdateBanner from "./UpdateBanner.svelte";
  import ProvenanceBlock from "./ProvenanceBlock.svelte";
  import TagsBlock from "./TagsBlock.svelte";
  import MediaScreenshots from "./MediaScreenshots.svelte";
  import MediaReplays from "./MediaReplays.svelte";
  import MediaBackgrounds from "./MediaBackgrounds.svelte";
  import DetailHero from "./DetailHero.svelte";
  import CarSpecsBlock from "./CarSpecsBlock.svelte";
  import EngineSoundBlock from "./EngineSoundBlock.svelte";
  import PickerCard from "./PickerCard.svelte";
  import TrackInfoBlock from "./TrackInfoBlock.svelte";
  import TrackSkinsBlock from "./TrackSkinsBlock.svelte";
  import DescriptionCard, { type TextTab } from "./DescriptionCard.svelte";
  import AttachedBlock from "./AttachedBlock.svelte";
  import AttachedModSheet from "./AttachedModSheet.svelte";
  import { stopEngine, toggleEngine } from "$lib/detail/enginePlayer.svelte";

  import { errorText } from "$lib/errors";
  interface Props {
    id: string;
    kind: ModKind;
    onclose: () => void;
    onchange?: () => void;
  }
  let { id, kind, onclose, onchange }: Props = $props();
  const isCar = $derived(kind === "Car");

  let detail = $state<ModDetail | null>(null);
  // Onglets de premier niveau de la fiche (§6.1) — réinitialisé à "fiche" à
  // chaque changement d'entité (voir le $effect suivant `id`).
  /** Trois onglets (REFONTE§7.1) : ce que l'objet EST, ce qu'il a produit ou
   * apporté, et ce que son installation a fait. Les six d'avant exposaient la
   * mécanique — Ressources et Ajouts au jeu étaient vides la plupart du temps,
   * et il fallait cliquer pour le découvrir. */
  type DetailTab = "content" | "media" | "install";
  let activeTab = $state<DetailTab>("content");
  /** Grille de vignettes du sélecteur dépliée (§7.3). **Repliée à chaque
   * ouverture de fiche**, sans persistance : la fiche s'ouvre sur l'aperçu et
   * ses données, pas sur une grille de trente livrées. */
  let pickerOpen = $state(false);
  /** Fiche d'une couche ouverte par-dessus celle de l'hôte (REFONTE§8.4) : la fermer
   * y ramène, au lieu de renvoyer à la liste — même règle que la fiche d'un
   * pack. Le nombre de couches sœurs voyage avec elle : la carte Ordre n'a de
   * sens qu'à partir de deux. */
  let openLayer = $state<{ layer: LayerRow; siblings: number } | null>(null);
  /** Ce qui est greffé sur ce mod (§4.3) : livrées, sons, couches, configs CSP,
   * polices, notices. Relu à chaque recomposition — activer une couche change
   * la réponse.
   *
   * C'est la contrepartie de l'inventaire du côté de l'hôte, et la raison pour
   * laquelle une déduction de rattachement peut se tromper sans dommage : au
   * pire il manque un raccourci ici, le mod restant listé là-bas. */
  let attached = $state<InventoryRow[]>([]);
  /** Les documents rattachés : ceux dont tout le contenu est une annexe
   * (§4.5.2). Ils ont leur place dans l'onglet Médias **et** dans le bloc
   * « Posé sur ce mod » — l'un pour les lire, l'autre pour les gérer. */
  const attachedDocs = $derived(attached.filter((a) => a.kind === "DOCUMENT"));

  /** Fiche d'un mod greffé, ouverte par-dessus celle de l'hôte — comme celle
   * d'une couche, et pour la même raison : on y est arrivé DEPUIS ce mod. */
  let openAttached = $state<OtherModRow | null>(null);
  $effect(() => {
    const current = id;
    void contentRevision;
    listAttached(current)
      .then((rows) => {
        if (current === id) attached = rows;
      })
      .catch(() => {
        if (current === id) attached = [];
      });
  });

  /** La ligne complète d'un mod « autre », chargée à la demande : elle coûte un
   * parcours de fichiers qu'on ne paie qu'en ouvrant la fiche. */
  async function openAttachedFiche(row: InventoryRow) {
    if (!row.uid.startsWith("OTHER:")) return;
    try {
      const all = await listOtherMods();
      openAttached = all.find((o) => o.id === row.id) ?? null;
    } catch (e) {
      actionError = errorText(e);
    }
  }
  /** Tracés apportés par une couche active (REFONTE§7.7). La carte des tracés montre
   * l'état **composé** — celui du lancement — et « 2 tracés » y est exact tout
   * en étant trompeur quand l'un des deux vient d'une extension.
   * Relu à chaque recomposition (`contentRevision`) : activer une couche change
   * la réponse. */
  let layoutOrigins = $state<LayoutOrigin[]>([]);
  $effect(() => {
    const current = id;
    void contentRevision;
    if (isCar) {
      layoutOrigins = [];
      return;
    }
    layerLayoutOrigins(current, "Track")
      .then((o) => {
        if (current === id) layoutOrigins = o;
      })
      .catch(() => {
        // Best-effort : sans cette mention, la carte reste juste, elle est
        // seulement moins bavarde.
        if (current === id) layoutOrigins = [];
      });
  });
  const originOf = $derived((layoutId: string) => layoutOrigins.find((o) => o.layout === layoutId) ?? null);
  /** Sub-tab of the text block (§7.4), back on the description whenever
   * another mod is opened. */
  let textTab = $state<TextTab>("desc");

  /** Ce que l'onglet Wikipédia affiche (§7). `null` = la recherche tourne
   * encore, ce que l'onglet dit lui-même — un onglet absent ne se distinguait
   * ni d'un chargement ni d'une fonctionnalité inexistante, d'où l'écart
   * assumé avec la §7.1 : **l'onglet est permanent**. */
  let wikiPanel = $state<WikiPanel | null>(null);

  /** Charge l'article à l'ouverture de la fiche, et à chaque changement de mod.
   *
   * Deux pièges du projet évités ici, tous deux documentés dans CLAUDE.md :
   * l'identifiant est lu **en tête**, avant toute sortie, sans quoi l'effet ne
   * s'abonnerait à rien au premier passage ; et `wikiLang()` passe par
   * `peekUiPref`, dont le cache est un `$state` global — le lire sans
   * `untrack` abonnerait cet effet à *toutes* les préférences de l'app, et
   * bouger un curseur de l'aperçu 3D relancerait la requête réseau. */
  $effect(() => {
    const key = detail?.id_interne ?? null;
    wikiPanel = null;
    if (!key) return;
    const lang = untrack(() => wikiLang());
    let cancelled = false;
    void getWikiPanel(key, lang).then((p) => {
      if (!cancelled) wikiPanel = p;
    });
    return () => {
      cancelled = true;
    };
  });

  /** Recharge l'onglet : changement de langue de lecture (WIKI§5.4, mémorisée
   * globalement et non par mod), ou article associé à la main (WIKI§7.6). */
  async function reloadWiki(lang?: string) {
    const key = detail?.id_interne;
    if (!key) return;
    if (lang) await setWikiLang(lang);
    wikiPanel = null;
    wikiPanel = await getWikiPanel(key, lang);
  }
  // Chiffres affichés entre parenthèses sur les onglets Médias/Ressources —
  // mêmes appels que ceux faits à l'ouverture de l'onglet (media.rs parcourt
  // en direct `screens/`/`replay/`, potentiellement coûteux), mais lancés ici
  // en tâche de fond dès l'ouverture de la fiche : `null` tant que la réponse
  // n'est pas là (onglet affiché sans chiffre plutôt que fiche retardée),
  // silencieux en cas d'échec (juste un indice visuel, pas une action).
  let screenshotsCount = $state<number | null>(null);
  let replaysCount = $state<number | null>(null);
  let resourcesCount = $state<number | null>(null);
  let extrasCount = $state<number | null>(null);
  let backgroundsCount = $state<number | null>(null);
  let skins = $state<SkinItem[]>([]);
  let previewSkin = $state(0);
  let previewLayout = $state(0);
  let sounds = $state<SubModRow[]>([]);
  let soundBusy = $state(false);
  let trackSkins = $state<SubModRow[]>([]);
  let activeTrackSkins = $state<string[]>([]);
  let trackSkinsLoading = $state(true);
  let trackSkinBusy = $state(false);
  let busy = $state(false);
  let actionError = $state("");
  let exporting = $state(false);
  let exportResult = $state<ExportReport | null>(null);
  // Provenance / pack d'origine (§4.4).
  let siblings = $state<ModCard[]>([]);
  let packBusy = $state(false);

  /** Décompte de l'onglet Médias : la **somme** des quatre blocs qu'il réunit
   * (REFONTE§7.8). `null` tant qu'aucun n'a répondu — afficher « (0) » avant de savoir
   * est un mensonge qui dure une seconde ; un seul bloc connu suffit en
   * revanche à donner un chiffre, les autres s'y ajoutent en arrivant. */
  const mediaCount = $derived.by(() => {
    const parts = [screenshotsCount, replaysCount, resourcesCount, isCar ? null : backgroundsCount];
    const known = parts.filter((n): n is number => n !== null);
    if (!known.length) return null;
    // Les documents rattachés comptent pour un chacun : ils rejoignent la
    // liste des ressources, et leur livraison ne contient qu'eux — c'est le
    // cas qui les définit (§4.5.2).
    return known.reduce((a, b) => a + b, 0) + attachedDocs.length;
  });

  const tabItems = $derived.by(() => {
    const count = (n: number | null) => (n !== null ? ` (${n})` : "");
    return [
      { id: "content", label: isCar ? t("detail.tabCar") : t("detail.tabTrack") },
      { id: "media", label: t("detail.tabMedia") + count(mediaCount) },
      // Pas de décompte sur Installation : l'onglet n'est pas une collection
      // mais une section — origine, couches, étiquettes, ajouts au jeu. Y
      // afficher le seul nombre d'ajouts au jeu donnait un « (0) » qui avait
      // l'air de dire que l'onglet était vide (signalé).
      { id: "install", label: t("detail.tabInstall") },
    ];
  });

  // Image héros : voiture → skin sélectionné ; circuit → preview du layout
  // sélectionné ; sinon preview par défaut du mod.
  /**
   * Compteur de recomposition de `content/`.
   *
   * Activer, déplacer ou supprimer une **couche** remplace des fichiers sans
   * changer un seul chemin : le `.kn5` de la voiture et ses `skins/<nom>/
   * preview.jpg` gardent leur nom. La liste des skins se relisait donc bien
   * (le nombre changeait), mais les images restaient celles d'avant — le
   * navigateur les sert par URL — et l'aperçu 3D continuait de montrer le
   * modèle précédent, faute de voir bouger la voiture ou le skin dont il
   * dépend. Ce compteur est la seule chose qui bouge dans ces cas-là, et il
   * suffit à les rafraîchir tous les deux. Bug réel remonté sur
   * `ks_toyota_ae86_tuned`, dont la couche change le modèle **et** les skins.
   */
  let contentRevision = $state(0);

  const heroImg = $derived.by(() => {
    if (isCar && skins[previewSkin]?.preview) return previewSrc(skins[previewSkin].preview, contentRevision);
    const lay = detail?.track?.layouts[previewLayout];
    if (!isCar && lay?.preview) return previewSrc(lay.preview, contentRevision);
    return previewSrc(detail?.preview ?? null, contentRevision);
  });

  async function filterByPack() {
    if (!detail?.source_pack) return;
    if (await requestSection(detail.kind === "Track" ? "tracks" : "cars")) {
      nav.search = detail.source_pack;
    }
  }

  // Ouvre la fiche du pack (§4.4). La fiche du mod reste dessous : la fermer
  // y ramène, ce qui est le seul chemin par lequel on arrive ici.
  function openPack() {
    if (detail?.source_pack) nav.openPack = detail.source_pack;
  }

  async function openSibling(c: ModCard) {
    if (await requestSection(c.kind === "Track" ? "tracks" : "cars")) {
      nav.openMod = c.id_interne;
    }
  }

  // Archive/dossier source conservé pour la version active (§10/§11), s'il y
  // en a un — conditionne l'affichage du bouton « Réinstaller ».
  function keptArchive(d: ModDetail): string | null {
    return d.versions.find((v) => v.id === d.active_version_id)?.kept_archive_path ?? null;
  }

  let deleteBusy = $state(false);
  let reinstallBusy = $state(false);
  let reinstallOk = $state(false);
  /** Ce qu'est devenue la version supprimée (§10) — corbeille ou
   * suppression définitive. Un message, pas une erreur. */
  let versionNotice = $state("");

  // Supprimer de la bibliothèque : action distincte de Désactiver (§10) —
  // efface les fichiers de toutes les versions, jamais réversible sans
  // réimport (sauf réinstallation depuis une archive source conservée).
  async function doDelete() {
    if (!detail || deleteBusy) return;
    const ok = await confirm(t("detail.deleteConfirm", { name: detail.display_name ?? detail.id_interne }), {
      title: t("detail.deleteTitle"),
      kind: "warning",
    });
    if (!ok) return;
    deleteBusy = true;
    actionError = "";
    try {
      await deleteBrokenMod(detail.id_interne);
      onchange?.();
      onclose();
    } catch (e) {
      actionError = errorText(e);
      deleteBusy = false;
    }
  }

  async function doReinstall() {
    if (!detail || reinstallBusy) return;
    const ok = await confirm(t("detail.reinstallConfirm", { name: detail.display_name ?? detail.id_interne }), {
      title: t("detail.reinstallConfirmTitle"),
      kind: "warning",
    });
    if (!ok) return;
    reinstallBusy = true;
    actionError = "";
    reinstallOk = false;
    try {
      await reinstallFromArchive(detail.id_interne);
      await reload();
      onchange?.();
      reinstallOk = true;
    } catch (e) {
      actionError = errorText(e);
    } finally {
      reinstallBusy = false;
    }
  }


  /** Recharge la fiche + les couches + les ressources (après compositing/
   * import) en préservant le layout sélectionné : activer une couche ajoute
   * souvent des layouts (§4.4). */
  /** Enregistre une surcharge (§5bis.3) puis recharge la fiche. `null` =
   * renoncer et revenir à ce qu'annonce le fichier du mod. La bibliothèque est
   * prévenue (`bumpLibraryVersion`, via `onchange`) : un nom change aussi la
   * liste et le bloc SESSION, pas seulement cette page. */
  async function saveOverride(field: "display_name_user" | "description_user", value: string | null) {
    try {
      await setModField(id, field, value);
      await refreshEntity();
      onchange?.();
    } catch (e) {
      actionError = errorText(e);
    }
  }

  /** Note libre (§9). Passe par la commande commune à tous les types plutôt
   * que par `setModField` : c'est la même colonne sur les cinq tables, et le
   * même geste. */
  async function saveNote(value: string | null) {
    try {
      await setEntityNote("MOD", id, value ?? "");
      await refreshEntity();
      onchange?.();
    } catch (e) {
      actionError = errorText(e);
    }
  }

  async function refreshEntity() {
    const current = id;
    const d = await getModDetail(current);
    if (current !== id) return;
    if (d) {
      const prevLayoutId = detail?.track?.layouts[previewLayout]?.id;
      detail = d;
      if (!isCar && d.track) {
        const li = d.track.layouts.findIndex((l) => l.id === prevLayoutId);
        previewLayout = li >= 0 ? li : Math.min(previewLayout, Math.max(0, d.track.layouts.length - 1));
        resyncStaleSessionLayout(current, d.track.layouts);
      }
    }
    if (isCar) {
      // Le skin regardé se retrouve **par son identité**, jamais par son rang :
      // une couche qui va et vient ajoute ou retire des skins, et un index
      // conservé tel quel désigne alors une autre livrée — ou plus rien. Même
      // règle que le layout d'un circuit, quelques lignes plus haut.
      const prevSkinId = skins[previewSkin]?.id;
      const s = await listModSkins(current);
      if (current === id) {
        skins = s;
        const si = s.findIndex((sk) => sk.id === prevSkinId);
        previewSkin = si >= 0 ? si : Math.min(previewSkin, Math.max(0, s.length - 1));
      }
      // Les sons aussi : importer un mod de son pendant que la fiche de sa
      // voiture est ouverte doit le faire apparaître dans la liste. Ils
      // manquaient ici, et seuls un aller-retour hors de la fiche ou une
      // activation les rechargeaient.
      await loadSounds(current);
    } else {
      await loadTrackSkins(current);
    }
  }


  async function uninstallPack() {
    if (!detail?.source_pack || packBusy) return;
    const ok = await confirm(
      t("detail.uninstallConfirm", { pack: detail.source_pack, count: siblings.length + 1 }),
      { title: t("detail.uninstallTitle"), kind: "warning" },
    );
    if (!ok) return;
    packBusy = true;
    actionError = "";
    try {
      await deletePack(detail.source_pack);
      onchange?.();
      onclose();
    } catch (e) {
      actionError = errorText(e);
      packBusy = false;
    }
  }

  async function doExport() {
    if (!detail || exporting) return;
    const dir = await open({ directory: true, multiple: false, title: t("detail.exportDirTitle") });
    if (!dir || typeof dir !== "string") return;
    exporting = true;
    actionError = "";
    exportResult = null;
    try {
      exportResult = await exportMod(detail.id_interne, dir);
    } catch (e) {
      actionError = errorText(e);
    } finally {
      exporting = false;
    }
  }

  // Aperçu 3D natif (acShowroom.exe) : lancé en **process indépendant**, par
  // -dessus l'app, avec les réglages vidéo du jeu. C'est l'utilisateur qui
  // ferme le showroom pour revenir à Pit Box. L'intégration de la fenêtre
  // native dans la page a été tentée puis abandonnée (voir showroom.rs).
  let showroomBusy = $state(false);

  // The in-app 3D preview lives in `DetailHero`, next to the photo it replaces.
  /** Panneau de réglages posé sur l'aperçu. Ouvert, il garde la barre d'outils
   * visible même quand la souris s'en va — sinon régler un curseur la ferait
   * disparaître sous les doigts. */
  let preview3dPanel = $state(false);

  // Résolu une fois les skins de la fiche courante chargés (§skin sélectionné) —
  // `openShowroom` l'attend pour ne jamais ouvrir avant de connaître le skin
  // sélectionné (sinon SKIN= part vide → voiture toute blanche au 1er affichage,
  // course entre le chargement de `detail` et celui de `skins`).
  let skinsLoadResolve: (() => void) | null = null;
  let skinsLoadPromise: Promise<void> = Promise.resolve();

  async function openShowroom() {
    if (!detail || showroomBusy) return;
    showroomBusy = true;
    actionError = "";
    try {
      // Attend que le skin sélectionné soit connu (sinon course possible avec
      // le chargement de la fiche → showroom ouvert sans skin, voiture blanche).
      await skinsLoadPromise;
      await openNativeShowroom(detail.id_interne, skins[previewSkin]?.id ?? null);
    } catch (e) {
      actionError = errorText(e);
    } finally {
      showroomBusy = false;
    }
  }

  $effect(() => {
    const current = id;
    actionError = "";
    activeTab = "content";
    pickerOpen = false;
    textTab = "desc";
    siblings = [];
    previewLayout = 0;
    trackSkinsLoading = true;
    getModDetail(current).then((d) => {
      if (current !== id) return;
      detail = d;
      // Autres entités du même pack (§4.4).
      if (d?.source_pack) {
        listLibrary().then((all) => {
          if (current !== id) return;
          siblings = all.filter((c) => c.source_pack === d.source_pack && c.id_interne !== d.id_interne);
        });
      }
      // Circuit : restaure le layout mémorisé pour cette entité.
      if (d && !isCar && d.track) {
        const savedLayout = getPreferredLayout(current);
        const li = d.track.layouts.findIndex((l) => l.id === savedLayout?.id);
        previewLayout = li >= 0 ? li : 0;
        resyncStaleSessionLayout(current, d.track.layouts);
      }
    });
    if (isCar) {
      skinsLoadPromise = new Promise((resolve) => {
        skinsLoadResolve = resolve;
      });
      // `untrack` obligatoire, et pas par précaution : `getPreferredSkin` lit
      // le cache de `ui_prefs.json`, qui est un `$state`. Lu à découvert dans
      // le corps de cet effet, il l'abonne à **toutes** les préférences de
      // l'app — si bien qu'un curseur de l'aperçu 3D, en écrivant sa valeur,
      // relançait le chargement complet de la fiche : skins rechargés, skin
      // sélectionné réinitialisé, donc aperçu 3D remonté et retour à la photo.
      // C'est une restauration ponctuelle à l'ouverture, jamais une dépendance.
      const savedSkin = untrack(() => getPreferredSkin(current));
      // Livrée demandée par l'inventaire (§4.2). Lue en `untrack` comme la
      // préférence juste au-dessus : c'est une intention ponctuelle, pas une
      // dépendance de cet effet — et elle est consommée, donc remise à `null`.
      const wanted = untrack(() => {
        const w = nav.openSkin;
        nav.openSkin = null;
        return w;
      });
      listModSkins(current)
        .then((s) => {
          if (current !== id) return;
          skins = s;
          // La demande l'emporte sur la préférence enregistrée : on vient de
          // cliquer cette livrée-là.
          const wi = wanted ? s.findIndex((x) => x.id === wanted) : -1;
          const pi = wi >= 0 ? wi : s.findIndex((x) => x.id === savedSkin?.id);
          previewSkin = pi >= 0 ? pi : 0;
          // Venir voir une livrée vaut la choisir : `selectSkin` la mémorise et
          // met à jour le duo de session, exactement comme un clic dans le
          // sélecteur. C'est la convention de l'app — la bibliothèque met déjà
          // la voiture en session dès qu'on la sélectionne — et s'en écarter
          // ici laisserait l'utilisateur sans moyen évident de choisir ce qu'il
          // a sous les yeux.
          if (wi >= 0) selectSkin(wi);
        })
        .finally(() => skinsLoadResolve?.());
      loadSounds(current);
    } else {
      loadTrackSkins(current);
    }

    screenshotsCount = null;
    replaysCount = null;
    resourcesCount = null;
    extrasCount = null;
    listMediaScreenshots(current)
      .then((f) => {
        if (current === id) screenshotsCount = f.length;
      })
      .catch(() => {});
    listMediaReplays(current)
      .then((f) => {
        if (current === id) replaysCount = f.length;
      })
      .catch(() => {});
    listModResources(current)
      .then((f) => {
        if (current === id) resourcesCount = f.length;
      })
      .catch(() => {});
    listModExtras(current)
      .then((f) => {
        if (current === id) extrasCount = f.length;
      })
      .catch(() => {});
  });

  // Curseur manette à l'ouverture de la fiche (§7.4bis). Sans point de départ
  // désigné, `moveFocus` part du premier élément focusable de la page — le
  // bouton « retour » — et rejoindre les skins demandait une dizaine d'appuis.
  // La grille de skins (ou de layouts) est ce qu'on vient régler ici, donc
  // c'est là que le curseur se pose, sur la vignette **sélectionnée** : par
  // défaut la première, celle mémorisée sinon.
  //
  // Une seule fois par mod ouvert : le curseur appartient à l'utilisateur dès
  // qu'il l'a bougé, le lui reprendre à chaque rechargement de la fiche serait
  // pire que de ne rien faire.
  let cursorPlacedFor: string | null = null;
  $effect(() => {
    const current = id;
    // Dépendances explicites : le curseur ne se pose qu'une fois les vignettes
    // rendues, donc après l'arrivée des skins (voiture) ou de la fiche (circuit).
    const ready = isCar ? skins.length > 0 : !!detail?.track?.layouts.length;
    if (!ready || cursorPlacedFor === current || !isGamepadDriving()) return;
    cursorPlacedFor = current;
    tick().then(() => {
      if (current !== id) return;
      focusGamepadElement(document.querySelector<HTMLElement>(".skin.preview"));
    });
  });

  // Chiffre de l'onglet Backgrounds (circuits seulement) : dépend en plus du
  // layout sélectionné (même filtrage que MediaBackgrounds), donc effet
  // séparé plutôt que mêlé au chargement de la fiche ci-dessus.
  const currentLayoutId = $derived(!isCar ? (detail?.track?.layouts[previewLayout]?.id ?? null) : null);
  $effect(() => {
    if (isCar) return;
    const current = id;
    const layout = currentLayoutId;
    backgroundsCount = null;
    listMediaBackgrounds(current, layout)
      .then((f) => {
        if (current === id && layout === currentLayoutId) backgroundsCount = f.length;
      })
      .catch(() => {});
  });

  async function loadSounds(parent: string) {
    const all = await listSubMods(parent);
    if (parent !== id) return;
    sounds = all.filter((s) => s.sub_type === "SOUND");
  }

  async function loadTrackSkins(parent: string) {
    try {
      // Reconnaît d'abord les skins fournis avec le mod (§8) — sinon ils
      // n'apparaîtraient pas encore dans le listSubMods qui suit.
      await syncTrackSkins(parent);
      if (parent !== id) return;
      const [all, active] = await Promise.all([listSubMods(parent), listActiveTrackSkins(parent)]);
      if (parent !== id) return;
      trackSkins = all.filter((s) => s.sub_type === "TRACK_SKIN");
      activeTrackSkins = active;
    } finally {
      if (parent === id) trackSkinsLoading = false;
    }
  }

  async function toggleTrackSkin(name: string) {
    if (trackSkinBusy) return;
    trackSkinBusy = true;
    const wasActive = activeTrackSkins.includes(name);
    try {
      await setTrackSkinActive(id, name, !wasActive);
      activeTrackSkins = wasActive ? activeTrackSkins.filter((n) => n !== name) : [...activeTrackSkins, name];
    } catch (e) {
      actionError = errorText(e);
    } finally {
      trackSkinBusy = false;
    }
  }

  // Un import, une activation ou une suppression peuvent survenir depuis
  // n'importe quel écran (§4.2/§ resynchronisation) et concerner le mod
  // ouvert (ex. une extension importée, désactivée depuis le panneau
  // compact). Dès que la bibliothèque change, recharger la fiche.
  let lastLibraryVersion = libraryVersion();
  $effect(() => {
    const v = libraryVersion();
    if (v === lastLibraryVersion) return;
    lastLibraryVersion = v;
    // Différé hors du suivi réactif : ne dépend que de libraryVersion(),
    // pas de `id` (évite un double rechargement à la navigation).
    queueMicrotask(() => void refreshEntity());
  });

  // Son = bascule exclusive (§8.3) : un seul actif, original restaurable.
  async function pickSound(subId: string | null) {
    if (!detail || soundBusy) return;
    soundBusy = true;
    actionError = "";
    try {
      if (subId) await activateSound(subId);
      else await restoreSound(detail.id_interne);
      await loadSounds(detail.id_interne);
    } catch (e) {
      actionError = errorText(e);
    } finally {
      soundBusy = false;
    }
  }

  /**
   * Écouter une entrée, sans rien déployer — à ne pas confondre avec
   * `pickSound` juste au-dessus, qui remplace les fichiers du jeu.
   *
   * Ne pose pas `soundBusy` : ce drapeau désarme les boutons radio le temps
   * d'un déploiement, et écouter n'en est pas un. Les deux gestes doivent
   * rester possibles en même temps.
   */
  async function listenSound(subId: string | null) {
    if (!detail) return;
    actionError = "";
    try {
      const clip = await toggleEngine(detail.id_interne, subId);
      // Quel échantillon a été retenu, et comment : invisible à l'écran, mais
      // c'est ce qu'il faut dans le journal quand quelqu'un rapporte avoir
      // entendu le klaxon.
      if (clip) {
        console.info(
          `[son moteur] ${clip.codec} #${clip.sampleIndex}` +
            ` ${clip.sampleName ?? "(sans nom)"} par ${clip.pickedBy}, ${clip.seconds.toFixed(1)} s`,
        );
      }
    } catch (e) {
      actionError = errorText(e);
    }
  }

  // Un moteur qui survit à son bouton est un moteur qu'on ne peut plus couper.
  //
  // L'effet **lit `id`** : sans cette lecture il ne se rejouerait jamais, et
  // passer à une voiture voisine (`openSibling`, qui remplace le contenu sans
  // démonter la fiche) laisserait tourner le moteur de la précédente sous la
  // suivante.
  $effect(() => {
    void id;
    return () => stopEngine();
  });

  async function reload() {
    detail = await getModDetail(id);
  }

  // Sélectionner un skin (SESSION§1/§8.3) : mémorisé par voiture ET poussé dans
  // le duo de session (visible dans le menu). Remplace l'ancienne « étoile ».
  function selectSkin(i: number) {
    previewSkin = i;
    if (!detail) return;
    const sk = skins[i];
    if (sk) setPreferredSkin(detail.id_interne, sk);
    // Marque et année seulement (SPEC SESSION§1) : la livrée a sa propre ligne
    // dans la colonne de session.
    const meta = [detail.brand, detail.year].filter(Boolean).join(" · ");
    pickSession("Car", {
      id: detail.id_interne,
      name: detail.display_name ?? detail.id_interne,
      meta,
      preview: sk?.preview ?? detail.preview,
      layout: null,
      skin: sk?.id ?? null,
      outline: null,
    });
    // Un showroom déjà ouvert garde le skin avec lequel il a été lancé : il
    // n'a pas de mécanisme connu pour en changer à chaud (pas d'IPC vers
    // acShowroom.exe, voir docs/showroom-3d-preview-research.md). Le prochain
    // clic sur « Aperçu 3D » prendra le nouveau skin.
  }

  // Sélectionner un layout de circuit : mémorisé + poussé dans le duo de session
  // (photo + tracé en surimpression dans le menu).
  /** Si le layout mémorisé comme choix de session (SESSION§1) pour cette entité a
   * disparu (couche retirée/réordonnée, §4.4) alors qu'il s'agit bien de
   * l'entité de la fiche courante, le resynchronise — sinon un layout fantôme
   * reste affiché dans la barre latérale et proposé au lancement, alors qu'il
   * n'existe plus sur le disque. Suppose `detail` déjà à jour à l'appel. */
  function resyncStaleSessionLayout(trackId: string, layouts: LayoutItem[]): void {
    if (nav.sessionTrack?.id !== trackId || !nav.sessionTrack.layout) return;
    if (layouts.some((l) => l.id === nav.sessionTrack?.layout)) return;
    selectLayout(previewLayout);
  }

  function selectLayout(i: number) {
    previewLayout = i;
    if (!detail?.track) return;
    const l = detail.track.layouts[i];
    if (l) setPreferredLayout(detail.id_interne, l);
    // L'auteur seul : le tracé a sa propre ligne, le nom est juste au-dessus.
    const meta = detail.author ?? "";
    pickSession("Track", {
      id: detail.id_interne,
      name: detail.display_name ?? detail.id_interne,
      meta,
      preview: l?.preview ?? detail.preview,
      layout: l?.id ?? null,
      skin: null,
      outline: l?.outline ?? detail.outline,
    });
  }

  // Ouvre le dossier réel du mod dans l'explorateur Windows (voir aussi
  // ce qu'il y a dedans, en dehors de la fiche). Fonctionne aussi pour le
  // contenu de base Kunos (lecture seule).
  async function openFolder() {
    if (!detail) return;
    try {
      await openModFolder(detail.id_interne);
    } catch (e) {
      actionError = errorText(e);
    }
  }

  async function activate(versionId?: string) {
    if (!detail || busy) return;
    busy = true;
    actionError = "";
    try {
      await activateMod(detail.id_interne, versionId);
      await reload();
      onchange?.();
    } catch (e) {
      actionError = errorText(e);
    } finally {
      busy = false;
    }
  }

  // Supprimer une version rangée (§10). Deux avertissements à donner
  // AVANT que ce soit irréversible : les profils qui l'épinglaient (ils
  // basculeront sur la version en place) et le fait que la corbeille peut
  // refuser une version volumineuse, auquel cas la suppression est définitive.
  // Ce qui a réellement eu lieu revient dans le résultat, et s'affiche.
  async function deleteVersion(versionId: string) {
    if (!detail || busy) return;
    const v = detail.versions.find((ver) => ver.id === versionId);
    const label = v?.version_label ?? t("detail.noVersionNumber");
    let pinned: string[] = [];
    try {
      pinned = await profilesUsingVersion(versionId);
    } catch (e) {
      actionError = errorText(e);
      return;
    }
    const message = [
      t("detail.deleteVersionConfirm", { label }),
      pinned.length ? t("detail.deleteVersionProfiles", { profiles: pinned.join(", ") }) : "",
    ]
      .filter(Boolean)
      .join("\n\n");
    const ok = await confirm(message, { title: t("detail.deleteVersion"), kind: "warning" });
    if (!ok) return;
    busy = true;
    actionError = "";
    versionNotice = "";
    try {
      const outcome = await deleteModVersion(versionId);
      versionNotice = outcome.recycled
        ? t("detail.deleteVersionRecycled", { label })
        : t("detail.deleteVersionPurged", { label });
      await reload();
      onchange?.();
    } catch (e) {
      actionError = errorText(e);
    } finally {
      busy = false;
    }
  }

  async function deactivate() {
    if (!detail || busy) return;
    busy = true;
    actionError = "";
    try {
      await deactivateMod(detail.id_interne);
      await reload();
      onchange?.();
    } catch (e) {
      actionError = errorText(e);
    } finally {
      busy = false;
    }
  }

  async function toggleFav() {
    if (!detail) return;
    detail.is_favorite = !detail.is_favorite;
    await setFavorite(detail.id_interne, detail.is_favorite);
    onchange?.();
  }

  async function addManual(tag: string) {
    if (!detail || detail.tags_manual.includes(tag)) return;
    detail.tags_manual = [...detail.tags_manual, tag];
    await setManualTags(detail.id_interne, detail.tags_manual);
    onchange?.();
  }

  async function removeManual(tag: string) {
    if (!detail) return;
    detail.tags_manual = detail.tags_manual.filter((x) => x !== tag);
    await setManualTags(detail.id_interne, detail.tags_manual);
    onchange?.();
  }

  /** Sous-titre de l'en-tête (§6.1) : ce qui identifie l'objet en une ligne,
   * auteur compris — c'est une propriété du mod, elle n'a rien à faire en bas
   * de colonne. Les parties absentes ne laissent pas de séparateur orphelin. */
  const subtitle = $derived.by(() => {
    const d = detail;
    if (!d) return "";
    // Un circuit n'a ni marque, ni année, ni classe : sa ligne d'identité se
    // réduisait à son auteur. Ce qui l'identifie, c'est sa longueur et le
    // nombre de tracés qu'il porte (maquette écran 7). Le décompte ne
    // s'affiche qu'à partir de deux — « 1 tracé » n'apprend rien, et c'est
    // aussi ce qui évite un pluriel que l'i18n ne sait pas accorder.
    const layouts = d.track?.layouts ?? [];
    const parts = isCar
      ? [d.brand, d.year ? String(d.year) : null, d.car_class ? d.car_class.toUpperCase() : null]
      : [
          trackLength(layouts[previewLayout]?.length),
          layouts.length > 1 ? t("detail.layoutCount", { count: layouts.length }) : null,
        ];
    parts.push(d.author ? t("detail.byAuthor", { author: d.author }) : null);
    return parts.filter(Boolean).join(" · ");
  });

  // Actions de la fiche (§6.3) : le ⋮ de `FicheHeader` les rend et les
  // positionne — ici ne reste que leur liste. Cœur favori et pastille d'état
  // n'y sont pas : ils se lisent en permanence, ce ne sont pas des actions.
  const menuItems = $derived.by(() => {
    const d = detail;
    if (!d) return [];
    const items: { label: string; onclick: () => void; disabled?: boolean; danger?: boolean }[] = [];
    if (!d.is_stock) {
      items.push({
        label: d.active ? t("common.deactivate") : t("common.activate"),
        onclick: d.active ? deactivate : () => activate(),
        disabled: busy,
      });
    }
    if (isCar) {
      items.push({
        label: showroomBusy ? t("detail.showroomLaunching") : t("detail.showroom"),
        onclick: openShowroom,
        disabled: showroomBusy,
      });
    }
    if (isCar && d.brand) {
      const brand = d.brand;
      items.push({ label: t("detail.editBrand", { brand }), onclick: () => void editBrand(brand) });
    }
    items.push({ label: t("detail.openFolder"), onclick: openFolder });
    if (!d.is_stock) {
      items.push({
        label: exporting ? t("detail.exporting") : t("detail.export"),
        onclick: doExport,
        disabled: exporting,
      });
      if (keptArchive(d)) {
        items.push({
          label: reinstallBusy ? t("detail.reinstalling") : t("detail.reinstallFromArchive"),
          onclick: doReinstall,
          disabled: reinstallBusy,
        });
      }
      items.push({
        label: deleteBusy ? t("common.working") : t("detail.deleteFromLibrary"),
        onclick: doDelete,
        disabled: deleteBusy,
        danger: true,
      });
    }
    return items;
  });

</script>

<div class="page">
  <!-- Fiches posées PAR-DESSUS celle du mod : une couche (REFONTE§8.4), un mod greffé
       (§4.3). La fermer ramène ici, ce qui est le chemin par lequel on y est
       arrivé — même disposition que la fiche d'un pack. -->
  {#if openLayer}
    <div class="sheet-wrap">
      <LayerDetail
        layer={openLayer.layer}
        hostName={detail?.display_name ?? null}
        siblingCount={openLayer.siblings}
        onclose={() => (openLayer = null)}
        onchanged={() => {
          contentRevision += 1;
          void refreshEntity();
        }}
      />
    </div>
  {:else if openAttached}
    <div class="sheet-wrap">
      <AttachedModSheet
        bind:row={openAttached}
        hostId={id}
        onattached={(rows) => (attached = rows)}
        onerror={(m) => (actionError = m)}
      />
    </div>
  {:else if !detail}
    <div class="empty">{t("common.loading")}</div>
  {:else}
    {@const d = detail}
    <FicheHeader
      flush
      onback={onclose}
      backLabel={t("detail.backTooltip")}
      glyph={isCar ? "▤" : "◠"}
      image={isCar && d.badge ? previewSrc(d.badge) : null}
      imageAlt={d.brand ?? ""}
      imagePlaque={isCar && isPlaque(d.badge)}
      name={d.display_name ?? d.id_interne}
      {subtitle}
      category={d.category}
      rename={{
        original: d.display_name_file,
        overridden: !!d.display_name_user,
        onsave: (v) => saveOverride("display_name_user", v),
      }}
      deployment={{ active: d.active, stock: d.is_stock, unmanaged: d.is_unmanaged }}
      favorite={{ on: d.is_favorite, ontoggle: toggleFav }}
      actions={menuItems}
    />

    <Tabs flush tabs={tabItems} active={activeTab} onselect={(v) => (activeTab = v as DetailTab)} />

    <UpdateBanner kind={isCar ? "Car" : "Track"} id={d.id_interne} />

    {#if actionError}<div class="errbox">{actionError}</div>{/if}
    {#if reinstallOk}<div class="export-ok">{t("detail.reinstallSuccess")}</div>{/if}
    {#if versionNotice}<div class="export-ok">{versionNotice}</div>{/if}
    {#if exportResult}
      <div class="export-ok">
        {t("detail.exportSuccess", { count: exportResult.included.length })}
        {#if exportResult.warnings.length}
          <ul class="export-warn">{#each exportResult.warnings as w}<li>⚠ {w}</li>{/each}</ul>
        {/if}
      </div>
    {/if}

    {#if activeTab === "content"}
    <!-- RANGÉE HAUTE : héros + panneau données -->
    <div class="row top" class:track={!isCar}>
      <!-- Colonne principale (§7.2) : l'aperçu, le sélecteur qui le pilote,
           puis le bloc textuel en dernier — sa longueur est imprévisible, il ne
           doit donc rien repousser. Le sélecteur est un FRÈRE du héros et non
           son enfant : `.hero` est un conteneur flex centré à ratio fixe, un
           second enfant s'y retrouvait posé au milieu de l'image (signalé). -->
      <div class="maincol">
        <DetailHero
          car={isCar}
          modId={d.id_interne}
          name={d.display_name ?? d.id_interne}
          image={heroImg}
          skinId={skins[previewSkin]?.id ?? null}
          carClass={d.car_class}
          revision={contentRevision}
          outline={isCar ? null : previewSrc(d.track?.layouts[previewLayout]?.outline ?? null)}
          {showroomBusy}
          bind:panelOpen={preview3dPanel}
        />
      </div>

      <div class="data">
        {#if isCar}
          <CarSpecsBlock detail={d} />
          <EngineSoundBlock modId={d.id_interne} {sounds} busy={soundBusy} onpick={pickSound} onlisten={listenSound} />
          <PickerCard
            title={t("detail.skinsLabel")}
            items={skins.map((sk) => ({
              id: sk.id,
              name: sk.name,
              // `livery.png` — couleurs et motif de la livrée seule — et non la
              // photo de la voiture entière : à 20 px dans une liste déroulante,
              // celle-ci ne montre plus rien. Même choix que le sélecteur de la
              // colonne de session, et la convention de CM. La photo reprend ses
              // droits dans la grille dépliée, où elle a la place.
              thumb: previewSrc(sk.livery ?? sk.preview, contentRevision),
              image: previewSrc(sk.preview, contentRevision),
              livery: previewSrc(sk.livery, contentRevision),
            }))}
            index={previewSkin}
            onpick={selectSkin}
            expanded={pickerOpen}
            ontoggle={() => (pickerOpen = !pickerOpen)}
            emptyText={t("detail.noSkins")}
            cellTitle={t("detail.chooseSkinTooltip")}
          />
        {:else}
          <TrackInfoBlock
            detail={d}
            layout={d.track?.layouts[previewLayout]}
            addedLayouts={layoutOrigins.length}
          />
          <TrackSkinsBlock
            skins={trackSkins}
            active={activeTrackSkins}
            loading={trackSkinsLoading}
            busy={trackSkinBusy}
            ontoggle={toggleTrackSkin}
          />
          <!-- Même carte que le sélecteur de livrée, et même raison : il vit
               parmi les cartes de la colonne, il en prend le cadre. -->
          <PickerCard
            title={t("detail.layoutsLabel")}
            fit="contain"
            items={d.track?.layouts.map((l, i) => {
              const from = originOf(l.id);
              const outline = previewSrc(l.outline);
              return {
                id: l.id || String(i),
                name: l.name,
                thumb: outline,
                image: outline,
                origin: from
                  ? {
                      label: t("detail.layoutFromLayer"),
                      title: t("detail.layoutFromLayerTip", { layer: from.layer_name }),
                    }
                  : null,
              };
            }) ?? null}
            index={previewLayout}
            onpick={selectLayout}
            expanded={pickerOpen}
            ontoggle={() => (pickerOpen = !pickerOpen)}
            emptyText={t("detail.singleLayout")}
            cellTitle={t("detail.chooseLayoutTooltip")}
            note={trackLength(d.track?.layouts[previewLayout]?.length) ?? undefined}
          />
        {/if}
      </div>
    </div>

    <!-- ZONE 2 — le bloc de lecture, pleine largeur mais **contenu centré sur
         une largeur de mesure**. Il occupait jusqu'ici le bas de la colonne de
         gauche, où une description de trois mots laissait la colonne de droite
         courir seule sur toute la hauteur de la page. Sorti de la rangée, il la
         laisse se refermer sur la hauteur de l'aperçu, et gagne au passage une
         ligne lisible : un paragraphe qui court sur 900 px se relit mal, l'œil
         perdant le début de la ligne suivante. -->
    <DescriptionCard
      car={isCar}
      modKey={d.id_interne}
      text={isCar ? (d.specs?.description ?? null) : (d.track?.description ?? null)}
      overridden={!!d.description_user}
      notes={d.notes_user ?? null}
      {wikiPanel}
      bind:tab={textTab}
      ondescription={(v) => saveOverride("description_user", v)}
      onnote={saveNote}
      onreloadwiki={reloadWiki}
    />

    {:else if activeTab === "media"}
      <!-- Quatre blocs, deux groupes (REFONTE§7.8) : ce que TU as produit (captures,
           replays), puis ce qui est LIVRÉ avec le mod (ressources, fonds).
           Aucun n'est masqué quand il est vide : ses actions « Ouvrir le
           dossier » et « Lier un fichier… » sont la seule voie pour y ajouter
           quelque chose. -->
      <div class="tab-body stack">
        <MediaScreenshots modId={id} onerror={(m) => (actionError = m)} oncount={(n) => (screenshotsCount = n)} />
        <MediaReplays modId={id} onerror={(m) => (actionError = m)} oncount={(n) => (replaysCount = n)} />
        <!-- Les documents livrés AVEC ce mod mais rangés à part (REFONTE§7.8) — une
             notice, un manuel, des notes de version — rejoignent la liste des
             ressources plutôt que d'ouvrir une carte chacun : trois cartes
             au-dessus d'une carte « Ressources » annonçant « aucun fichier
             annexe » disaient le contraire de la vérité. -->
        <ResourcesBlock
          modId={id}
          extras={attachedDocs.map((doc) => ({ id: doc.id, source: "other" as const, label: doc.name }))}
          onerror={(m) => (actionError = m)}
        />
        {#if !isCar}
          <MediaBackgrounds
            modId={id}
            layoutId={d.track?.layouts[previewLayout]?.id ?? null}
            onerror={(m) => (actionError = m)}
          />
        {/if}
      </div>
    {:else}
      <!-- Installation : d'où vient ce mod, ce que l'app en a fait, et ce qu'il
           a posé dans le jeu. Les étiquettes y sont aussi (REFONTE§7.5) : elles sont la
           matière première d'où la catégorie est dérivée, et on les ouvre au
           moment précis où la dérivation s'est trompée — c'est-à-dire en même
           temps que l'origine et les décisions d'import. -->
      <div class="tab-body install">
        <div class="col">
          <AttachedBlock rows={attached} onopen={(a) => void openAttachedFiche(a)} />
          <ExtrasBlock modId={id} />
          <DecisionsBlock modId={id} />
        </div>
        <div class="col">
          <ProvenanceBlock
            detail={d}
            {siblings}
            busy={packBusy}
            onfilterbypack={filterByPack}
            onopenpack={openPack}
            onopensibling={openSibling}
            onuninstallpack={uninstallPack}
          />
          <HistoryBlock detail={d} {busy} onactivateversion={(vid) => activate(vid)} ondeleteversion={deleteVersion} />
          <LayersBlock
            modId={id}
            onchanged={() => {
              // Le contenu déployé vient de changer sous les mêmes chemins :
              // relire ne suffit pas, il faut aussi le dire (voir
              // `contentRevision`).
              contentRevision += 1;
              void refreshEntity();
            }}
            onerror={(m) => (actionError = m)}
            hostName={d.display_name}
            onopen={(layer, siblings) => (openLayer = { layer, siblings })}
          />
          <TagsBlock detail={d} onaddtag={addManual} onremovetag={removeManual} />
          {#if d.csp_features.length}
            <!-- Les extensions CSP ont quitté les étiquettes (REFONTE§7.5) : elles
                 décrivent l'installation, pas le contenu. Une ligne grise
                 suffit — on ne les compose pas, on les constate. -->
            <section class="blk">
              <header class="blk-h">
                <span class="blk-t">{t("columns.csp")}</span>
                <span class="blk-n">{d.csp_features.length}</span>
              </header>
              <div class="blk-b csp-row">{#each d.csp_features as f}<span class="csp">{f}</span>{/each}</div>
            </section>
          {/if}
        </div>
      </div>
    {/if}
  {/if}
</div>


<style>
  .page {
    margin: -28px -32px;
    min-height: 100%;
    background: var(--card);
    /* Conteneur de référence des requêtes ci-dessous : c'est cette largeur-là,
       celle qui reste à la fiche, qui décide de la mise en colonnes. Also
       the container of `DescriptionCard`'s reading widths. */
    container: detail / inline-size;
  }
  .empty {
    color: var(--muted);
    text-align: center;
    padding: 80px 0;
  }
  /* Onglets : `Tabs.svelte` (variante `flush`), partagé avec Réglages,
     Add-ons et Règles de tags — plus de style local ici. */
  /* Fiche posée par-dessus celle du mod : même respiration que le corps d'un
     onglet, puisqu'elle en occupe la place. */
  .sheet-wrap {
    padding: 18px;
  }
  .tab-body {
    padding: 18px;
  }
  /* Médias : quatre blocs l'un sous l'autre, pleine largeur. Une grille de
     colonnes y mettrait côte à côte une galerie de vignettes et une liste de
     replays, qui n'ont ni la même largeur utile ni le même rythme. */
  .tab-body.stack {
    display: flex;
    flex-direction: column;
    gap: 14px;
  }
  /* Installation : deux colonnes. À gauche ce que le mod a posé (souvent long
     — 34 dossiers pour une seule voiture), à droite d'où il vient et ce que
     l'app en a fait. */
  .tab-body.install {
    display: grid;
    grid-template-columns: 1.2fr 1fr;
    gap: 14px;
    align-items: start;
  }
  .tab-body.install .col {
    display: flex;
    flex-direction: column;
    gap: 14px;
    min-width: 0;
  }
  .errbox {
    margin: 10px 18px 0;
  }
  .export-ok {
    margin: 10px 18px 0;
    padding: 8px 10px;
    background: var(--green-dim);
    border: 1px solid var(--green-border);
    color: var(--green);
    font-size: 11.5px;
    line-height: 1.5;
  }
  .export-warn {
    list-style: none;
    margin-top: 6px;
    color: var(--yellow);
    font-size: 11px;
  }

  .row {
    display: grid;
    gap: 1px;
    /* Fond de page (`.page`, plus bas), pas une couleur de carte : c'est ce
       qui se voit dans l'interligne de 1px entre héros et panneau de
       données, et sous le héros lui-même quand celui-ci (16:9, jamais
       étiré — voir `.hero` dans DetailHero) est plus court que sa ligne. `--line`
       y ressortait comme un gris clair qui ne se voyait nulle part ailleurs
       (bug réel signalé). */
    background: var(--card);
  }
  .row.top {
    grid-template-columns: 1.4fr 1fr;
    /* **La rangée se referme sur son contenu.** Par défaut une colonne de
       grille s'étire à la hauteur de la plus haute, et le panneau de données —
       qui porte un fond — courait donc jusqu'en bas de la page alors qu'il
       n'avait de contenu que sur un tiers. Le vide se voyait surtout sur un
       circuit, dont la colonne est la plus maigre. Avec `start`, chaque colonne
       s'arrête où son contenu s'arrête, et le cas inverse — une colonne de
       droite plus haute que l'aperçu, un circuit à vingt habillages — se règle
       du même coup : elle s'allonge, l'aperçu reste aligné en haut, et rien ne
       casse la rangée. */
    align-items: start;
  }
  /* Une seule colonne quand la place manque : l'aperçu, puis le panneau.
     **Requête de conteneur et non de média** : la fiche ne voit pas la fenêtre
     mais ce qui lui reste une fois le rail et la colonne de session pris — et
     le zoom d'interface (un `zoom` CSS sur `<html>`, §13) déplace le seuil
     d'une requête de média sans déplacer la largeur réellement disponible. */
  @container detail (max-width: 1100px) {
    .row.top,
    .row.top.track {
      grid-template-columns: 1fr;
    }
  }
  .row.track {
    grid-template-columns: 1fr 1fr;
  }

  /* Colonne principale : le héros garde son ratio et sa taille propres, le
     sélecteur et le bloc textuel s'empilent dessous. */
  .maincol {
    display: flex;
    flex-direction: column;
    gap: 1px;
    min-width: 0;
    /* `align-self` appartient désormais à la colonne, plus au héros : c'est
       elle qui est l'élément de grille. Sur le héros, dans une colonne flex,
       il ne voudrait plus dire « ne t'étire pas en hauteur » mais « ne t'étire
       pas en largeur » — et un héros de voiture, dont le média est en absolu,
       n'a pas de largeur propre : il se réduirait à rien. */
    align-self: start;
  }
  .data {
    background: var(--card);
    padding: 14px;
    min-width: 0;
  }
  /* La rangée se refermant sur son contenu, la marge basse de la dernière
     carte se lirait comme un reste de l'ancien fond qui courait jusqu'en bas.
     `:global` because the cards are now child components, whose root elements
     do not carry this component's scoping class. */
  .data > :global(:last-child) {
    margin-bottom: 0;
  }
  .csp-row {
    display: flex;
    flex-wrap: wrap;
    gap: 5px;
  }
  .csp {
    font-size: 10px;
    color: var(--green);
    border: 1px solid var(--green-border);
    padding: 2px 8px;
  }

  .col {
    background: var(--card);
    padding: 14px;
  }
</style>
