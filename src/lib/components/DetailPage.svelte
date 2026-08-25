<script lang="ts">
  // Page de détail pleine page (§6.3, maquette pitbox-fiche-B-revisee.html).
  // Riche pour les voitures (héros + specs natives + fiche technique + courbe +
  // description + skins + tags/versions/historique). Panneaux Son et Distance =
  // placeholders « à venir » (lots §12bis et §6.5). Réduite pour les circuits.
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
    type NativeSpecs,
    type LayoutItem,
  } from "$lib/library";
  import { listMediaScreenshots, listMediaReplays, listMediaBackgrounds } from "$lib/media";
  import { listModSkins, openNativeShowroom, type SkinItem } from "$lib/launch";
  import CarPreview3D from "./detail/CarPreview3D.svelte";
  import InlineEdit from "./InlineEdit.svelte";
  import Tabs from "./Tabs.svelte";
  import StateBadge from "./StateBadge.svelte";
  import { tick, untrack } from "svelte";
  import { focusGamepadElement, isGamepadDriving } from "$lib/gamepadNav";
  import {
    preview3dPrefs,
    resetPreview3dView,
    savePreview3dPrefs,
    setPreview3dEnabled,
  } from "$lib/preview3dPrefs.svelte";
  import {
    exportMod,
    deletePack,
    deleteBrokenMod,
    reinstallFromArchive,
    deleteModVersion,
    profilesUsingVersion,
    type ExportReport,
  } from "$lib/maintenance";
  import {
    listSubMods,
    activateSound,
    restoreSound,
    syncTrackSkins,
    listActiveTrackSkins,
    setTrackSkinActive,
    type SubModRow,
  } from "$lib/submods";
  import { open, confirm } from "@tauri-apps/plugin-dialog";
  import PowerCurve from "./PowerCurve.svelte";
  import ContextMenu from "./ContextMenu.svelte";
  import { nav, pickSession, requestSection } from "$lib/nav.svelte";
  import { libraryVersion } from "$lib/libraryVersion.svelte";
  import { getPreferredSkin, setPreferredSkin, getPreferredLayout, setPreferredLayout } from "$lib/preferred";
  import { getConfig } from "$lib/config";
  import { t } from "$lib/i18n/index.svelte";
  import LayersBlock from "./detail/LayersBlock.svelte";
  import ResourcesBlock from "./detail/ResourcesBlock.svelte";
  import DecisionsBlock from "./detail/DecisionsBlock.svelte";
  import ExtrasBlock from "./detail/ExtrasBlock.svelte";
  import HistoryBlock from "./detail/HistoryBlock.svelte";
  import ProvenanceBlock from "./detail/ProvenanceBlock.svelte";
  import TagsBlock from "./detail/TagsBlock.svelte";
  import MediaScreenshots from "./detail/MediaScreenshots.svelte";
  import MediaReplays from "./detail/MediaReplays.svelte";
  import MediaBackgrounds from "./detail/MediaBackgrounds.svelte";
  import IgnitionKey from "./detail/IgnitionKey.svelte";
  import {
    engineControls,
    engineRev,
    engineShowcase,
    engineState,
    setEngineRev,
    setEngineShowcase,
    stopEngine,
    toggleEngine,
  } from "$lib/enginePlayer.svelte";
  import Slider from "$lib/components/Slider.svelte";

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
  type DetailTab = "fiche" | "screenshots" | "replays" | "resources" | "extras" | "backgrounds";
  let activeTab = $state<DetailTab>("fiche");
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
  const activeSound = $derived(sounds.find((s) => s.is_active) ?? null);
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
  // Couches / extensions rattachées (§4.4).
  // Fichiers annexes du mod (§4.5.2, Bloc Ressources) — lus en direct sur disque.

  // Onglets de la fiche. Le décompte fait partie du libellé (`null` tant qu'il
  // n'est pas connu : afficher « (0) » avant de savoir est un mensonge qui
  // dure une seconde). « Backgrounds » n'existe que pour un circuit.
  const tabItems = $derived.by(() => {
    const count = (n: number | null) => (n !== null ? ` (${n})` : "");
    const items = [
      { id: "fiche", label: t("detail.tabFiche") },
      { id: "screenshots", label: t("detail.tabScreenshots") + count(screenshotsCount) },
      { id: "replays", label: t("detail.tabReplays") + count(replaysCount) },
      { id: "resources", label: t("detail.tabResources") + count(resourcesCount) },
      { id: "extras", label: t("detail.tabExtras") + count(extrasCount) },
    ];
    if (!isCar) items.push({ id: "backgrounds", label: t("detail.tabBackgrounds") + count(backgroundsCount) });
    return items;
  });

  // Image héros : voiture → skin sélectionné ; circuit → preview du layout
  // sélectionné ; sinon preview par défaut du mod.
  const heroImg = $derived.by(() => {
    if (isCar && skins[previewSkin]?.preview) return previewSrc(skins[previewSkin].preview);
    const lay = detail?.track?.layouts[previewLayout];
    if (!isCar && lay?.preview) return previewSrc(lay.preview);
    return previewSrc(detail?.preview ?? null);
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
      const s = await listModSkins(current);
      if (current === id) skins = s;
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

  // Aperçu 3D maison (KN5 → glTF → three.js, docs/SPEC-preview-3d-kn5.md) :
  // il **coexiste** avec le showroom natif ci-dessus, il ne le remplace pas.
  // Les deux ne rendent pas le même service : celui-ci est inline et
  // manipulable dans la fiche, l'autre donne le rendu fidèle du jeu.
  // La bascule et les réglages de cadrage vivent dans `preview3dPrefs` : le
  // même réglage se change aussi depuis l'écran Réglages, et les deux doivent
  // rester d'accord sans qu'aucun des deux écrans n'ait à être remonté.
  const preview3d = $derived(preview3dPrefs().enabled);
  /** Panneau de réglages posé sur l'aperçu. Ouvert, il garde la barre d'outils
   * visible même quand la souris s'en va — sinon régler un curseur la ferait
   * disparaître sous les doigts. */
  let preview3dPanel = $state(false);

  /** Raccourci vers Réglages → Aperçu, qui porte l'aperçu 3D et les treize
   * réglages. Passe par `nav.settingsTab` : l'onglet actif est un état interne
   * de `Settings.svelte`, et le lui demander avant de naviguer évite de sortir
   * cet état de son composant pour un seul appelant. */
  function openPreviewSettings() {
    nav.settingsTab = "preview";
    nav.section = "settings";
  }

  function togglePreview3d() {
    setPreview3dEnabled(!preview3d);
    // Enregistré sur-le-champ, contrairement aux curseurs de l'écran Réglages :
    // c'est un interrupteur, pas un formulaire. Personne ne s'attend à devoir
    // aller valider ailleurs pour qu'une bascule d'un clic tienne.
    void savePreview3dPrefs().catch((e) => console.error("save_ui_prefs", e));
  }
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
    activeTab = "fiche";
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
      listModSkins(current)
        .then((s) => {
          if (current !== id) return;
          skins = s;
          const pi = s.findIndex((x) => x.id === savedSkin?.id);
          previewSkin = pi >= 0 ? pi : 0;
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

  // Son = bascule exclusive (§12bis.2) : un seul actif, original restaurable.
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
  // `null` tant que le chemin natif ne joue pas, ou quand l'événement joué
  // n'expose aucun paramètre de régime reconnaissable — il s'entend quand même,
  // il ne se règle simplement pas (§2.4).
  const revControls = $derived.by(() => {
    const c = engineControls();
    return c && c.revParam ? c : null;
  });

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

  // Sélectionner un skin (§8.6/§12bis.2) : mémorisé par voiture ET poussé dans
  // le duo de session (visible dans le menu). Remplace l'ancienne « étoile ».
  function selectSkin(i: number) {
    previewSkin = i;
    if (!detail) return;
    const sk = skins[i];
    if (sk) setPreferredSkin(detail.id_interne, sk);
    const meta = [detail.brand, sk ? `skin: ${sk.name}` : null].filter(Boolean).join(" · ");
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
  /** Si le layout mémorisé comme choix de session (§8.6) pour cette entité a
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
    const meta = [l?.name, detail.author].filter(Boolean).join(" · ");
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

  /** Cases vides à ajouter en fin de grille (§ correctif damier) : `.skins`
   * est une vraie grille CSS à colonnes fixes, donc la dernière ligne
   * incomplète laisse des cellules sans le moindre élément dedans — rien n'y
   * peint la couleur de carte (`--panel2`), c'est le fond de la grille
   * (`--card`, la couleur DERRIÈRE les cartes) qui s'y voit à la place (bug
   * réel signalé). Une case fantôme, muette pour tout le monde (souris,
   * clavier, lecteur d'écran), comble le trou avec la bonne couleur plutôt
   * que de la laisser transparente. */
  const SKINS_GRID_COLUMNS = 3;
  function gridFillerCount(n: number): number {
    return (SKINS_GRID_COLUMNS - (n % SKINS_GRID_COLUMNS)) % SKINS_GRID_COLUMNS;
  }

  function decodeDescription(html: string): string {
    return html
      .replace(/<\/?br\s*\/?>/gi, "\n")
      .replace(/<[^>]+>/g, "")
      .replace(/&#(\d+);/g, (_, n) => String.fromCharCode(+n))
      .replace(/&amp;/g, "&")
      .replace(/&lt;/g, "<")
      .replace(/&gt;/g, ">")
      .replace(/&quot;/g, '"')
      .replace(/&#39;|&apos;/g, "'")
      .trim();
  }

  function initials(brand: string | null, id: string): string {
    const src = (brand ?? id).replace(/[^a-zA-Z]/g, "");
    return (src.slice(0, 2) || "??").toUpperCase();
  }

  // Bandeau de specs natives en surimpression du héros (§6.3).
  function heroSpecs(s: NativeSpecs | null): string {
    if (!s) return "";
    return [s.bhp, s.torque, s.weight, s.topspeed].filter((x): x is string => !!x).join(" · ");
  }

  const DASH = "—";

  function posLabel(pos: string): string {
    if (pos === "FRONT") return t("detail.posFront");
    if (pos === "MID") return t("detail.posMid");
    if (pos === "REAR") return t("detail.posRear");
    return pos;
  }

  // Fiche technique (champs structurés) — abréviations façon maquette.
  function ficheRows(d: ModDetail): [string, string][] {
    const engine = [d.engine_config, d.engine_pos ? posLabel(d.engine_pos) : null]
      .filter(Boolean)
      .join(" · ");
    return [
      [t("detail.specEngine"), engine || DASH],
      [t("detail.specAspiration"), d.aspiration ?? DASH],
      [t("detail.specDrivetrain"), d.drivetrain ?? DASH],
      [t("detail.specGearbox"), d.gearbox ?? DASH],
      [t("detail.specCountry"), d.country ?? DASH],
      [t("detail.specPowerWeight"), d.specs?.pwratio ?? DASH],
    ];
  }

  // Menu ⋮ (§6.3, revue de la fiche) : regroupe les actions autrefois alignées
  // en rangée dans l'en-tête, peu utilisées au regard de la place qu'elles
  // prenaient une fois les onglets ajoutés. Cœur favori et badge « Contenu de
  // base » restent hors du menu (visibles en permanence, pas des actions).
  let menuPos = $state<{ x: number; y: number } | null>(null);
  function openActionsMenu(e: MouseEvent) {
    // Sans ça, ce même clic bulle jusqu'à `document` juste après le montage
    // de `ContextMenu` (son propre listener `click` de fermeture, voir
    // ContextMenu.svelte) et referme le menu dans la foulée — il s'ouvrait et
    // se refermait dans le même geste, invisible à l'œil (bug réel : hover
    // fonctionnait, le clic ne semblait « rien faire »).
    e.stopPropagation();
    const rect = (e.currentTarget as HTMLElement).getBoundingClientRect();
    menuPos = { x: rect.left, y: rect.bottom + 4 };
  }
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

<!-- Carte « Description », identique pour une voiture et pour un circuit :
     seule la source du texte change. Rendue MÊME VIDE, sans quoi un mod sans
     description n'offrirait aucun endroit où en écrire une (§5bis.3). -->
{#snippet descriptionCard(text: string | null, overridden: boolean)}
  <section class="blk">
    <header class="blk-h">
      <span class="blk-t">{t("common.description")}</span>
      <InlineEdit
        value={text}
        original={overridden ? null : text}
        {overridden}
        multiline
        label={t("detail.editDescriptionLabel")}
        placeholder={t("detail.editDescriptionPlaceholder")}
        onsave={(v) => saveOverride("description_user", v)}
      />
    </header>
    <div class="blk-b desc-body" class:empty-desc={!text}>
      {text ? decodeDescription(text) : t("detail.noDescription")}
    </div>
  </section>
{/snippet}

<div class="page">
  {#if !detail}
    <div class="empty">{t("common.loading")}</div>
  {:else}
    {@const d = detail}
    <header class="head">
      <button class="back" type="button" onclick={onclose} title={t("detail.backTooltip")}>←</button>
      {#if isCar && d.badge}
        <img class="escu badge-img" src={previewSrc(d.badge)} alt={d.brand ?? ""} />
      {:else}
        <span class="escu">{initials(d.brand, d.id_interne)}</span>
      {/if}
      <div class="title">
        <div class="t-name">
          <span class="t-name-txt">{d.display_name ?? d.id_interne}</span>
          <!-- Renommer (§5bis.3). Le repère montré pendant l'édition est le nom
               du FICHIER : `d.display_name` porte déjà la surcharge quand il y
               en a une, il ne dirait donc pas à quoi on reviendrait. -->
          <InlineEdit
            value={d.display_name}
            original={d.display_name_user ? d.display_name_file : null}
            overridden={!!d.display_name_user}
            label={t("detail.renameLabel")}
            onsave={(v) => saveOverride("display_name_user", v)}
          />
        </div>
        <div class="t-meta mono">
          {d.brand ?? ""}{d.year ? ` · ${d.year}` : ""}
          {#if d.category}· <span class="cat">{d.category}</span>{/if}
          {#if d.car_class}· {d.car_class.toUpperCase()}{/if}
        </div>
      </div>
      <div class="actions">
        <button class="fav" class:on={d.is_favorite} type="button" onclick={toggleFav} title={t("common.favorite")}>
          {d.is_favorite ? "♥" : "♡"}
        </button>
        <button class="kebab" type="button" onclick={openActionsMenu} title={t("detail.moreActions")}>
          <span class="kebab-dot"></span><span class="kebab-dot"></span><span class="kebab-dot"></span>
        </button>
      </div>
    </header>
    {#if menuPos}
      <ContextMenu x={menuPos.x} y={menuPos.y} items={menuItems} onclose={() => (menuPos = null)} />
    {/if}

    <!-- État du mod à droite de la bande d'onglets : c'est la première chose
         qu'on vient vérifier sur une fiche, et elle était introuvable sans
         ouvrir le menu ⋮ (dont le libellé Activer/Désactiver était le seul
         indice). Même pastille que la colonne « État » du tableau. -->
    <Tabs flush tabs={tabItems} active={activeTab} onselect={(v) => (activeTab = v as DetailTab)}>
      {#snippet trailing()}
        <StateBadge active={d.active} stock={d.is_stock} unmanaged={d.is_unmanaged} />
      {/snippet}
    </Tabs>

    {#if actionError}<div class="action-err">{actionError}</div>{/if}
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

    {#if activeTab === "fiche"}
    <!-- RANGÉE HAUTE : héros + panneau données -->
    <div class="row top" class:track={!isCar}>
      <div class="hero">
        <!-- Photo et aperçu 3D partagent le même cadre (§ correctif marge) :
             la photo est un enfant normal, `CarPreview3D` se pose en absolu
             `inset:0` — sans ce conteneur commun, chacun résolvait sa marge
             contre un ancêtre différent (`.hero` avec son padding pour l'un,
             `.hero` sans aucun pour l'autre), d'où le décalage constaté entre
             les deux vues. -->
        <div class="hero-inner">
          {#if heroImg}
            <img src={heroImg} alt={d.display_name ?? d.id_interne} />
          {:else}
            <div class="hero-icon">{isCar ? "🚗" : "🏁"}</div>
          {/if}
          {#if isCar && preview3d}
            <CarPreview3D
              carId={d.id_interne}
              skinId={skins[previewSkin]?.id ?? null}
              fallbackSrc={heroImg}
            />
          {/if}
        </div>
        {#if isCar}
          <!-- Commandes de l'aperçu : révélées au survol de la zone héros, pour
               qu'elles ne mangent pas l'image le reste du temps. Le focus
               clavier les révèle aussi (`:focus-within`), sans quoi elles
               seraient inatteignables autrement qu'à la souris. -->
          <div class="hero-tools" class:open={preview3dPanel}>
            <button
              class="hero-btn"
              type="button"
              onclick={togglePreview3d}
              title={preview3d ? t("detail.preview3dShowPhoto") : t("detail.preview3dShow3d")}
              aria-label={preview3d ? t("detail.preview3dShowPhoto") : t("detail.preview3dShow3d")}
            >
              {#if preview3d}
                <!-- Retour à la photo : un cadre et sa montagne. -->
                <svg viewBox="0 0 16 16" aria-hidden="true">
                  <rect x="1.5" y="3.5" width="13" height="9" rx="1" />
                  <path d="M2.5 11 L6 7.5 L8.5 10 L10.5 8.5 L13.5 11.5" fill="none" />
                  <circle cx="5.5" cy="6" r="1" />
                </svg>
              {:else}
                <!-- Passage en 3D : un volume en perspective. -->
                <svg viewBox="0 0 16 16" aria-hidden="true">
                  <path d="M8 1.8 L14 5 V11 L8 14.2 L2 11 V5 Z" fill="none" />
                  <path d="M2 5 L8 8.2 L14 5" fill="none" />
                  <path d="M8 8.2 V14.2" fill="none" />
                </svg>
              {/if}
            </button>
            {#if preview3d}
              <button
                class="hero-btn"
                type="button"
                onclick={resetPreview3dView}
                title={t("detail.preview3dReplace")}
                aria-label={t("detail.preview3dReplace")}
              >
                <!-- Replacer et relancer : une flèche qui reboucle. -->
                <svg viewBox="0 0 16 16" aria-hidden="true">
                  <path d="M13.2 8 A5.2 5.2 0 1 1 11.4 4.1" fill="none" />
                  <path d="M11.9 1.2 V4.5 H8.6" fill="none" />
                </svg>
              </button>
              <button
                class="hero-btn"
                class:on={preview3dPanel}
                type="button"
                onclick={() => (preview3dPanel = !preview3dPanel)}
                title={t("detail.preview3dSettings")}
                aria-label={t("detail.preview3dSettings")}
                aria-expanded={preview3dPanel}
              >
                <!-- Réglages : deux curseurs. -->
                <svg viewBox="0 0 16 16" aria-hidden="true">
                  <path d="M2 5.5 H14" fill="none" />
                  <path d="M2 10.5 H14" fill="none" />
                  <circle cx="6" cy="5.5" r="1.8" />
                  <circle cx="10.5" cy="10.5" r="1.8" />
                </svg>
              </button>
            {/if}
          </div>
          {#if preview3d && preview3dPanel}
            <!-- Les curseurs vivaient ici, en version compacte. Ils sont
                 partis dans Réglages → Aperçu, qui porte désormais son
                 propre aperçu 3D : on y règle en voyant le résultat, sur les
                 treize réglages et non sur les cinq qui tenaient dans ce
                 panneau. Reste le raccourci. -->
            <div class="hero-panel">
              <p class="hero-panel-t">{t("detail.preview3dSettingsMoved")}</p>
              <button class="btn" type="button" onclick={openPreviewSettings}>
                {t("detail.preview3dSettingsOpen")}
              </button>
            </div>
          {/if}
        {/if}
        {#if showroomBusy}
          <!-- Lancement d'acShowroom : pastille discrète le temps que le
               process démarre, il s'affichera ensuite par-dessus l'app. -->
          <div class="hero-loading" title={t("detail.showroomLoading")}>
            <span class="spinner"></span>
          </div>
        {/if}
        {#if !isCar}
          {@const ol = previewSrc(d.track?.layouts[previewLayout]?.outline ?? null)}
          {#if ol}<img class="hero-outline" src={ol} alt="" />{/if}
        {/if}
        {#if isCar}
          {@const hs = heroSpecs(d.specs)}
          {#if hs}
            <div class="hero-specs">
              <div class="mono hs-line">{hs}</div>
              <div class="mono hs-label">{t("detail.specNative")}</div>
            </div>
          {/if}
        {/if}
      </div>

      <div class="data">
        {#if isCar}
          {@const hasCurve = !!d.specs && d.specs.power_curve.length > 1}
          <div class="tech-curve" class:with-curve={hasCurve}>
            <section class="blk fiche">
              <header class="blk-h"><span class="blk-t">{t("detail.techSheet")}</span></header>
              <div class="specgrid">
                {#each ficheRows(d) as [k, v]}
                  <div><div class="k lbl-key">{k}</div><div class="v">{v}</div></div>
                {/each}
              </div>
            </section>
            {#if hasCurve && d.specs}
              <section class="blk curve-col">
                <header class="blk-h">
                  <span class="blk-t">{t("detail.curve")}</span>
                  <span class="blk-n"><span class="lg-pow">— bhp</span> <span class="lg-tor">— Nm</span></span>
                </header>
                <div class="blk-b curve-box">
                  <PowerCurve power={d.specs.power_curve} torque={d.specs.torque_curve} />
                </div>
              </section>
            {/if}
          </div>

          {@render descriptionCard(d.specs?.description ?? null, !!d.description_user)}
        {:else}
          {@const lay = d.track?.layouts[previewLayout]}
          <section class="blk">
            <header class="blk-h"><span class="blk-t">{t("detail.trackInfo")}</span></header>
            <div class="specgrid" style="grid-template-columns:1fr 1fr;">
              <div><div class="k lbl-key">{t("detail.layoutLabel")}</div><div class="v">{lay?.name ?? t("detail.defaultLayout")}</div></div>
              <div><div class="k lbl-key">{t("detail.lengthLabel")}</div><div class="v">{lay?.length ?? "—"}</div></div>
            </div>
          </section>
          {#if d.csp_features.length}
            <section class="blk">
              <header class="blk-h">
                <span class="blk-t">{t("columns.csp")}</span>
                <span class="blk-n">{d.csp_features.length}</span>
              </header>
              <div class="blk-b csp-row">{#each d.csp_features as f}<span class="csp">{f}</span>{/each}</div>
            </section>
          {/if}
          {@render descriptionCard(d.track?.description ?? null, !!d.description_user)}
        {/if}
      </div>
    </div>

    <!-- RANGÉE BASSE -->
    <div class="row bottom" class:track={!isCar}>
      {#if isCar}
        <!-- Skins : le skin sélectionné devient le skin de session (§8.6), mémorisé -->
        <div class="col">
          <section class="blk">
            <header class="blk-h">
              <span class="blk-t">{t("detail.skinsLabel")}</span>
              <span class="blk-n">{skins.length}</span>
            </header>
          {#if skins.length}
            <div class="skins">
              {#each skins as sk, i (sk.id)}
                {@const sp = previewSrc(sk.preview)}
                {@const lv = previewSrc(sk.livery)}
                <button
                  class="skin"
                  class:preview={i === previewSkin}
                  onclick={() => selectSkin(i)}
                  title={t("detail.chooseSkinTooltip")}
                >
                  <div class="skin-img">
                    {#if sp}<img src={sp} alt={sk.name} loading="lazy" />{:else}<span class="skin-noimg">▦</span>{/if}
                    <!-- `livery.png` (§8.6) : couleurs/motif du skin seul, en
                         complément de la photo de la voiture — jamais sur la
                         grande image du skin sélectionné (heroImg), juste ici
                         dans la grille de choix. -->
                    {#if lv}<img class="skin-livery" src={lv} alt="" loading="lazy" />{/if}
                    {#if i === previewSkin}<span class="skin-apercu mono">{t("library.sessionBadge")}</span>{/if}
                  </div>
                  <div class="skin-b">
                    <span class="skin-name">{sk.name}</span>
                  </div>
                </button>
              {/each}
              {#each Array.from({ length: gridFillerCount(skins.length) }) as _}
                <div class="skin-filler" aria-hidden="true"></div>
              {/each}
            </div>
          {:else}
            <div class="blk-b muted small">{t("detail.noSkins")}</div>
          {/if}
          </section>
        </div>

        <!-- Distance + Son : placeholders « à venir » désactivés -->
        <div class="col">
          <section class="blk">
            <header class="blk-h"><span class="blk-t">{t("detail.distanceLabel")}</span></header>
            <div class="blk-b">
            <div class="dist">
              <span class="dist-ic">🛣</span>
              <span class="dist-km mono">{d.distance_km != null ? `${d.distance_km.toFixed(1)} km` : "—"}</span>
              <span class="dist-state mono" class:on={d.tried}>{d.tried ? t("detail.triedYes") : t("detail.triedNo")}</span>
            </div>
            </div>
          </section>
          <section class="blk">
            <header class="blk-h"><span class="blk-t">{t("detail.engineSound")}</span></header>
            <div class="blk-b">
          <div class="sounds">
            <!-- Deux boutons par ligne, et c'est délibéré : le premier **active**
                 le son (il remplace les fichiers du jeu), la clé ne fait
                 qu'écouter. Un bouton imbriqué dans un autre serait invalide, et
                 surtout les deux gestes ne doivent pas se confondre. -->
            <div class="sound-row">
              <button class="sound" class:sel={!activeSound} type="button" onclick={() => pickSound(null)} disabled={soundBusy}>
                <span class="radio"></span>
                <span class="s-name">{t("detail.soundOrigin")}</span>
                <span class="s-tag mono">{t("library.baseBadge")}</span>
              </button>
              <IgnitionKey state={engineState(detail.id_interne, null)} onclick={() => listenSound(null)} />
            </div>
            {#each sounds as snd (snd.id)}
              <div class="sound-row">
                <button class="sound" class:sel={snd.is_active} type="button" onclick={() => pickSound(snd.id)} disabled={soundBusy}>
                  <span class="radio"></span>
                  <span class="s-name">{snd.name}</span>
                  <span class="s-tag mono">{t("detail.modTag")}</span>
                </button>
                <IgnitionKey state={engineState(detail.id_interne, snd.id)} onclick={() => listenSound(snd.id)} />
              </div>
            {/each}
          </div>
          <!-- Le curseur n'apparaît que quand le vrai moteur du jeu tourne : le
               repli joue un échantillon figé, qu'il n'y a rien à régler. Sa
               plage vient de la courbe de puissance de **cette** voiture, d'où
               un F1 qui monte à 19 500 et un utilitaire diesel à 5 000. -->
          {#if revControls}
            <div class="rev-row">
              <!-- Le curseur disparaît pendant la démonstration : les deux
                   pilotent le même paramètre, et un curseur qui ne suit pas ce
                   qu'on entend serait pire qu'absent. -->
              {#if !engineShowcase()}
                <Slider
                  compact
                  label={t("detail.soundRev")}
                  min={revControls.revFloor}
                  max={revControls.revCeiling}
                  step={50}
                  value={engineRev()}
                  display={t("detail.soundRevValue", { rpm: Math.round(engineRev()).toLocaleString() })}
                  oninput={setEngineRev}
                />
              {/if}
              <button
                class="blip"
                class:on={engineShowcase()}
                type="button"
                onclick={() => setEngineShowcase(!engineShowcase())}
              >
                {engineShowcase() ? t("detail.soundBlipStop") : t("detail.soundBlip")}
              </button>
            </div>
          {/if}
          <!-- L'exclusivité et l'absence de mod se lisent sur les boutons radio
               eux-mêmes : « Origine » seule et cochée dit tout. -->
            </div>
          </section>

          <TagsBlock detail={d} onaddtag={addManual} onremovetag={removeManual} />
        </div>

        <!-- Versions + Historique + Provenance -->
        <div class="col">
          <HistoryBlock
            detail={d}
            {busy}
            onactivateversion={(vid) => activate(vid)}
            ondeleteversion={deleteVersion}
          />
          <ProvenanceBlock detail={d} {siblings} busy={packBusy} onfilterbypack={filterByPack} onopenpack={openPack} onopensibling={openSibling} onuninstallpack={uninstallPack} />
          <LayersBlock modId={id} onchanged={refreshEntity} onerror={(m) => (actionError = m)} />
          <!-- Sous Provenance : c'est du même ordre — d'où vient ce mod et ce
               que l'app en a fait. Le bloc s'efface quand il n'y a rien à dire. -->
          <DecisionsBlock modId={id} />
        </div>
      {:else}
        <!-- Layouts (galerie illustrée par le tracé, comme les skins voiture) -->
        <div class="col">
          <section class="blk">
            <header class="blk-h">
              <span class="blk-t">{t("columns.layouts")}</span>
              <span class="blk-n">{d.track?.layouts.length ?? 0}</span>
            </header>
          {#if d.track && d.track.layouts.length}
            <div class="skins">
              {#each d.track.layouts as l, i (l.id || i)}
                {@const o = previewSrc(l.outline)}
                <button class="skin" class:preview={i === previewLayout} onclick={() => selectLayout(i)} title={t("detail.chooseLayoutTooltip")}>
                  <div class="skin-img layout-img">
                    {#if o}<img src={o} alt={l.name} loading="lazy" />{:else}<span class="skin-noimg">▦</span>{/if}
                    {#if i === previewLayout}<span class="skin-apercu mono">{t("library.sessionBadge")}</span>{/if}
                  </div>
                  <div class="skin-b"><span class="skin-name">{l.name}</span></div>
                </button>
              {/each}
              {#each Array.from({ length: gridFillerCount(d.track.layouts.length) }) as _}
                <div class="skin-filler" aria-hidden="true"></div>
              {/each}
            </div>
          {:else}
            <div class="blk-b muted small">{t("detail.singleLayout")}</div>
          {/if}
          </section>

          <!-- Skins de circuit (TRACK_SKIN) — activables individuellement, plusieurs
               à la fois (§8, pas de notion d'exclusivité côté CSP). -->
          <section class="blk">
            <header class="blk-h">
              <span class="blk-t">{t("detail.trackSkinsLabelPlain")}</span>
              {#if !trackSkinsLoading}<span class="blk-n">{trackSkins.length}</span>{/if}
            </header>
            <div class="blk-b">
          {#if trackSkinsLoading}
            <div class="muted small loading-inline"><span class="spinner-sm"></span>{t("common.loading")}</div>
          {:else if trackSkins.length}
            <ul class="tsk-list">
              {#each trackSkins as s (s.id)}
                {@const active = activeTrackSkins.includes(s.name)}
                <li class:inactive={!active}>
                  <label class="tog" title={active ? t("detail.trackSkinActiveOn") : t("detail.trackSkinActiveOff")}>
                    <input
                      type="checkbox"
                      checked={active}
                      disabled={trackSkinBusy}
                      onchange={() => toggleTrackSkin(s.name)}
                    />
                  </label>
                  <span class="tsk-name">{s.name}</span>
                  {#if s.source_archive}<span class="tsk-src mono">{s.source_archive}</span>{/if}
                </li>
              {/each}
            </ul>
            <div class="muted small">{t("detail.trackSkinsNote")}</div>
          {:else}
            <div class="muted small">{t("detail.noTrackSkins")}</div>
          {/if}
            </div>
          </section>
        </div>

        <!-- Distance + Tags (l'auteur vit désormais dans Source / origine) -->
        <div class="col">
          <section class="blk">
            <header class="blk-h"><span class="blk-t">{t("detail.distanceLabel")}</span></header>
            <div class="blk-b">
            <div class="dist">
              <span class="dist-ic">🛣</span>
              <span class="dist-km mono">{d.distance_km != null ? `${d.distance_km.toFixed(1)} km` : "—"}</span>
              <span class="dist-state mono" class:on={d.tried}>{d.tried ? t("detail.triedYes") : t("detail.triedNo")}</span>
            </div>
            </div>
          </section>
          <TagsBlock detail={d} onaddtag={addManual} onremovetag={removeManual} />
        </div>

        <!-- Versions + Historique + Provenance -->
        <div class="col">
          <HistoryBlock
            detail={d}
            {busy}
            onactivateversion={(vid) => activate(vid)}
            ondeleteversion={deleteVersion}
          />
          <ProvenanceBlock detail={d} {siblings} busy={packBusy} onfilterbypack={filterByPack} onopenpack={openPack} onopensibling={openSibling} onuninstallpack={uninstallPack} />
          <LayersBlock modId={id} onchanged={refreshEntity} onerror={(m) => (actionError = m)} />
          <!-- Sous Provenance : c'est du même ordre — d'où vient ce mod et ce
               que l'app en a fait. Le bloc s'efface quand il n'y a rien à dire. -->
          <DecisionsBlock modId={id} />
        </div>
      {/if}
    </div>
    {:else if activeTab === "screenshots"}
      <div class="tab-body">
        <MediaScreenshots modId={id} onerror={(m) => (actionError = m)} />
      </div>
    {:else if activeTab === "replays"}
      <div class="tab-body">
        <MediaReplays modId={id} onerror={(m) => (actionError = m)} />
      </div>
    {:else if activeTab === "resources"}
      <div class="tab-body">
        <ResourcesBlock modId={id} onerror={(m) => (actionError = m)} />
      </div>
    {:else if activeTab === "extras"}
      <div class="tab-body">
        <ExtrasBlock modId={id} />
      </div>
    {:else if activeTab === "backgrounds" && !isCar}
      <div class="tab-body">
        <MediaBackgrounds
          modId={id}
          layoutId={d.track?.layouts[previewLayout]?.id ?? null}
          onerror={(m) => (actionError = m)}
        />
      </div>
    {/if}
  {/if}
</div>


<style>
  .page {
    margin: -28px -32px;
    min-height: 100%;
    background: var(--card);
  }
  .empty {
    color: var(--muted);
    text-align: center;
    padding: 80px 0;
  }
  .head {
    display: flex;
    align-items: center;
    gap: 12px;
    padding: 12px 18px;
    border-bottom: 1px solid var(--line);
    background: var(--panel2);
  }
  .back {
    background: transparent;
    color: var(--muted);
    font-size: 18px;
    line-height: 1;
    padding: 2px 8px;
  }
  .back:hover {
    color: var(--txt);
  }
  .escu {
    width: 30px;
    height: 30px;
    background: var(--rosso);
    display: flex;
    align-items: center;
    justify-content: center;
    color: #fff;
    font-family: var(--mono);
    font-weight: 600;
    font-size: 11px;
    flex: none;
  }
  .escu.badge-img {
    background: var(--panel2);
    border: 1px solid var(--line);
    object-fit: contain;
    padding: 3px;
  }
  .title {
    min-width: 0;
    /* Prend la place disponible : en édition, le champ de saisie a besoin
       d'une largeur utile plutôt que de la seule largeur du nom. */
    flex: 1;
  }
  .t-name {
    font-size: 14px;
    font-weight: 600;
    /* 1.2 et pas 1.1 : à 1.1 la boîte de ligne est plus courte que la fonte,
       et les jambages descendants se font rogner en bas (le « g » de
       « Mugello » — retour utilisateur direct). */
    line-height: 1.2;
    /* Le crayon se pose au bout du nom, sur la même ligne de base. En édition,
       `InlineEdit` remplace le crayon par son champ : la colonne du titre
       s'élargit alors au lieu de pousser le nom hors cadre. */
    display: flex;
    align-items: center;
    gap: 6px;
    min-width: 0;
  }
  .t-name-txt {
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }
  /* Description absente : le texte de remplacement se distingue d'une vraie
     description, sinon « Aucune description. » se lit comme le contenu du mod. */
  .empty-desc {
    color: var(--muted);
    font-style: italic;
  }
  .t-meta {
    color: var(--muted);
    font-size: 10px;
    margin-top: 2px;
  }
  .t-meta .cat {
    color: var(--rosso-bright);
  }
  .actions {
    margin-left: auto;
    display: flex;
    align-items: center;
    gap: 8px;
  }
  .fav {
    background: transparent;
    color: var(--txt2);
    font-size: 18px;
    line-height: 1;
  }
  .fav:hover {
    color: var(--rosso-bright);
  }
  .fav.on {
    color: var(--rosso-bright);
  }
  /* Menu ⋮ : regroupe les actions autrefois en rangée dans l'en-tête (§6.3).
     Icône construite en CSS (3 carrés empilés) plutôt qu'un glyphe Unicode —
     le rendu du caractère « ⋮ » dépendait trop de la police (fin, peu
     lisible, cible de clic minuscule dans certains cas). */
  .kebab {
    background: transparent;
    display: flex;
    flex-direction: column;
    align-items: center;
    justify-content: center;
    gap: 3px;
    padding: 4px 7px;
  }
  .kebab-dot {
    width: 4px;
    height: 4px;
    background: var(--txt2);
    border-radius: 1px;
  }
  .kebab:hover .kebab-dot {
    background: var(--rosso-bright);
  }
  /* Onglets : `Tabs.svelte` (variante `flush`), partagé avec Réglages,
     Add-ons et Règles de tags — plus de style local ici. */
  .tab-body {
    padding: 18px;
  }
  .action-err {
    margin: 10px 18px 0;
    padding: 8px 10px;
    background: var(--rosso-dim);
    border: 1px solid var(--rosso-border);
    color: var(--rosso-bright);
    font-size: 11.5px;
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
       étiré — voir `.hero` plus bas) est plus court que sa ligne. `--line`
       y ressortait comme un gris clair qui ne se voyait nulle part ailleurs
       (bug réel signalé). */
    background: var(--card);
  }
  .row.top {
    grid-template-columns: 1.4fr 1fr;
  }
  .row.bottom {
    grid-template-columns: 1.3fr 1fr 1fr;
  }
  .row.track {
    grid-template-columns: 1fr 1fr;
  }
  .row.bottom.track {
    grid-template-columns: 1fr 1fr 1fr;
  }

  .hero {
    /* **Même carte que ses voisines.** Le panneau de données d'à côté est fait
       de `.blk` — encadré, fond `--panel2` — et le héros était un simple `div`
       au fond `--card` sans bordure : deux traitements différents pour deux
       blocs de la même rangée, ce qui se voyait (retour utilisateur). Il prend
       essayé la bordure et le fond `--panel2` d'un `.blk` : **les deux ont été
       retirés**. Le média n'occupe que l'intérieur du cadre, donc le fond plus
       sombre se voyait en bandes le long des bords, et la bordure ressortait
       comme un trait vertical au bord de l'aperçu — deux retours utilisateur
       successifs. Ce qui fait la parenté avec les cartes voisines, ici, c'est
       le retrait du média (`--hero-pad`, aligné sur `.blk-b`), pas un trait. */
    background: var(--card);
    /* Retrait du média dans le cadre. Les incrustations (commandes, caracté-
       ristiques, pastille de chargement) s'en déduisent : elles sont posées
       sur `.hero` et non dans `.hero-inner`, sinon ce dernier les **rogne** au
       bord du média — bug constaté, capture à l'appui : « Native spec » et les
       trois boutons coupés en deux. Elles ne sont pas le média, rien ne les
       oblige à partager son cadre. */
    --hero-pad: 14px;
    min-height: 300px;
    display: flex;
    align-items: center;
    justify-content: center;
    position: relative;
    overflow: hidden;
    /* Même respiration que les autres cartes (`.data`/`.col`) — l'image
       collée aux bords haut/gauche était un retour utilisateur direct.
       Pour une voiture, ce padding est repris par `.hero-inner` à la place
       (voir plus bas) : lui seul encadre aussi l'aperçu 3D. */
    padding: var(--hero-pad);
  }
  /* Photo et aperçu 3D (voiture uniquement) partagent ce cadre : la photo y
     est un enfant normal, l'aperçu 3D s'y pose en absolu `inset:0` — sans ce
     conteneur commun, chacun résolvait sa marge contre un ancêtre différent
     (`.hero` et son padding pour l'un, `.hero` sans aucun pour l'autre), d'où
     le décalage constaté entre les deux vues. Pour un circuit, simple
     passe-plat en flux normal : `.hero` garde son padding, rien ne change. */
  .hero-inner {
    position: relative;
    width: 100%;
    height: 100%;
    display: flex;
    align-items: center;
    justify-content: center;
  }
  /* Voiture : cadre à ratio fixe 16:9 (celui des previews AC), aligné en haut
     — `.hero` reste dimensionné par SON PROPRE ratio (`align-self: start`,
     ci-dessous), jamais étiré à la hauteur du panneau de données voisin.
     Un essai précédent avait fait l'inverse (`.hero` étiré, ratio seulement
     sur `.hero-inner`) pour que le fond de `.hero` couvre l'espace sous un
     aperçu court — mais rien ne garantissait plus que `.hero` reste assez
     haut pour SON PROPRE contenu : une fiche à description courte ramenait
     la ligne de grille sous la hauteur qu'exige le 16:9, et le bloc suivant
     (skins/distance) rognait l'aperçu par-dessus (bug réel signalé). Revenu
     à la version qui ne peut pas rogner : `.hero` a toujours exactement la
     taille de son média, l'espace qui reste dans sa ligne de grille montre le
     fond de `.row` (`--card`, identique au sien) plutôt que le sien propre. */
  .row.top:not(.track) .hero {
    --hero-pad: 16px;
    aspect-ratio: 16 / 9;
    min-height: 0;
    align-self: start;
    padding: 0;
  }
  .row.top:not(.track) .hero-inner {
    position: absolute;
    /* 16px comme le corps d'une carte (`.blk-b`) : c'est ce qui fait lire les
       deux blocs de la rangée comme une paire. */
    inset: var(--hero-pad);
  }
  .hero img {
    width: 100%;
    height: 100%;
    object-fit: cover;
  }
  /* Bascule photo / 3D, en bas à droite pour ne pas gêner le badge d'état de
     l'aperçu ni la pastille de lancement du showroom, tous deux en haut. */
  .hero-tools {
    position: absolute;
    /* **Sur le média**, à dix pixels de son bord : leur fond est un noir
       translucide, qui a besoin de l'image derrière lui pour se détacher.
       Posées sur la bande de fond du cadre, elles devenaient quasi invisibles
       — retour utilisateur. */
    right: calc(var(--hero-pad) + 10px);
    bottom: calc(var(--hero-pad) + 10px);
    display: flex;
    gap: 6px;
    z-index: 4;
    /* Effacées tant qu'on ne survole pas la zone : l'aperçu est là pour être
       regardé, pas pour montrer ses commandes. */
    opacity: 0;
    transition: opacity 0.15s ease;
  }
  .hero:hover .hero-tools,
  .hero:focus-within .hero-tools,
  .hero-tools.open {
    opacity: 1;
  }
  .hero-btn {
    width: 26px;
    height: 26px;
    display: flex;
    align-items: center;
    justify-content: center;
    padding: 0;
    /* Assez opaque pour se détacher d'une carrosserie claire comme d'un fond
       noir, et une bordure plus franche que `--line`, qui disparaissait sur
       les deux. */
    background: rgba(6, 6, 9, 0.82);
    border: 1px solid var(--muted2);
    color: var(--txt);
    cursor: pointer;
  }
  .hero-btn:hover {
    border-color: var(--rosso);
    color: var(--txt);
  }
  .hero-btn.on {
    border-color: var(--rosso);
    color: var(--rosso-bright);
  }
  .hero-btn svg {
    width: 14px;
    height: 14px;
    /* Tracé plutôt que remplissage, comme les boutons de la barre de titre :
       une seule couleur à piloter, et un rendu net à cette taille. */
    fill: none;
    stroke: currentColor;
    stroke-width: 1.3;
    stroke-linecap: round;
    stroke-linejoin: round;
  }
  .hero-panel {
    position: absolute;
    right: calc(var(--hero-pad) + 10px);
    bottom: calc(var(--hero-pad) + 44px);
    /* Bornée à la largeur disponible : le panneau porte maintenant une phrase
       et un bouton, et une largeur fixe le faisait déborder sur une fiche
       étroite. */
    width: min(240px, calc(100% - 2 * var(--hero-pad) - 20px));
    padding: 10px 12px 12px;
    background: rgba(8, 8, 12, 0.9);
    border: 1px solid var(--line);
    z-index: 4;
  }
  .hero-panel-t {
    margin: 0 0 10px;
    font-size: 11.5px;
    line-height: 1.5;
    color: var(--txt2);
  }
  /* Pastille de lancement de l'aperçu 3D : petite, en haut à droite, sans
     assombrir l'image — le showroom s'ouvrira par-dessus l'app. */
  .hero-loading {
    position: absolute;
    top: calc(var(--hero-pad) + 10px);
    right: calc(var(--hero-pad) + 10px);
    display: flex;
    align-items: center;
    justify-content: center;
    width: 26px;
    height: 26px;
    background: rgba(8, 8, 12, 0.6);
    border: 1px solid var(--line);
    z-index: 3;
  }
  .hero-loading .spinner {
    width: 15px;
    height: 15px;
    border: 2px solid var(--line);
    border-top-color: var(--rosso);
    border-radius: 50%;
    animation: hero-spin 0.8s linear infinite;
  }
  @keyframes hero-spin {
    to {
      transform: rotate(360deg);
    }
  }
  /* Tracé du layout superposé à la photo du circuit (§6.1). */
  .hero img.hero-outline {
    position: absolute;
    inset: 0;
    object-fit: contain;
    padding: 24px;
  }
  .hero-icon {
    font-size: 90px;
    opacity: 0.5;
  }
  .hero-specs {
    position: absolute;
    /* Mêmes retraits que les commandes d'en face. */
    left: calc(var(--hero-pad) + 12px);
    bottom: calc(var(--hero-pad) + 10px);
  }
  .hs-line {
    color: #e8e8ea;
    font-size: 13px;
  }
  .hs-label {
    color: var(--muted);
    font-size: 8px;
    margin-top: 3px;
  }
  .data {
    background: var(--card);
    padding: 14px;
  }
  /* Fiche technique + courbe carrée côte à côte (§5bis.1). */
  .tech-curve {
    display: flex;
    flex-wrap: wrap;
    gap: 12px;
    align-items: flex-start;
    margin-bottom: 12px;
  }
  .tech-curve .fiche {
    flex: 1 1 200px;
    min-width: 0;
    margin-bottom: 0;
  }
  .tech-curve.with-curve .specgrid {
    grid-template-columns: 1fr 1fr;
  }
  .curve-col {
    flex: 1 1 200px;
    max-width: 260px;
    min-width: 0;
  }
  /* Bandeau de tête d'encadré : c'est une rubrique (`.lbl`), simplement
     rendue en bandeau — d'où l'habillage local et la marge basse annulée. */
  .specgrid {
    display: grid;
    grid-template-columns: 1fr 1fr 1fr;
    background: var(--line);
    gap: 1px;
  }
  .specgrid > div {
    background: var(--panel2);
    padding: 7px 10px;
  }
  .specgrid .k {
    margin-bottom: 3px;
  }
  .specgrid .v {
    color: var(--txt2);
    font-size: 11px;
    font-family: var(--mono);
  }
  .lg-pow {
    color: var(--rosso-bright);
  }
  .lg-tor {
    color: var(--yellow);
  }
  .curve-box {
    border: 1px solid var(--line);
    padding: 8px;
    margin-bottom: 0;
  }
  .desc-body {
    border: 1px solid var(--line);
    border-top: none;
    background: var(--panel2);
    padding: 9px;
    color: var(--txt2);
    font-size: 11px;
    line-height: 1.55;
    white-space: pre-line;
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

  .skins {
    display: grid;
    grid-template-columns: repeat(3, 1fr);
    gap: 1px;
    /* Fond de page dans l'interligne de 1px entre vignettes, comme `.row`
       plus haut — pas une couleur de carte (bug réel signalé : `--line`,
       trop clair, y ressortait comme un gris qui ne se voyait nulle part
       ailleurs). */
    background: var(--card);
    border: 1px solid var(--line);
  }
  .skin {
    /* Même fond que les autres cartes de la fiche (`.blk`, global.css) : plus
       sombre que la page, c'est ce contraste qui détache la vignette. */
    background: var(--panel2);
    padding: 0;
    text-align: left;
    cursor: pointer;
    position: relative;
  }
  /* Case fantôme de fin de grille (§ correctif damier, `gridFillerCount`) :
     même fond que `.skin`, sans rien d'interactif. */
  .skin-filler {
    background: var(--panel2);
  }
  /* Cadre du choix de session en calque par-dessus la vignette : un `outline`
     inset était peint avant les descendants positionnés (.skin-img), donc
     masqué par le tracé/la preview qui remplit la cellule. */
  .skin.preview::after {
    content: "";
    position: absolute;
    inset: 0;
    border: 2px solid var(--rosso);
    pointer-events: none;
    z-index: 2;
  }
  .skin-img {
    /* Ratio des previews AC (~16:9) : la hauteur suit la largeur de la cellule,
       au lieu d'une hauteur fixe qui rognait la voiture. */
    aspect-ratio: 16 / 9;
    display: flex;
    align-items: center;
    justify-content: center;
    border-bottom: 1px solid var(--line);
    position: relative;
    overflow: hidden;
    background: var(--bg);
  }
  .skin-img img {
    width: 100%;
    height: 100%;
    object-fit: cover;
  }
  /* Tracé de layout : afficher la forme complète (pas de recadrage). */
  .layout-img img {
    object-fit: contain;
    padding: 4px;
  }
  .skin-noimg {
    color: var(--faint);
    font-size: 16px;
  }
  .skin-apercu {
    position: absolute;
    bottom: 3px;
    left: 3px;
    background: var(--rosso);
    color: #fff;
    font-size: 7px;
    padding: 0 3px;
  }
  /* `livery.png` (§8.6) : coin supérieur droit, libre (le badge session est
     en bas à gauche). Bordure pour rester lisible sur une preview claire.
     Sélecteur descendant obligatoire, et pas par style : `.skin-img img`
     (0,1,1) l'emporte sur `.skin-livery` (0,1,0) quel que soit l'ordre des
     règles, donc le médaillon héritait de `width/height: 100%` et recouvrait
     la photo de la voiture — soit exactement ce que le §8.6 interdit. */
  .skin-img img.skin-livery {
    position: absolute;
    top: 4px;
    right: 4px;
    width: 22px;
    height: 22px;
    object-fit: cover;
    border: 1px solid var(--line);
    background: var(--bg);
  }
  .skin-b {
    padding: 5px 7px;
    display: flex;
    align-items: center;
    gap: 5px;
  }
  .skin-name {
    font-size: 10px;
    white-space: nowrap;
    overflow: hidden;
    text-overflow: ellipsis;
    flex: 1;
  }

  .dist {
    display: flex;
    align-items: center;
    gap: 8px;
    padding: 7px 10px;
  }
  .dist-ic {
    font-size: 14px;
    opacity: 0.8;
  }
  .dist-km {
    font-size: 13px;
    font-weight: 600;
    color: var(--txt);
  }
  .dist-state {
    margin-left: auto;
    font-size: 8px;
    color: var(--muted);
  }
  .dist-state.on {
    color: var(--green);
  }

  .sounds {
    display: flex;
    flex-direction: column;
    gap: 5px;
  }
  .rev-row {
    display: flex;
    align-items: flex-end;
    gap: 12px;
    margin-top: 10px;
  }

  /* Le curseur prend la place restante ; le bouton garde la sienne. */
  .rev-row :global(.slider) {
    flex: 1;
    min-width: 0;
  }

  .blip {
    flex: 0 0 auto;
    margin-left: auto;
    padding: 6px 12px;
    border: 1px solid var(--rosso-border);
    border-radius: 4px;
    background: var(--rosso-dim);
    color: var(--txt);
    font-size: 11.5px;
    cursor: pointer;
  }

  .blip:hover {
    border-color: var(--rosso-bright);
  }

  .blip.on {
    background: var(--rosso-bright);
    border-color: var(--rosso-bright);
    color: #fff;
  }

  .sound-row {
    display: flex;
    align-items: center;
    gap: 6px;
  }
  .sound-row .sound {
    flex: 1;
    min-width: 0;
  }
  .sound {
    display: flex;
    align-items: center;
    gap: 8px;
    background: var(--panel2);
    border: 1px solid var(--line);
    padding: 7px 10px;
    text-align: left;
  }
  .sound.sel {
    border-color: var(--rosso-border);
    background: var(--rosso-dim);
  }
  .sound:disabled {
    opacity: 0.6;
  }
  .radio {
    width: 13px;
    height: 13px;
    border-radius: 50%;
    border: 1px solid var(--muted2);
    flex: none;
  }
  .sound.sel .radio {
    border-color: var(--rosso-bright);
    background: radial-gradient(var(--rosso-bright) 40%, transparent 45%);
  }
  .s-name {
    flex: 1;
    font-size: 11px;
    color: var(--txt2);
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }
  .s-tag {
    font-size: 7px;
    padding: 1px 5px;
    border: 1px solid var(--line);
    color: var(--muted);
  }

  .muted {
    color: var(--muted);
  }
  .small {
    font-size: 11px;
  }

  .tsk-list {
    list-style: none;
    display: flex;
    flex-direction: column;
    gap: 4px;
    margin-bottom: 6px;
  }
  .tsk-list li {
    display: flex;
    align-items: center;
    gap: 8px;
    border: 1px solid var(--line);
    background: var(--panel2);
    padding: 5px 9px;
  }
  .tsk-list li.inactive {
    opacity: 0.6;
  }
  .tsk-list .tog {
    flex: none;
    display: flex;
    align-items: center;
    cursor: pointer;
  }
  .tsk-name {
    flex: 1;
    font-size: 11px;
    color: var(--txt2);
  }
  .tsk-src {
    font-size: 9px;
    color: var(--muted2);
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
    max-width: 120px;
  }
  .loading-inline {
    display: flex;
    align-items: center;
    gap: 8px;
  }
  .spinner-sm {
    flex: none;
    width: 12px;
    height: 12px;
    border: 2px solid var(--line);
    border-top-color: var(--rosso);
    border-radius: 50%;
    animation: tsk-spin 0.8s linear infinite;
  }
  @keyframes tsk-spin {
    to {
      transform: rotate(360deg);
    }
  }

  /* Couches / extensions (§4.4) */

  /* Bloc Ressources (§4.5.2) : déplacé dans son propre onglet (§6.1) */

  /* Provenance / pack d'origine (§4.4) */
</style>
