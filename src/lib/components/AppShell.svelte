<script lang="ts">
  import { onMount } from "svelte";
  import Settings from "./Settings.svelte";
  import Library from "./Library.svelte";
  import RulesEditor from "./RulesEditor.svelte";
  import Profiles from "./Profiles.svelte";
  import Launch from "./Launch.svelte";
  import Maintenance from "./Maintenance.svelte";
  import Transversal from "./Transversal.svelte";
  import Apps from "./Apps.svelte";
  import OtherMods from "./OtherMods.svelte";
  import Import from "./Import.svelte";
  import ImportOverlay from "./ImportOverlay.svelte";
  import TitleBar from "./TitleBar.svelte";
  import { nav, requestSection } from "$lib/nav.svelte";
  import { previewSrc } from "$lib/library";
  import { initGlobalDragDrop } from "$lib/importState.svelte";
  import { openContentManager } from "$lib/launch";
  import { t, setLocale } from "$lib/i18n/index.svelte";
  import { setZoom } from "$lib/zoom.svelte";
  import { getConfig } from "$lib/config";
  import { startGamepadNav } from "$lib/gamepadNav";

  // Barre latérale unifiée (maquette pitbox-biblio-session2.html) : bloc
  // SESSION (le duo sélectionné = point d'accès aux bibliothèques) puis
  // ADD-ONS et ATELIER en deux colonnes.
  type NavBtn = { id: string; labelKey: string; full?: boolean; action?: boolean };
  const addons: NavBtn[] = [
    { id: "skins", labelKey: "nav.skins" },
    { id: "sounds", labelKey: "nav.sounds" },
    { id: "others", labelKey: "nav.others", full: true },
    { id: "apps", labelKey: "nav.apps", full: true },
  ];
  const atelier: NavBtn[] = [
    { id: "rules", labelKey: "nav.rules" },
    { id: "import", labelKey: "nav.import" },
    { id: "profiles", labelKey: "nav.profiles" },
    { id: "maintenance", labelKey: "nav.maintenance" },
    // Action directe, pas un écran : n'affecte jamais nav.section.
    { id: "opencm", labelKey: "nav.openCm", action: true },
    { id: "settings", labelKey: "nav.settings", full: true },
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

  // Navigation manette dans toute l'app (croix/stick = déplace le focus,
  // A/Croix = valide, B/Rond = ferme la fiche pleine page). Un seul scrutin
  // global, monté une fois ici.
  onMount(() => startGamepadNav());

  // Langue forcée par l'utilisateur (Réglages), sinon langue système (déjà
  // appliquée par défaut par le module i18n). Zoom d'interface, idem.
  onMount(async () => {
    const cfg = await getConfig();
    if (cfg.prefs.language) setLocale(cfg.prefs.language);
    setZoom(cfg.prefs.ui_zoom);
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

  // Bouton rouge « Démarrer la session » : lance directement avec les
  // réglages courants (dernier preset du type de session), sans repasser par
  // l'écran Paramétrage — pose le drapeau consommé par Launch.svelte une fois
  // monté et prêt (mêmes valeurs que si l'écran avait été ouvert normalement).
  async function launchNow() {
    nav.autoLaunch = true;
    if (!(await requestSection("race"))) nav.autoLaunch = false;
  }
</script>

<TitleBar />
<div class="frame">
  <div class="topbar"></div>
  <div class="shell">
    <aside class="side">
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
        <button
          class="slot"
          class:on={nav.section === "cars"}
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
            <div class="slot-name">{nav.sessionCar?.name ?? t("session.noCar")}</div>
            <div class="slot-meta">{nav.sessionCar?.meta || t("session.clickToChoose")}</div>
          </div>
        </button>
        <button
          class="slot"
          class:on={nav.section === "tracks"}
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
            <div class="slot-name">{nav.sessionTrack?.name ?? t("session.noTrack")}</div>
            <div class="slot-meta">{nav.sessionTrack?.meta || t("session.clickToChoose")}</div>
          </div>
        </button>
        <button class="btn-configure" onclick={() => requestSection("race")}>{t("session.configure")}</button>
        <button class="btn-launch" onclick={launchNow}>{t("session.start")}</button>
      </div>

      <div class="nsec">{t("nav.addons")}</div>
      <div class="navgrid">
        {#each addons as b}
          <button class="nb" class:full={b.full} class:on={nav.section === b.id} onclick={() => requestSection(b.id)}>{t(b.labelKey)}</button>
        {/each}
      </div>

      <div class="nsec">{t("nav.atelier")}</div>
      <div class="navgrid">
        {#each atelier as b}
          <button class="nb" class:full={b.full} class:on={!b.action && nav.section === b.id} onclick={() => handleAtelierClick(b)}>{t(b.labelKey)}</button>
        {/each}
      </div>
    </aside>

    <main class="content" class:fixed={noPad}>
      {#if nav.section === "settings"}
        <Settings />
      {:else if nav.section === "cars"}
        <Library kind="Car" />
      {:else if nav.section === "tracks"}
        <Library kind="Track" />
      {:else if nav.section === "rules"}
        <RulesEditor />
      {:else if nav.section === "profiles"}
        <Profiles />
      {:else if nav.section === "race"}
        <Launch />
      {:else if nav.section === "maintenance"}
        <Maintenance />
      {:else if nav.section === "import"}
        <Import />
      {:else if nav.section === "skins"}
        <Transversal subType="SKIN" />
      {:else if nav.section === "sounds"}
        <Transversal subType="SOUND" />
      {:else if nav.section === "apps"}
        <Apps />
      {:else if nav.section === "others"}
        <OtherMods />
      {/if}
    </main>
  </div>
</div>

<ImportOverlay />

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
  .topbar {
    background: var(--rosso);
    height: 3px;
    flex: none;
  }
  .shell {
    flex: 1;
    min-height: 0;
    display: grid;
    grid-template-columns: 222px 1fr;
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
    text-align: left;
    border: 1px solid var(--line);
    background: var(--panel);
    margin-bottom: 9px;
    padding: 0;
  }
  .slot:hover {
    border-color: var(--rosso-border);
  }
  .slot.on {
    border-color: var(--rosso);
  }
  .slot-img {
    height: 96px;
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
  .nb.full {
    grid-column: 1 / -1;
  }

  .content {
    min-height: 0;
    padding: 28px 32px;
    overflow: auto;
  }
  /* Bibliothèques : hauteur fixe + défilement interne (évite la double scrollbar). */
  .content.fixed {
    padding: 0;
    overflow: hidden;
  }
</style>
