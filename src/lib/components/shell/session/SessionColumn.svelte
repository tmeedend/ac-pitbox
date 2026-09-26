<script lang="ts">
  // The session column (SPEC §7.2): the middle territory of the shell, the one
  // that says what will be launched. It assembles the two mod blocks, the
  // session types and the launch button, and holds what they share: the fresh
  // details of the session car and track, which both the blocks and the
  // activation guard read.
  import SessionSlot, { type SlotState } from "./SessionSlot.svelte";
  import TrackFields from "./TrackFields.svelte";
  import CarFields from "./CarFields.svelte";
  import SessionTypes from "./SessionTypes.svelte";
  import { gridMods } from "$lib/launch/gridMods.svelte";
  import { bumpLibraryVersion, libraryVersion } from "$lib/library/libraryVersion.svelte";
  import { nav, requestSection, openInSection } from "$lib/shell/nav.svelte";
  import { previewSrc, getModDetail, activateMod } from "$lib/library/library";
  import { withoutBrand } from "$lib/library/displayName";
  import { peekUiPref } from "$lib/uiPrefs.svelte";
  import { StorageKey } from "$lib/storage";
  import { message } from "@tauri-apps/plugin-dialog";
  import { errorText } from "$lib/errors";
  import { untrack } from "svelte";
  import { t } from "$lib/i18n/index.svelte";
  import { zoomFactor } from "$lib/shell/zoom.svelte";
  import { LAUNCH_BUTTON_ATTR } from "$lib/shell/gamepadNav";

  interface Props {
    /** Assetto Corsa or its content folder cannot be found (SPEC SESSION§1). */
    pathsBroken: boolean;
    /** On a screen of the session zone (SPEC §7.2). Hidden rather than
     * unmounted: the fresh details of the pair would otherwise be fetched
     * again, name and badge flickering in, at every return to a library. */
    shown: boolean;
  }
  let { pathsBroken, shown }: Props = $props();

  /** "You are here" inside the zone (SPEC §7.2): the card of the screen on
   * display carries the same left rule as the active rail entry, since the
   * rail's single `Session` entry no longer tells cars from tracks. The driver
   * screen marks its own line, in `CarFields`. */
  const trackHere = $derived(nav.section === "tracks");
  const carHere = $derived(nav.section === "cars");

  const carSlot = $derived<SlotState>(nav.sessionCar ? "picked" : pathsBroken ? "broken" : "empty");
  const trackSlot = $derived<SlotState>(nav.sessionTrack ? "picked" : pathsBroken ? "broken" : "empty");
  /** Tant que le duo n'est pas complet, il n'y a ni session à paramétrer ni
   * session à lancer (SPEC SESSION§1) — l'app ne choisit pas une voiture à la place de
   * l'utilisateur pour se donner un bouton à activer. */
  const sessionReady = $derived(nav.sessionCar != null && nav.sessionTrack != null);

  // --- Colonne d'intitulés partagée (SPEC SESSION§1) -----------------------------
  //
  // Les quatre champs alignent leurs valeurs sur une même colonne d'intitulé :
  // c'est ce qui fait lire le bloc comme une fiche technique plutôt que comme
  // une pile de menus, et ça disparaît si chaque ligne se dimensionne seule.
  // 60 px conviennent au français ; une autre langue peut demander plus
  // (LACKIERUNG en allemand), d'où une mesure une fois par langue plutôt
  // qu'une constante — plafonnée à 88 px, au-delà l'intitulé tronque.
  const FIELD_LABELS = $derived([
    t("session.fieldLivery"),
    t("session.fieldDriver"),
    t("session.fieldPerformance"),
    t("session.fieldLayout"),
    t("session.fieldTrackSkin"),
  ]);
  let labelProbe = $state<HTMLElement | null>(null);
  let labelWidth = $state(60);
  $effect(() => {
    // Dépendance explicite : c'est le changement de langue qui doit relancer
    // la mesure, et il ne passe que par le contenu du gabarit caché.
    FIELD_LABELS;
    const el = labelProbe;
    // Hidden, the probe has no width to read — and the language is changed in
    // Settings, precisely where the column is hidden: measure again on return.
    if (!shown || !el) return;
    // `getBoundingClientRect` rend des pixels RÉELS de fenêtre, déjà
    // multipliés par le zoom d'interface, alors que la valeur repart dans un
    // `style` en pixels CSS que le zoom multipliera à son tour — sans cette
    // division, la colonne s'élargirait à chaque cran de zoom.
    const f = zoomFactor();
    let widest = 0;
    for (const child of Array.from(el.children)) {
      widest = Math.max(widest, (child as HTMLElement).getBoundingClientRect().width / f);
    }
    labelWidth = Math.min(88, Math.max(60, Math.ceil(widest)));
  });

  /**
   * **Un clic ouvre la bibliothèque, deux ouvrent la fiche — et les deux gestes
   * sont comptés ici, jamais confiés à `ondblclick`.**
   *
   * Les deux étaient posés côte à côte sur le même bouton, `onclick` agissant
   * tout de suite. Deux défauts en découlaient, dont l'un se voyait :
   *
   * - **Le double-clic n'ouvrait pas la fiche** quand on n'était pas déjà sur
   *   la bibliothèque visée (signalé à l'usage). Le premier clic changeait de
   *   section, donc remontait tout l'écran principal et remplaçait le contenu
   *   sous le curseur, entre les deux moitiés du geste — et un `dblclick` ne
   *   part que si les deux clics tombent sur le même élément, dans le temps
   *   système ET sans que la cible ait bougé. Impossible à reproduire en
   *   entrée synthétique, où les deux clics partent avant tout rendu : c'est
   *   précisément le genre de course qu'on ne corrige pas en la retentant.
   * - **L'historique enregistrait un écran que personne n'a regardé** : la
   *   liste de la bibliothèque, ouverte par le premier clic, puis la fiche.
   *   « Précédent » y ramenait, ce que `openInSection` existe justement pour
   *   éviter (§7.2bis).
   *
   * Compter les clics soi-même règle les deux d'un coup : rien ne part avant
   * que le geste ne soit fini, donc aucun rendu ne s'intercale et aucun écran
   * intermédiaire n'existe. Le prix est un quart de seconde d'attente sur le
   * clic simple — le prix habituel d'une cible qui porte deux gestes.
   */
  const DOUBLE_CLICK_MS = 260;
  let slotClicks = 0;
  let slotTimer: ReturnType<typeof setTimeout> | null = null;

  function pressSlot(section: "cars" | "tracks", state: SlotState, id: string | null | undefined) {
    slotClicks += 1;
    if (slotTimer) clearTimeout(slotTimer);
    slotTimer = setTimeout(() => {
      const clicks = slotClicks;
      slotClicks = 0;
      slotTimer = null;
      // Un emplacement vide ou en impasse n'a pas de fiche à ouvrir : le
      // double-clic y vaut le simple, il mène où le simple mène.
      if (clicks >= 2 && state === "picked" && id) void openSessionDetail(section, id);
      else void openSlot(section, state);
    }, DOUBLE_CLICK_MS);
  }

  async function openSlot(section: "cars" | "tracks", state: SlotState) {
    if (state === "broken") {
      nav.settingsTab = "paths";
      await requestSection("settings");
      return;
    }
    await requestSection(section);
  }

  // Double-clic sur le slot de session : ouvre directement la fiche détail de
  // l'entité choisie (skin, layout…) plutôt que la liste de la bibliothèque.
  async function openSessionDetail(section: "cars" | "tracks", id: string | null | undefined) {
    // `openInSection` plutôt que `requestSection` puis `openMod` : les deux
    // écritures d'affilée n'exposent qu'un seul état, sinon l'historique
    // enregistre la liste de la bibliothèque comme un écran traversé et
    // « précédent » y ramène au lieu de rendre l'écran d'où l'on vient.
    if (id) await openInSection(section, id);
    else await requestSection(section);
  }

  const carPrev = $derived(previewSrc(nav.sessionCar?.preview ?? null));
  const trackPrev = $derived(previewSrc(nav.sessionTrack?.preview ?? null));
  const trackOutline = $derived(previewSrc(nav.sessionTrack?.outline ?? null));

  // État d'activation frais du duo de session (§ garde-fou lancement
  // ci-dessous) : jamais déduit de `nav.sessionCar`/`sessionTrack` eux-mêmes
  // (juste id/nom/preview pour l'affichage, persistés tels quels — une donnée
  // d'activation qui y serait figée resterait fausse dès que l'état change
  // ailleurs, ex. désactivé depuis la fiche détail sans repasser par ce
  // sélecteur). Chaque effet est clé sur l'id, ce qui écarte déjà les
  // réponses obsolètes.
  let carDetail = $state<Awaited<ReturnType<typeof getModDetail>>>(null);
  let trackDetail = $state<Awaited<ReturnType<typeof getModDetail>>>(null);

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
      return;
    }
    getModDetail(trackId).then((d) => {
      if (nav.sessionTrack?.id === trackId) trackDetail = d;
    });
  });

  // `null` tant que le détail n'est pas encore chargé (juste après une
  // sélection) : pas d'avertissement affiché dans ce court intervalle plutôt
  // que de risquer un faux positif pendant le chargement.
  const carInactive = $derived(nav.sessionCar != null && carDetail != null && !carDetail.active);
  const trackInactive = $derived(nav.sessionTrack != null && trackDetail != null && !trackDetail.active);

  /**
   * Le nom de la voiture tel que la colonne l'écrit — sans sa marque, qui est
   * juste au-dessus.
   *
   * Suit le réglage de la bibliothèque plutôt que de trancher dans son coin :
   * c'est la même décision d'affichage, elle n'a pas à se prendre deux fois.
   * `!== "0"` parce que l'option est désormais active par défaut, et
   * `peekUiPref` parce qu'un nom se rend, il ne s'attend pas — la valeur est
   * dans le cache réactif dès que la bibliothèque a chargé ses préférences, et
   * son absence donne le défaut, pas un écran vide.
   */
  const hideBrandPref = $derived(peekUiPref(StorageKey.gridHideBrand) !== "0");
  const sessionCarName = $derived.by(() => {
    const name = nav.sessionCar?.name ?? "";
    return hideBrandPref ? withoutBrand(name, carDetail?.brand ?? null) : name;
  });

  // --- Garde d'activation (SESSION§3) ---
  //
  // La bibliothèque montre les mods désactivés, Assetto Corsa ne les voit pas :
  // lancer une session qui en contient échoue, et c'est un trou propre à Pit
  // Box — Content Manager ne montre que ce qui est installé.
  //
  // **Une ligne au-dessus du bouton, pas un dialogue au lancement.** C'était
  // un `confirm()` posé au clic : au moment où on l'ouvre, on est déjà parti
  // mentalement, et il ne couvrait que la voiture et le circuit. La ligne
  // couvre les trois, se voit avant de cliquer, et porte son remède ; le
  // bouton reste verrouillé tant qu'elle est là, donc aucune session ne peut
  // partir en échec.
  //
  // **Les doublons comptent pour un** : trois adversaires sur la même voiture
  // inactive font une seule activation, et la ligne annonce un mod, pas trois.
  let opponentDetails = $state<Record<string, Awaited<ReturnType<typeof getModDetail>>>>({});
  $effect(() => {
    const ids = gridMods.carIds;
    for (const id of ids) {
      if (id in untrack(() => opponentDetails)) continue;
      getModDetail(id).then((d) => {
        opponentDetails = { ...untrack(() => opponentDetails), [id]: d };
      });
    }
  });

  const inactiveMods = $derived.by(() => {
    const out = new Map<string, string>();
    if (carInactive && nav.sessionCar) out.set(nav.sessionCar.id, nav.sessionCar.name);
    if (trackInactive && nav.sessionTrack) out.set(nav.sessionTrack.id, nav.sessionTrack.name);
    for (const id of gridMods.carIds) {
      const d = opponentDetails[id];
      if (d && !d.active) out.set(id, d.display_name ?? id);
    }
    return [...out].map(([id, name]) => ({ id, name }));
  });

  let activating = $state(false);
  async function activateInactive() {
    if (activating) return;
    activating = true;
    try {
      for (const m of inactiveMods) await activateMod(m.id);
      // Relit tout de suite l'état frais : la ligne doit disparaître au clic,
      // pas au prochain changement de sélection.
      if (nav.sessionCar) carDetail = await getModDetail(nav.sessionCar.id);
      if (nav.sessionTrack) trackDetail = await getModDetail(nav.sessionTrack.id);
      const fresh: Record<string, Awaited<ReturnType<typeof getModDetail>>> = {};
      for (const id of gridMods.carIds) fresh[id] = await getModDetail(id);
      opponentDetails = fresh;
      bumpLibraryVersion();
    } catch (e) {
      await message(errorText(e), { title: t("session.activateFailedTitle"), kind: "error" });
    } finally {
      activating = false;
    }
  }

  // Bouton rouge « Démarrer la session » : lance directement avec les
  // réglages courants (dernier preset du type de session), sans repasser par
  // l'écran Paramétrage — pose le drapeau consommé par Launch.svelte une fois
  // monté et prêt (mêmes valeurs que si l'écran avait été ouvert normalement).
  //
  // L'activation n'est plus demandée ici : la garde ci-dessus verrouille le
  // bouton tant qu'un mod de la session est inactif, donc ce chemin n'est
  // atteint que sur une session lançable. L'activation reste **explicite** —
  // lancer une course ne doit pas modifier la bibliothèque dans le dos de
  // l'utilisateur, même si les liens durs rendent l'opération réversible.
  async function launchNow() {
    nav.autoLaunch = true;
    if (!(await requestSection("race"))) nav.autoLaunch = false;
  }
</script>

<!-- Zone parcourue par les gâchettes hautes de la manette (§7.4bis), comme
     l'écran actif de l'autre côté. -->
<aside class="side" data-gp-region="sidebar" hidden={!shown}>
  <!-- La colonne répond à une seule question — « qu'est-ce que je
       lance ? » — et les deux blocs de mods ont exactement la même
       anatomie : vignette, nom, source, champs. Plus de traitement
       d'exception sur le bloc voiture.

       **L'ordre suit celui de la décision** : le circuit, puis la voiture
       qu'on y emmène, puis le genre de séance qu'on y fait — et le bouton
       de lancement tombe juste sous la liste des types, qui est le dernier
       choix avant de partir. -->
  <div class="session" style="--sess-lblw:{labelWidth}px">
    <div class="nsec">{t("session.trackTag")}</div>
    <div class="blk" class:here={trackHere}>
      <SessionSlot
        kind="track"
        slot={trackSlot}
        preview={trackPrev}
        outline={trackOutline}
        name={nav.sessionTrack?.name ?? null}
        year={trackDetail?.year}
        inactive={trackInactive}
        source={nav.sessionTrack?.meta}
        onpress={() => pressSlot("tracks", trackSlot, nav.sessionTrack?.id)}
      />
      {#if nav.sessionTrack}
        <TrackFields {trackDetail} />
      {/if}
    </div>

    <div class="nsec section">{t("session.carTag")}</div>
    <div class="blk" class:here={carHere}>
      <SessionSlot
        kind="car"
        slot={carSlot}
        preview={carPrev}
        name={nav.sessionCar?.name ?? null}
        displayName={sessionCarName}
        badge={carDetail?.badge}
        brand={carDetail?.brand}
        year={carDetail?.year}
        reserve
        inactive={carInactive}
        source={carDetail?.author}
        onpress={() => pressSlot("cars", carSlot, nav.sessionCar?.id)}
      />
      {#if nav.sessionCar}
        <CarFields {carDetail} carName={sessionCarName} />
      {/if}
    </div>

    <div class="nsec section">{t("nav.session")}</div>
    <SessionTypes />

    {#if inactiveMods.length}
      <div class="warnbox guard">
        <span aria-hidden="true">⚠</span>
        <span class="guard-txt"
          >{inactiveMods.length === 1
            ? t("session.inactiveOne")
            : t("session.inactiveMany", { count: inactiveMods.length })}</span
        >
        <button class="guard-btn" type="button" disabled={activating} onclick={activateInactive}
          >{t("common.activate")}</button
        >
      </div>
    {/if}
    <!-- Cible du bouton Start de la manette (§7.4bis) : il y amène le
         curseur depuis n'importe quel écran, il ne lance pas lui-même. -->
    <button
      class="btn-launch"
      disabled={!sessionReady || inactiveMods.length > 0}
      {...{ [LAUNCH_BUTTON_ATTR]: "" }}
      onclick={launchNow}>{t("session.start")}</button
    >
    <!-- La sortie vers Content Manager a rejoint le pied du rail
         (`NavRail`), entre Réglages et À propos : cette colonne n'a plus
         de hauteur à donner à ce qui n'est pas la session. -->
  </div>

  <!-- Gabarit de mesure des intitulés de champ : hors flux, invisible, et
       surtout NON contraint en largeur — c'est la largeur naturelle du
       plus long qui décide de la colonne. -->
  <div class="lbl-probe" aria-hidden="true" bind:this={labelProbe}>
    {#each FIELD_LABELS as l}<span>{l}</span>{/each}
  </div>
</aside>

<style>
  .side {
    background: var(--bg);
    border-right: 1px solid var(--line);
    overflow-y: auto;
    /* **La colonne ne doit jamais défiler** (L5§2.3), et ce qu'on réduit
       quand le compte n'y est pas, ce sont les deux vignettes. Le seuil est
       donc une requête de CONTENEUR et non de média : une `@media
       (max-height)` interroge la fenêtre, que le zoom d'interface ne touche
       pas — à 150 %, une fenêtre de 1080 px n'offre plus que 720 px de mise en
       page et la règle ne se déclencherait pas. Le conteneur, lui, mesure la
       hauteur réellement disponible.
       960 et non les 900 de la spec : c'est une **mesure**, pas un nombre rond
       — la colonne la plus chargée (Course sélectionnée, sous-entrée affichée,
       replis fermés) faisait un millier de pixels, relevés à l'écran par
       bissection du seuil jusqu'à ce qu'il bascule, et le départ du bandeau de
       marque vers la barre de titre lui en a rendu une cinquantaine. En dessous
       de ce seuil elle n'a plus de marge, et c'est exactement là qu'il faut
       réagir : 900 l'aurait laissée déborder de quelques pixels — le bas de la
       colonne passe alors sous le bord de la fenêtre — avant que la règle ne se
       déclenche. Une fenêtre de 1920 × 1080 au zoom d'origine offre 1044 px à
       la colonne : elle garde ses vignettes entières, ce que la spec demande.
       La règle elle-même vit avec les vignettes, dans `SessionSlot`. */
    container: sidecol / size;
  }
  /* Titres de section de la colonne : mono, majuscules espacées, séparateur.
     Plus rouges depuis le barème de l'accent (SPEC §7.2ter) : un titre de section est de la
     STRUCTURE, pas un état — et ils sont assez nombreux, répartis sur toute
     la hauteur de la colonne, pour que leur filet mette le seul bouton rouge
     de l'écran (« Démarrer la session ») en concurrence avec quatre titres.
     Les capitales espacées suffisent à les faire lire comme des titres. */
  .nsec {
    color: var(--muted);
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
    background: var(--line);
  }

  .session {
    padding: 0 13px 4px;
  }
  .session .nsec {
    padding-left: 0;
    padding-right: 0;
  }
  /* Le second titre suit un champ, pas une marge de bloc : il lui faut un peu
     plus d'air pour que les deux blocs se lisent comme deux blocs. */
  .session .nsec.section {
    padding-top: 18px;
  }
  /* Un bloc = vignette cliquable + champs. Aucune bordure de SÉLECTION : elle
     n'aurait de sens que parmi des pairs, or il n'y a qu'une voiture. Le seul
     trait rouge qu'il peut porter dit autre chose — l'écran affiché, voir
     `.here` plus bas.
     Mais un plan, oui — et le même que la carte de bibliothèque (`--cell` /
     `--cell-line`), puisque les deux montrent désormais la même chose avec le
     même composant. Le `.blk` global posait l'inverse : un fond PLUS SOMBRE
     que la colonne, sans marge intérieure — donc un texte collé au filet, et
     une boîte qui se lisait comme un trou plutôt que comme une carte. Les
     8 px sont ceux de `.card`, pas une valeur de plus à faire diverger. */
  .blk {
    display: block;
    background: var(--cell);
    border-color: var(--cell-line);
    padding: 8px;
    /* La colonne de session est large de 328 px : le nom y a la place de la
       grille confortable, pas celle de la dense. */
    --ident-size: 12.5px;
  }
  /* **`:global` obligatoire, et ce n'était pas une précaution de style.** Les
     enfants de ce bloc sont des COMPOSANTS (`SessionSlot`,
     `ImageSelectDropdown`, `TrackSkinChecklistDropdown`…) : leur élément
     racine porte le hachage de leur propre fichier, pas celui-ci. Svelte
     compilait donc `.blk > * + *` en
     `.blk.svelte-xxx > :where(.svelte-xxx) + :where(.svelte-xxx)`,
     que le `<div class="isd">` du sélecteur ne satisfait jamais — **la règle
     des cinq pixels n'a donc jamais rien espacé**, et la correction de la
     marge sous le nom, écrite de la même façon, n'a rien corrigé non plus.
     Cherché dans le CSS compilé après un premier essai infructueux ; c'est le
     seul endroit où ça se voit. */
  .blk > :global(* + *) {
    margin-top: 5px;
  }
  /* **Le nom du mod ne touche pas le champ qui suit.** Cinq pixels séparent
     bien deux champs entre eux — ils forment une liste — mais pas une identité
     d'un contrôle : le nom se lisait collé à la liste déroulante « Livrée »,
     comme s'il en était l'étiquette. L'écart marque la frontière entre ce
     qu'on a choisi et ce qu'on règle dessus. */
  .blk > :global(.pick + *) {
    margin-top: 12px;
  }
  /* Champ nommé : l'intitulé est une COLONNE, pas une ligne au-dessus. Coût
     en hauteur : zéro — la ligne reste à 30 px, là où un intitulé posé
     au-dessus aurait coûté 14 px par champ pour le même service. Mêmes
     valeurs que `ImageSelectDropdown` : ces lignes doivent s'aligner au pixel
     avec les siennes.
     `:global` parce que la grammaire appartient à la colonne — c'est elle qui
     mesure la colonne d'intitulé (`--sess-lblw`) — alors que les lignes sont
     posées par ses composants (`CarFields`, `PerformanceFold`). La portée
     reste bornée à `.session`. */
  .session :global(.field) {
    display: flex;
    align-items: center;
    /* 8 px, comme `.isd-trigger.labelled` du sélecteur — qui était resté à
       9 malgré ce commentaire, d'où 2 px d'écart sur le début de la valeur.
       Les deux lignes partagent la colonne d'intitulé et portent chacune une
       vignette de 28 px — un pixel d'écart ici décale la valeur de l'une par
       rapport à l'autre, ce qui se voit d'autant mieux qu'elles sont
       voisines. */
    gap: 8px;
    width: 100%;
    height: 30px;
    padding: 0 9px;
    background: var(--panel2);
    border: 1px solid var(--line);
    color: var(--txt2);
    font-size: 11px;
    text-align: left;
    cursor: pointer;
  }
  .session :global(.field:hover) {
    border-color: var(--faint2);
  }
  /* "You are here" (SPEC §7.2): the card of the screen on display, or the
     driver line on the driver screen, carries the left rule of an active rail
     entry — level 2 of the accent scale (§7.2ter), "what has the focus".
     Border colour plus a 1 px inset shadow, not a wider border: the rule is
     2 px of red, and nothing inside moves when it comes and goes. After the
     hover rule, which would otherwise grey the rule under the pointer. */
  .blk.here,
  .session :global(.field.here) {
    border-left-color: var(--rosso);
    box-shadow: inset 1px 0 0 var(--rosso);
  }
  .session :global(.field .k) {
    flex: 0 0 var(--sess-lblw, 60px);
    max-width: 88px;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
    font-size: 8.5px;
    letter-spacing: 0.16em;
    text-transform: uppercase;
    color: var(--muted);
  }
  .session :global(.field .unit) {
    flex: none;
    font-size: 9px;
    color: var(--muted);
  }
  .session :global(.field .v) {
    flex: 1;
    min-width: 0;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
    color: var(--txt);
  }
  /* Valeur par défaut : un possessif en italique grise, jamais une négation —
     même formulation que « Celle de la livrée » de l'écran Pilote. */
  .session :global(.field .v.stock) {
    color: var(--muted);
    font-style: italic;
  }
  /* Un réglage posé sur la voiture se voit sans lire la ligne, comme le lest
     dans son champ : rouge au sens du barème (§7.2ter) — un réglage qui change
     la course. */
  .session :global(.field .v.set) {
    color: var(--rosso-bright);
    font-size: 10px;
  }
  .session :global(.field .chev) {
    flex: none;
    color: var(--faint);
    font-size: 9px;
  }
  /* Hors flux et sans contrainte de largeur : sert uniquement à mesurer le
     plus long intitulé de la locale courante. `visibility: hidden` et non
     `display: none` — un élément non rendu n'a pas de largeur à lire. */
  .lbl-probe {
    position: absolute;
    visibility: hidden;
    pointer-events: none;
    font-size: 8.5px;
    letter-spacing: 0.16em;
    text-transform: uppercase;
  }
  .lbl-probe span {
    display: block;
    width: max-content;
    white-space: nowrap;
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
    /* Détaché de la liste des types : il suit désormais le dernier choix qu'on
       fait avant de partir, et deux pixels le faisaient lire comme une
       cinquième entrée de cette liste. */
    margin-top: 12px;
  }
  /* L'encadré vient de `.warnbox` (global) : jaune, parce que ce n'est pas une
     erreur mais une condition réparable d'un clic, et parce que le rouge de
     cette colonne appartient au lancement (§7.2ter). Ne reste ici que la mise
     en ligne et le bouton, que `.warnbox` ne connaît pas. */
  .guard {
    display: flex;
    align-items: center;
    gap: 9px;
    margin-bottom: 8px;
    font-size: 10.5px;
  }
  .guard-txt {
    flex: 1;
  }
  .guard-btn {
    background: transparent;
    border: 1px solid currentcolor;
    color: inherit;
    font-size: 9px;
    letter-spacing: 0.09em;
    text-transform: uppercase;
    padding: 5px 9px;
    flex: none;
  }
  .guard-btn:hover:not(:disabled) {
    background: rgb(241 216 60 / 12%);
  }
  .btn-launch:hover:not(:disabled) {
    background: var(--rosso-bright);
  }
  /* Garde son fond rouge à l'état désactivé, en opacité réduite (SPEC SESSION§1) : il
     reste la destination visible de l'écran, et le griser complètement
     effacerait le but à atteindre. */
  .btn-launch:disabled {
    opacity: 0.45;
    cursor: not-allowed;
  }
</style>
