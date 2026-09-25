<script lang="ts">
  // Hero of the detail page: the photo, or the in-app 3D preview of a car, with
  // its hover tools; for a track, the layout outline over the photo.
  //
  // Aperçu 3D maison (KN5 → glTF → three.js, docs/SPEC-preview-3d-kn5.md) :
  // il **coexiste** avec le showroom natif, il ne le remplace pas. Les deux ne
  // rendent pas le même service : celui-ci est inline et manipulable dans la
  // fiche, l'autre donne le rendu fidèle du jeu.
  // La bascule et les réglages de cadrage vivent dans `preview3dPrefs` : le
  // même réglage se change aussi depuis l'écran Réglages, et les deux doivent
  // rester d'accord sans qu'aucun des deux écrans n'ait à être remonté.
  import CarPreview3D from "./CarPreview3D.svelte";
  import {
    preview3dPrefs,
    resetPreview3dView,
    savePreview3dPrefs,
    setPreview3dEnabled,
  } from "$lib/preview3d/preview3dPrefs.svelte";
  import { nav } from "$lib/shell/nav.svelte";
  import { t } from "$lib/i18n/index.svelte";

  interface Props {
    car: boolean;
    /** Mod id: the car the 3D preview loads. */
    modId: string;
    /** Alt text of the photo. */
    name: string;
    /** Photo: the selected skin of a car, the selected layout of a track. */
    image: string | null;
    skinId: string | null;
    carClass: string | null;
    /** Content revision of the page — see `contentRevision` in DetailPage. */
    revision: number;
    /** Layout outline drawn over a track photo. */
    outline: string | null;
    /** The native showroom is starting: a small spinner in the corner. */
    showroomBusy: boolean;
    /** Settings panel open over the preview. Owned by the page so that it
     * survives a round trip through another tab, as it always has. */
    panelOpen: boolean;
  }
  let {
    car,
    modId,
    name,
    image,
    skinId,
    carClass,
    revision,
    outline,
    showroomBusy,
    panelOpen = $bindable(),
  }: Props = $props();

  const preview3d = $derived(preview3dPrefs().enabled);

  /** Raccourci vers Réglages → Aperçu, qui porte l'aperçu 3D et les treize
   * réglages. Passe par `nav.settingsTab` : l'onglet actif est un état interne
   * de `Settings.svelte`, et le lui demander avant de naviguer évite de sortir
   * cet état de son composant pour un seul appelant. */
  function openPreviewSettings() {
    nav.settingsTab = "preview";
    nav.section = "settings";
  }

  // **Pas de bouton « refaire la vignette » ici**, et c'est un retrait, pas un
  // oubli. Il a existé, pour le cas d'un rendu abîmé sans que le mod y soit
  // pour rien — contexte WebGL perdu, textures non arrivées. Le contrôle de
  // plausibilité qui refuse ces images avant de les écrire
  // (`gridThumbs.svelte.ts`) est censé rendre le cas impossible ; on le vérifie
  // à l'usage plutôt que de garder un bouton de dépannage sur un écran qui n'en
  // a pas besoin. `regenerateGridThumb` reste, un `{#if}` de le ramener.

  function togglePreview3d() {
    setPreview3dEnabled(!preview3d);
    // Enregistré sur-le-champ, contrairement aux curseurs de l'écran Réglages :
    // c'est un interrupteur, pas un formulaire. Personne ne s'attend à devoir
    // aller valider ailleurs pour qu'une bascule d'un clic tienne.
    void savePreview3dPrefs().catch((e) => console.error("save_ui_prefs", e));
  }
</script>

<div class="hero" class:car>
  <!-- Photo et aperçu 3D partagent le même cadre (§ correctif marge) :
       la photo est un enfant normal, `CarPreview3D` se pose en absolu
       `inset:0` — sans ce conteneur commun, chacun résolvait sa marge
       contre un ancêtre différent (`.hero` avec son padding pour l'un,
       `.hero` sans aucun pour l'autre), d'où le décalage constaté entre
       les deux vues. -->
  <div class="hero-inner">
    <!-- **Rien sous l'aperçu 3D.** La photo est un enfant normal et
         l'aperçu se pose par-dessus en absolu : tant que le modèle n'est
         pas prêt, son canevas est transparent et c'est donc la photo
         qu'on voyait — celle du premier skin, derrière le témoin de
         chargement. Quand l'aperçu 3D tient la zone, il la tient
         entièrement ; c'est lui qui remet la photo, entière, s'il ne peut
         pas aboutir (`fallbackSrc`). -->
    {#if !(car && preview3d)}
      {#if image}
        <img src={image} alt={name} />
      {:else}
        <div class="hero-icon">{car ? "🚗" : "🏁"}</div>
      {/if}
    {/if}
    {#if car && preview3d}
      <CarPreview3D carId={modId} {skinId} fallbackSrc={image} {carClass} {revision} />
    {/if}
  </div>
  {#if car}
    <!-- Commandes de l'aperçu : révélées au survol de la zone héros, pour
         qu'elles ne mangent pas l'image le reste du temps. Le focus
         clavier les révèle aussi (`:focus-within`), sans quoi elles
         seraient inatteignables autrement qu'à la souris. -->
    <div class="hero-tools" class:open={panelOpen}>
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
          class:on={panelOpen}
          type="button"
          onclick={() => (panelOpen = !panelOpen)}
          title={t("detail.preview3dSettings")}
          aria-label={t("detail.preview3dSettings")}
          aria-expanded={panelOpen}
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
    {#if preview3d && panelOpen}
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
  {#if !car && outline}<img class="hero-outline" src={outline} alt="" />{/if}
</div>

<style>
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
  .hero.car {
    --hero-pad: 16px;
    aspect-ratio: 16 / 9;
    min-height: 0;
    padding: 0;
  }
  .hero.car .hero-inner {
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
</style>
