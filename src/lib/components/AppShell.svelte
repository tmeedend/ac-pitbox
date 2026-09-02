<script lang="ts">
  import { onMount } from "svelte";
  import Settings from "./Settings.svelte";
  import About from "./About.svelte";
  import Library from "./Library.svelte";
  import RulesEditor from "./RulesEditor.svelte";
  import Profiles from "./Profiles.svelte";
  import Launch from "./Launch.svelte";
  import Maintenance from "./Maintenance.svelte";
  import Transversal from "./Transversal.svelte";
  import DriverScreen from "./driver/DriverScreen.svelte";
  import Apps from "./Apps.svelte";
  import OtherMods from "./OtherMods.svelte";
  import Import from "./Import.svelte";
  import ImportOverlay from "./ImportOverlay.svelte";
  import PendingDialog from "./PendingDialog.svelte";
  import ImportToasts from "./ImportToasts.svelte";
  import ToastStack from "./ToastStack.svelte";
  import ControllerToast from "./ControllerToast.svelte";
  import BulkToasts from "./BulkToasts.svelte";
  import TitleBar from "./TitleBar.svelte";
  import ControllerSetup from "./ControllerSetup.svelte";
  import ImageSelectDropdown from "./ImageSelectDropdown.svelte";

  import { driverFor, isEmpty, wearsFallback } from "$lib/driverOverride.svelte";
  import { wornOutfit } from "$lib/driverOutfits.svelte";
  import TrackSkinChecklistDropdown from "./TrackSkinChecklistDropdown.svelte";
  import { nav, requestSection, pickSession } from "$lib/nav.svelte";
  import { previewSrc, getModDetail, activateMod } from "$lib/library";
  import { confirm, message } from "@tauri-apps/plugin-dialog";
  import { errorText } from "$lib/errors";
  import { initGlobalDragDrop } from "$lib/importState.svelte";
  import { initBulkProgress } from "$lib/bulkState.svelte";
  import { openContentManager, listModSkins, type SkinItem } from "$lib/launch";
  import { setPreferredSkin, setPreferredLayout } from "$lib/preferred";
  import { syncTrackSkins, listTrackSkinOptions, setTrackSkinActive, type TrackSkinOption } from "$lib/submods";
  import { t, setLocale } from "$lib/i18n/index.svelte";
  import { setZoom } from "$lib/zoom.svelte";
  import { getConfig } from "$lib/config";
  import { LAUNCH_BUTTON_ATTR, startGamepadNav } from "$lib/gamepadNav";
  import { controllers, startControllerWatch } from "$lib/gamepadDevices.svelte";
  import { bigPictureState, exitBigPicture } from "$lib/bigpicture.svelte";
  import { musicEnterMenu, musicEnterGrid } from "$lib/music";
  import { libraryVersion } from "$lib/libraryVersion.svelte";

  // Barre latérale unifiée (maquette pitbox-biblio-session2.html) : bloc
  // SESSION (le duo sélectionné = point d'accès aux bibliothèques) puis
  // ADD-ONS et ATELIER en deux colonnes.
  type NavBtn = { id: string; labelKey: string; action?: boolean };
  const addons: NavBtn[] = [
    { id: "carskins", labelKey: "nav.carAddons" },
    { id: "trackskins", labelKey: "nav.trackAddons" },
    { id: "others", labelKey: "nav.others" },
    { id: "apps", labelKey: "nav.apps" },
  ];
  const atelier: NavBtn[] = [
    { id: "rules", labelKey: "nav.rules" },
    { id: "import", labelKey: "nav.import" },
    { id: "profiles", labelKey: "nav.profiles" },
    { id: "maintenance", labelKey: "nav.maintenance" },
    // Action directe, pas un écran : n'affecte jamais nav.section.
    { id: "opencm", labelKey: "nav.openCm", action: true },
    // Réglages partage la ligne d'Ouvrir CM ; « À propos » vit désormais
    // dans la barre de titre (icône ?), pas dans la navigation.
    { id: "settings", labelKey: "nav.settings" },
  ];

  async function handleAtelierClick(b: NavBtn) {
    if (b.action) {
      if (b.id === "opencm") {
        try {
          await openContentManager();
        } catch (e) {
          console.error(e);
        }
      }
      return;
    }
    await requestSection(b.id);
  }

  // Glisser-déposer disponible partout : un seul listener, monté ici à la
  // racine, plutôt que dans chaque écran susceptible de recevoir un drop.
  onMount(() => initGlobalDragDrop());

  // Progression des actions groupées (§6.3bis) : un seul écouteur, monté ici
  // comme le glisser-déposer — un lot lancé depuis la bibliothèque doit rester
  // visible même si on change d'écran pendant.
  onMount(() => initBulkProgress());

  // Navigation manette dans toute l'app (croix/stick = déplace le focus,
  // A/Croix = valide, B/Rond = ferme la fiche pleine page). Un seul scrutin
  // global, monté une fois ici.
  onMount(() => startGamepadNav());

  // Détection des périphériques et décision « lequel pilote l'interface »
  // (§7.4). Démarrage, branchement à chaud et première installation sont le
  // même événement — un périphérique visible sans décision enregistrée — donc
  // une seule surveillance, montée ici comme le scrutin ci-dessus.
  onMount(() => startControllerWatch());

  // Supprime le menu contextuel natif du navigateur (Actualiser/Enregistrer
  // sous/Imprimer…) partout dans l'app — une appli desktop n'en a pas besoin,
  // et il apparaîtrait sinon là où aucun menu contextuel maison n'est posé
  // (celui-ci, lui, s'affiche AVANT — donc gagne toujours). Un seul listener
  // global plutôt qu'un `preventDefault` à poser sur chaque écran.
  onMount(() => {
    const suppress = (e: MouseEvent) => e.preventDefault();
    document.addEventListener("contextmenu", suppress);
    return () => document.removeEventListener("contextmenu", suppress);
  });

  // Langue forcée par l'utilisateur (Réglages), sinon langue système (déjà
  // appliquée par défaut par le module i18n). Zoom d'interface, idem.
  onMount(async () => {
    const cfg = await getConfig();
    if (cfg.prefs.language) setLocale(cfg.prefs.language);
    setZoom(cfg.prefs.ui_zoom);
  });

  // Sortie du mode Big Picture au clavier — pas d'autre chrome de fenêtre
  // visible une fois en plein écran pour cliquer un bouton "retour" évident.
  onMount(() => {
    const onKeydown = (e: KeyboardEvent) => {
      if (e.key === "Escape" && bigPictureState.active) exitBigPicture();
    };
    document.addEventListener("keydown", onKeydown);
    return () => document.removeEventListener("keydown", onKeydown);
  });

  // Ambiance musicale suit l'écran affiché tant que Big Picture est actif
  // (§4 de la spec musique) : GRID sur l'écran de paramétrage de la session
  // ("race", l'équivalent Pit Box de la grille de départ), MENU partout
  // ailleurs.
  $effect(() => {
    if (!bigPictureState.active) return;
    if (nav.section === "race") musicEnterGrid();
    else musicEnterMenu();
  });

  const carPrev = $derived(previewSrc(nav.sessionCar?.preview ?? null));
  const trackPrev = $derived(previewSrc(nav.sessionTrack?.preview ?? null));
  const trackOutline = $derived(previewSrc(nav.sessionTrack?.outline ?? null));

  const isLibrary = $derived(nav.section === "cars" || nav.section === "tracks");
  // Écrans « pleine page » qui gèrent leur propre défilement interne plutôt
  // que de compter sur le padding + le scroll de `.content` (évite le hack
  // de marge négative — cause probable de l'espace perdu au-dessus du titre
  // « Réglages » signalé par l'utilisateur, cf. Launch.svelte).
  const noPad = $derived(isLibrary || nav.section === "race");

  // Double-clic sur le slot de session : ouvre directement la fiche détail de
  // l'entité choisie (skin, layout…) plutôt que la liste de la bibliothèque.
  async function openSessionDetail(section: "cars" | "tracks", id: string | null | undefined) {
    if (await requestSection(section) && id) nav.openMod = id;
  }

  // --- Sélecteurs rapides skin voiture / layout+skins circuit, directement
  // depuis le bloc SESSION (évite de passer par la fiche détail pour un
  // changement rapide). Mêmes actions que DetailPage.svelte (mémorise le
  // choix + met à jour le duo de session), réutilisées à l'identique.
  let carSkins = $state<SkinItem[]>([]);
  let carDetail = $state<Awaited<ReturnType<typeof getModDetail>>>(null);
  let trackDetail = $state<Awaited<ReturnType<typeof getModDetail>>>(null);
  let trackSkinOptions = $state<TrackSkinOption[]>([]);
  let trackSkinBusy = $state(false);

  $effect(() => {
    const carId = nav.sessionCar?.id ?? null;
    if (!carId) {
      carSkins = [];
      return;
    }
    listModSkins(carId).then((s) => {
      if (nav.sessionCar?.id === carId) carSkins = s;
    });
  });

  // État d'activation frais du duo de session (§ garde-fou lancement
  // ci-dessous) : jamais déduit de `nav.sessionCar`/`sessionTrack` eux-mêmes
  // (juste id/nom/preview pour l'affichage, persistés tels quels — une donnée
  // d'activation qui y serait figée resterait fausse dès que l'état change
  // ailleurs, ex. désactivé depuis la fiche détail sans repasser par ce
  // sélecteur). Repose sur le même effet-clé-sur-id que `trackDetail`
  // ci-dessous, qui écarte déjà les réponses obsolètes.
  $effect(() => {
    const carId = nav.sessionCar?.id ?? null;
    // Dépendance explicite : sans elle, désactiver la voiture de session
    // depuis sa fiche (ou en masse) ne rafraîchirait `carDetail` qu'au
    // prochain changement d'id — l'avertissement resterait faux jusqu'à ce
    // qu'on resélectionne la même voiture.
    libraryVersion();
    if (!carId) {
      carDetail = null;
      return;
    }
    getModDetail(carId).then((d) => {
      if (nav.sessionCar?.id === carId) carDetail = d;
    });
  });

  $effect(() => {
    const trackId = nav.sessionTrack?.id ?? null;
    libraryVersion();
    if (!trackId) {
      trackDetail = null;
      trackSkinOptions = [];
      return;
    }
    getModDetail(trackId).then((d) => {
      if (nav.sessionTrack?.id === trackId) trackDetail = d;
    });
    loadTrackSkinOptions(trackId);
  });

  // `null` tant que le détail n'est pas encore chargé (juste après une
  // sélection) : pas d'avertissement affiché dans ce court intervalle plutôt
  // que de risquer un faux positif pendant le chargement.
  const carInactive = $derived(nav.sessionCar != null && carDetail != null && !carDetail.active);

  // --- Point d'entrée de l'écran Pilote (SPEC-ecran-pilote §3.2) ---------
  //
  // Une ligne, pas trois menus : le choix a quitté cette colonne pour son
  // propre écran, parce que la hauteur y est la ressource rare et que
  // l'arrivée du corps y portait le nombre de listes à quatre (§D1). La ligne
  // porte le libellé, et un badge qui dit en un mot où en est le pilote.
  //
  // Proposé pour les voitures de rue seulement : sur une voiture de course le
  // pilote porte les couleurs de son écurie, donc celles du skin. La ligne
  // reste **visible et badgée**, jamais masquée — une option qui disparaît
  // sans un mot laisse chercher, et l'écran lui-même explique pourquoi.
  const carIsRace = $derived((carDetail?.car_class ?? "").toLowerCase() === "race");
  /** La tenue de **cette** voiture, cascade résolue : la sienne si elle en a
   * une, la tenue par défaut si l'option est active, la livrée sinon. */
  const driverPrefs = $derived(driverFor(nav.sessionCar?.id ?? null));

  /** Rien de choisi : la voiture et sa livrée décident de tout. */
  const driverUntouched = $derived(isEmpty(driverPrefs));

  /**
   * Ce que la ligne annonce.
   *
   * « Mon pilote » ne disait rien de ce qu'on porte. Trois cas, trois
   * réponses : le **nom de la tenue** quand on en a enregistré une et qu'on la
   * porte — c'est l'information la plus utile et c'est l'utilisateur qui l'a
   * écrite —, sinon la mention que rien n'a été touché, sinon qu'on a composé
   * quelque chose sans le nommer.
   */
  const driverLabel = $derived.by(() => {
    if (driverUntouched) return t("session.driverStock");
    const named = wornOutfit(driverPrefs)?.name;
    if (named) return named;
    // Une tenue héritée du défaut sans nom ne devrait pas exister — le défaut
    // *est* une tenue enregistrée — mais le dire plutôt que de mentir coûte
    // une ligne.
    return wearsFallback(nav.sessionCar?.id ?? null) ? t("session.driverFallback") : t("session.driverCustom");
  });

  /** Clé du badge, ou `null` (§3.2). « Modifié » a disparu de la liste : le
   * libellé le dit déjà, et un badge qui répète la ligne qu'il accompagne
   * n'est que du bruit. */
  const driverBadge = $derived.by(() => {
    if (carIsRace) return "disabled";
    if (driverPrefs.body) return "substituted";
    return null;
  });

  const trackInactive = $derived(nav.sessionTrack != null && trackDetail != null && !trackDetail.active);

  // Bouton rouge « Démarrer la session » : lance directement avec les
  // réglages courants (dernier preset du type de session), sans repasser par
  // l'écran Paramétrage — pose le drapeau consommé par Launch.svelte une fois
  // monté et prêt (mêmes valeurs que si l'écran avait été ouvert normalement).
  //
  // Garde-fou activation (§ bug réel signalé) : lancer une session avec une
  // voiture/un circuit sélectionné mais non activé (jamais junctionné dans
  // `content/`) fait planter Content Manager/AC, qui ne trouve pas le contenu.
  // On bloque, on demande confirmation, et on active avant de laisser
  // continuer — jamais d'activation silencieuse sans accord explicite.
  async function launchNow() {
    const toActivate: { id: string; name: string }[] = [];
    if (carInactive && nav.sessionCar) toActivate.push({ id: nav.sessionCar.id, name: nav.sessionCar.name });
    if (trackInactive && nav.sessionTrack) toActivate.push({ id: nav.sessionTrack.id, name: nav.sessionTrack.name });
    if (toActivate.length) {
      const ok = await confirm(t("session.inactivePrompt", { names: toActivate.map((m) => m.name).join(", ") }), {
        title: t("session.inactiveTitle"),
        kind: "warning",
      });
      if (!ok) return;
      try {
        for (const m of toActivate) await activateMod(m.id);
      } catch (e) {
        await message(errorText(e), { title: t("session.activateFailedTitle"), kind: "error" });
        return;
      }
      // Recharge tout de suite l'état frais : efface l'icône d'alerte sans
      // attendre le prochain changement de sélection.
      if (nav.sessionCar) carDetail = await getModDetail(nav.sessionCar.id);
      if (nav.sessionTrack) trackDetail = await getModDetail(nav.sessionTrack.id);
    }
    nav.autoLaunch = true;
    if (!(await requestSection("race"))) nav.autoLaunch = false;
  }

  async function loadTrackSkinOptions(trackId: string) {
    await syncTrackSkins(trackId);
    if (nav.sessionTrack?.id !== trackId) return;
    const opts = await listTrackSkinOptions(trackId);
    if (nav.sessionTrack?.id === trackId) trackSkinOptions = opts;
  }

  // `livery.png` (couleurs/motif du skin seul) plutôt que `preview` (photo de
  // la voiture entière, §8.6) : à 20px dans ce menu compact, la voiture
  // entière écrasée était illisible — repli sur `preview` si le skin n'a pas
  // de livery (convention pas garantie sur tous les skins).
  const carSkinOptions = $derived(
    carSkins.map((s) => ({ id: s.id, name: s.name, image: previewSrc(s.livery ?? s.preview) })),
  );
  // Le tracé (outline), pas la photo de fond : plus lisible en petite
  // miniature pour distinguer les layouts d'un même circuit d'un coup d'œil.
  const trackLayoutOptions = $derived(
    (trackDetail?.track?.layouts ?? []).map((l) => ({ id: l.id, name: l.name, image: previewSrc(l.outline) })),
  );
  const trackSkinChecklist = $derived(
    trackSkinOptions.map((o) => ({ name: o.name, image: previewSrc(o.image), active: o.active })),
  );

  function pickCarSkin(skinId: string) {
    const car = nav.sessionCar;
    const sk = carSkins.find((s) => s.id === skinId);
    if (!car || !sk) return;
    setPreferredSkin(car.id, sk);
    const base = car.meta.replace(/\s*·\s*skin:\s*[^·]+$/i, "").trim();
    pickSession("Car", {
      ...car,
      meta: [base, `skin: ${sk.name}`].filter(Boolean).join(" · "),
      preview: sk.preview ?? car.preview,
      skin: sk.id,
    });
  }

  function pickTrackLayout(layoutId: string) {
    const track = nav.sessionTrack;
    const d = trackDetail;
    const l = d?.track?.layouts.find((x) => x.id === layoutId);
    if (!track || !d || !l) return;
    setPreferredLayout(d.id_interne, l);
    pickSession("Track", {
      ...track,
      meta: [l.name, d.author].filter(Boolean).join(" · "),
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

{#if !bigPictureState.active}
  <TitleBar />
{/if}
<div class="frame" class:bigpicture={bigPictureState.active}>
  <div class="topbar"></div>
  <div class="shell">
    <!-- Zones parcourues par les gâchettes hautes de la manette (§7.4bis) :
         la barre latérale d'un côté, l'écran actif de l'autre. La
         bibliothèque redécoupe sa moitié en deux (liste et fiche) — les zones
         imbriquées les plus internes gagnent, voir `regions()`. -->
    <aside class="side" data-gp-region="sidebar">
      <div class="brand">
        <div class="logo"><span>PB</span></div>
        <div>
          <div class="brand-name">PIT BOX</div>
          <div class="brand-sub">AC MOD MANAGER</div>
        </div>
      </div>

      <!-- SESSION : duo sélectionné, point d'accès aux bibliothèques -->
      <div class="session">
        <div class="nsec">{t("nav.session")}</div>
        <div class="slot" class:on={nav.section === "cars"}>
          <button
            class="slot-main"
            type="button"
            onclick={() => requestSection("cars")}
            ondblclick={() => openSessionDetail("cars", nav.sessionCar?.id)}
            title={t("session.carTooltip")}
          >
            <div class="slot-img car">
              {#if carPrev}<img src={carPrev} alt="" />{:else}<span class="slot-ic">🚗</span>{/if}
              <span class="slot-tag">{t("session.carTag")}</span>
              <span class="slot-edit">{t("session.change")}</span>
            </div>
            <div class="slot-b">
              <div class="slot-name">
                {nav.sessionCar?.name ?? t("session.noCar")}
                {#if carInactive}<span class="slot-warn" title={t("session.inactiveTooltip")}>⚠</span>{/if}
              </div>
              <div class="slot-meta">{nav.sessionCar?.meta || t("session.clickToChoose")}</div>
            </div>
          </button>
          {#if nav.sessionCar}
            <div class="slot-pick">
              <!-- Le skin et la bascule pilote partagent une ligne : la
                   contrainte de cette colonne est la hauteur, et une bascule
                   décochée ne doit rien coûter (§4.6ter). -->
              <ImageSelectDropdown
                options={carSkinOptions}
                selectedId={nav.sessionCar.skin}
                placeholder={t("session.pickSkin")}
                emptyText={t("session.noSkinsAvailable")}
                onselect={pickCarSkin}
              />
              <button
                class="driver-line"
                class:on={nav.section === "driver"}
                type="button"
                title={driverUntouched ? t("session.driverStockTooltip") : t("session.driverTooltip")}
                onclick={() => requestSection("driver")}
              >
                <svg viewBox="0 0 16 16" aria-hidden="true">
                  <path d="M2.5 9.5a5.5 5.5 0 0 1 11 0v1.2a1.3 1.3 0 0 1-1.3 1.3H3.8a1.3 1.3 0 0 1-1.3-1.3z" />
                  <path d="M6.2 12v-1.6a1.8 1.8 0 0 1 1.8-1.8h5.5" />
                </svg>
                <span class="dl-name" class:stock={driverUntouched}>{driverLabel}</span>
                {#if driverBadge}
                  <span class="dl-badge" class:off={driverBadge === "disabled"}>
                    {t("session.driverBadge." + driverBadge)}
                  </span>
                {/if}
              </button>
            </div>
          {/if}
        </div>
        <div class="slot" class:on={nav.section === "tracks"}>
          <button
            class="slot-main"
            type="button"
            onclick={() => requestSection("tracks")}
            ondblclick={() => openSessionDetail("tracks", nav.sessionTrack?.id)}
            title={t("session.trackTooltip")}
          >
            <div class="slot-img track">
              {#if trackPrev}<img src={trackPrev} alt="" />{:else}<span class="slot-ic">🏁</span>{/if}
              {#if trackOutline}<img class="slot-outline" src={trackOutline} alt="" />{/if}
              <span class="slot-tag">{t("session.trackTag")}</span>
              <span class="slot-edit">{t("session.change")}</span>
            </div>
            <div class="slot-b">
              <div class="slot-name">
                {nav.sessionTrack?.name ?? t("session.noTrack")}
                {#if trackInactive}<span class="slot-warn" title={t("session.inactiveTooltip")}>⚠</span>{/if}
              </div>
              <div class="slot-meta">{nav.sessionTrack?.meta || t("session.clickToChoose")}</div>
            </div>
          </button>
          {#if nav.sessionTrack}
            <div class="slot-pick">
              <ImageSelectDropdown
                options={trackLayoutOptions}
                selectedId={nav.sessionTrack.layout}
                placeholder={t("session.pickLayout")}
                emptyText={t("session.noLayoutsAvailable")}
                onselect={pickTrackLayout}
                fit="contain"
              />
              <TrackSkinChecklistDropdown
                options={trackSkinChecklist}
                busy={trackSkinBusy}
                ontoggle={toggleTrackSkinFromSlot}
              />
            </div>
          {/if}
        </div>
        <button class="btn-configure" onclick={() => requestSection("race")}>{t("session.configure")}</button>
        <!-- Cible du bouton Start de la manette (§7.4bis) : il y amène le
             curseur depuis n'importe quel écran, il ne lance pas lui-même. -->
        <button class="btn-launch" {...{ [LAUNCH_BUTTON_ATTR]: "" }} onclick={launchNow}>{t("session.start")}</button>
      </div>

      <div class="nsec">{t("nav.addons")}</div>
      <div class="navgrid">
        {#each addons as b}
          <button class="nb" class:on={nav.section === b.id} onclick={() => requestSection(b.id)}>{t(b.labelKey)}</button>
        {/each}
      </div>

      <div class="nsec">{t("nav.atelier")}</div>
      <div class="navgrid">
        {#each atelier as b}
          <button class="nb" class:on={!b.action && nav.section === b.id} onclick={() => handleAtelierClick(b)}>{t(b.labelKey)}</button>
        {/each}
      </div>

      {#if bigPictureState.active}
        <!-- Seule sortie visible du mode Big Picture (plein écran, pas de
             chrome OS, barre de titre custom masquée) : bouton collant en
             bas de la barre latérale, position:sticky reste dans le flux
             normal donc ne peut jamais recouvrir les boutons au-dessus s'il
             manque de hauteur — il défile avec eux au lieu de les cacher. -->
        <button class="bigpicture-exit" type="button" onclick={exitBigPicture}>{t("bigpicture.exit")}</button>
      {/if}
    </aside>

    <div class="main-col">
      <main class="content" class:fixed={noPad} data-gp-region="main">
        {#if nav.section === "settings"}
          <Settings />
        {:else if nav.section === "about"}
          <About />
        {:else if nav.section === "cars"}
          <Library kind="Car" />
        {:else if nav.section === "tracks"}
          <Library kind="Track" />
        {:else if nav.section === "rules"}
          <RulesEditor />
        {:else if nav.section === "profiles"}
          <Profiles />
        {:else if nav.section === "driver"}
          <DriverScreen />
        {:else if nav.section === "race"}
          <Launch />
        {:else if nav.section === "maintenance"}
          <Maintenance />
        {:else if nav.section === "import"}
          <Import />
        {:else if nav.section === "carskins"}
          <Transversal variant="car" />
        {:else if nav.section === "trackskins"}
          <Transversal variant="track" />
        {:else if nav.section === "apps"}
          <Apps />
        {:else if nav.section === "others"}
          <OtherMods />
        {/if}
      </main>
    </div>
  </div>
</div>

<ImportOverlay />
<PendingDialog />

<!-- Tout ce que l'app a à dire sans interrompre, dans une seule colonne en bas
     à droite : progression et rapports d'import, nouveau périphérique. -->
<ToastStack>
  <ControllerToast />
  <BulkToasts />
  <ImportToasts />
</ToastStack>
{#if controllers.setupOpen}
  <ControllerSetup onclose={() => (controllers.setupOpen = false)} />
{/if}

<style>
  .frame {
    background: var(--panel);
    border: 1px solid var(--rosso);
    /* `zoom` (voir zoom.svelte.ts) agrandit tout le rendu, mais vh/vw restent
       relatifs à la fenêtre réelle — sans cette division, .frame devient plus
       haut que la fenêtre à >100% (rien à scroller pour atteindre le bas :
       bouton Enregistrer hors champ, coquille tronquée un peu partout). */
    height: calc(100vh / var(--ui-zoom, 1));
    /* Barre de titre custom en position fixe (voir TitleBar.svelte) : réserve
       sa hauteur ici plutôt que de la compter comme un enfant flex, pour
       qu'elle reste toujours à l'écran quel que soit ce qui défile en dessous. */
    padding-top: 32px;
    display: flex;
    flex-direction: column;
  }
  /* Big Picture : pas de barre de titre custom (masquée, gagne en hauteur)
     ni de bordure — rendu bord à bord, immersif. */
  .frame.bigpicture {
    padding-top: 0;
    border: none;
  }
  .topbar {
    background: var(--rosso);
    height: 3px;
    flex: none;
  }
  .frame.bigpicture .topbar {
    display: none;
  }
  .shell {
    flex: 1;
    min-height: 0;
    display: grid;
    /* 222px tant que la bibliothèque gardait son panneau de détail à droite ;
       celui-ci retiré, la zone principale n'a plus besoin d'autant de largeur
       et la colonne de session peut respirer — c'est elle qui porte le duo
       voiture/circuit et ses menus. Sa contrainte reste la **hauteur** : tout
       ce qu'on y ajoute doit tenir sans allonger la colonne, d'où la largeur
       prise ici (réglée à l'œil avec l'utilisateur). */
    grid-template-columns: 328px 1fr;
  }
  .side {
    background: var(--bg);
    border-right: 1px solid var(--line);
    overflow-y: auto;
  }
  .brand {
    display: flex;
    align-items: center;
    gap: 10px;
    padding: 14px 13px;
    border-bottom: 1px solid var(--line);
  }
  .logo {
    width: 26px;
    height: 26px;
    background: var(--rosso);
    display: flex;
    align-items: center;
    justify-content: center;
    transform: skewX(-8deg);
    flex: none;
  }
  .logo span {
    transform: skewX(8deg);
    color: #fff;
    font-size: 10px;
    font-weight: 700;
    font-style: italic;
  }
  .brand-name {
    font-size: 12px;
    font-weight: 600;
    letter-spacing: 1.5px;
    font-style: italic;
    line-height: 1;
  }
  .brand-sub {
    color: var(--rosso);
    font-size: 6.5px;
    letter-spacing: 2.5px;
    margin-top: 3px;
  }

  /* Titres de section : rouge, mono, séparateur (§6.1ter). */
  .nsec {
    color: var(--rosso);
    font-size: 9px;
    font-weight: 600;
    letter-spacing: 2px;
    padding: 14px 13px 8px;
    font-family: var(--mono);
    text-transform: uppercase;
    display: flex;
    align-items: center;
    gap: 8px;
  }
  .nsec::after {
    content: "";
    flex: 1;
    height: 1px;
    background: var(--rosso-border);
  }

  .session {
    padding: 0 13px 4px;
  }
  .session .nsec {
    padding-left: 0;
    padding-right: 0;
  }
  .slot {
    display: block;
    width: 100%;
    border: 1px solid var(--line);
    background: var(--panel);
    margin-bottom: 9px;
  }
  .slot:hover {
    border-color: var(--rosso-border);
  }
  .slot.on {
    border-color: var(--rosso);
  }
  .slot-main {
    display: block;
    width: 100%;
    text-align: left;
    background: transparent;
    border: none;
    padding: 0;
    cursor: pointer;
  }
  .slot-pick {
    display: flex;
    flex-direction: column;
    gap: 6px;
    padding: 8px;
    border-top: 1px solid var(--line);
  }
  /* Le menu de skin prend la place restante, la bascule ce qu'il lui faut. */
  /* Ligne « Mon pilote » : le point d'entrée de l'écran, sous le sélecteur de
     livrée. Un clic, jamais plus (§3.2). */
  .driver-line {
    display: flex;
    align-items: center;
    gap: 8px;
    width: 100%;
    height: 30px;
    padding: 0 9px;
    background: var(--panel);
    border: 1px solid var(--line);
    border-radius: 2px;
    color: var(--muted);
    font-size: 12px;
    text-align: left;
    cursor: pointer;
  }
  .driver-line svg {
    flex: 0 0 auto;
    width: 15px;
    height: 15px;
    fill: none;
    stroke: currentColor;
    stroke-width: 1.4;
    stroke-linecap: round;
    stroke-linejoin: round;
  }
  .driver-line:hover,
  .driver-line.on {
    border-color: var(--rosso-border);
    color: var(--txt);
  }
  .dl-name {
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }
  /* Rien de choisi : la ligne dit d'où vient la tenue, en retrait — c'est un
     état de fait, pas un réglage de l'utilisateur. */
  .dl-name.stock {
    color: var(--faint);
    font-style: italic;
  }
  .dl-badge {
    margin-left: auto;
    flex: 0 0 auto;
    font-size: 9.5px;
    letter-spacing: 0.12em;
    color: var(--rosso-bright);
    border: 1px solid var(--rosso-border);
    border-radius: 2px;
    padding: 1px 5px;
  }
  /* Voiture de course : le badge dit que le choix est suspendu, pas perdu —
     donc gris, pas rouge. */
  .dl-badge.off {
    color: var(--faint);
    border-color: var(--line);
  }
  .slot-img {
    /* **Un rapport, pas une hauteur fixe.** C'était `height: 96px`, et
       élargir la colonne a rogné les photos : à largeur croissante et hauteur
       figée, `object-fit: cover` agrandit l'image pour couvrir et coupe le
       haut et le bas — les roues disparaissaient. Le rapport ci-dessous est
       celui qu'avait la vignette à 222 px (222/96), donc le cadrage d'avant,
       et il le reste quelle que soit la largeur de la colonne. */
    aspect-ratio: 2.3;
    display: flex;
    align-items: center;
    justify-content: center;
    position: relative;
    border-bottom: 1px solid var(--line);
    overflow: hidden;
    background: linear-gradient(135deg, #1a0808, var(--panel));
  }
  .slot-img.track {
    background: linear-gradient(135deg, #0a1a14, var(--panel));
  }
  .slot-img img {
    width: 100%;
    height: 100%;
    object-fit: cover;
  }
  /* Tracé du layout superposé à la photo du circuit (comme la fiche). */
  .slot-img img.slot-outline {
    position: absolute;
    inset: 0;
    object-fit: contain;
    padding: 8px;
  }
  .slot-ic {
    font-size: 34px;
    opacity: 0.6;
  }
  .slot-tag {
    position: absolute;
    top: 6px;
    left: 6px;
    background: rgba(8, 8, 12, 0.75);
    color: var(--muted);
    font-size: 7px;
    letter-spacing: 1.5px;
    font-family: var(--mono);
    padding: 2px 6px;
  }
  .slot-edit {
    position: absolute;
    bottom: 6px;
    right: 6px;
    background: rgba(8, 8, 12, 0.8);
    color: var(--rosso-bright);
    font-size: 7px;
    letter-spacing: 1px;
    font-family: var(--mono);
    padding: 2px 7px;
  }
  .slot-b {
    padding: 7px 10px;
  }
  .slot-name {
    font-size: 11.5px;
    font-weight: 600;
    color: var(--txt);
    white-space: nowrap;
    overflow: hidden;
    text-overflow: ellipsis;
  }
  /* Mod sélectionné mais non activé (§ garde-fou lancement) : jaune = alerte,
     cohérent avec les couleurs sémantiques du projet. */
  .slot-warn {
    color: var(--yellow);
    margin-left: 4px;
  }
  .slot-meta {
    color: var(--muted);
    font-size: 8.5px;
    font-family: var(--mono);
    margin-top: 2px;
    white-space: nowrap;
    overflow: hidden;
    text-overflow: ellipsis;
  }
  .btn-configure {
    width: 100%;
    height: 36px;
    background: var(--raised);
    border: 1px solid var(--line);
    color: var(--txt2);
    font-size: 9.5px;
    letter-spacing: 1.5px;
    font-weight: 600;
    font-family: var(--mono);
    margin-top: 8px;
  }
  .btn-configure:hover {
    background: var(--card);
    border-color: var(--faint);
  }
  .btn-launch {
    width: 100%;
    height: 40px;
    background: var(--rosso);
    color: #fff;
    font-size: 10.5px;
    letter-spacing: 1.5px;
    font-weight: 600;
    font-family: var(--mono);
    margin-top: 2px;
  }
  .btn-launch:hover {
    background: var(--rosso-bright);
  }

  .navgrid {
    display: grid;
    grid-template-columns: 1fr 1fr;
    gap: 1px;
    background: var(--line);
    border: 1px solid var(--line);
    margin: 0 13px;
  }
  .nb {
    background: var(--bg);
    color: var(--muted);
    padding: 9px 10px;
    text-align: left;
    font-size: 11px;
  }
  .nb:hover {
    background: var(--raised);
    color: var(--txt);
  }
  .nb.on {
    background: var(--raised);
    color: var(--rosso-bright);
  }

  .main-col {
    display: flex;
    flex-direction: column;
    min-width: 0;
    min-height: 0;
  }
  .content {
    flex: 1;
    min-height: 0;
    padding: 28px 32px;
    overflow: auto;
  }
  /* Bibliothèques : hauteur fixe + défilement interne (évite la double scrollbar). */
  .content.fixed {
    padding: 0;
    overflow: hidden;
  }

  /* Seule sortie visible du mode Big Picture (plein écran, pas de chrome
     OS, barre de titre custom masquée). Dernier enfant de .side : `sticky`
     reste dans le flux normal (contrairement à `fixed`), donc ne peut pas
     recouvrir les boutons de navigation au-dessus s'il manque de hauteur —
     il défile avec eux au lieu de les cacher. Couleur bleue (secondaire,
     "info" — le rouge est déjà pris par primaire/destructif partout
     ailleurs) pour ne pas se confondre avec Lancer/Paramétrage. */
  .bigpicture-exit {
    position: sticky;
    bottom: 0;
    width: 100%;
    margin-top: 8px;
    padding: 12px 13px;
    background: var(--blue-dim);
    border-top: 1px solid var(--blue-border);
    color: var(--blue);
    font-size: 11px;
    font-weight: 600;
    letter-spacing: 1px;
    font-family: var(--mono);
  }
  .bigpicture-exit:hover {
    background: var(--blue-border);
    color: var(--txt);
  }
</style>
