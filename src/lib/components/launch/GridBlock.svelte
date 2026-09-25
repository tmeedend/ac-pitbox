<script lang="ts">
  // Le **plateau** : une ligne par adversaire, ses cellules éditables, et les
  // deux gestes qui le sauvegardent.
  //
  // **La sortie du générateur, pas son enfant.** Le plateau vivait dans un cadre
  // à l'intérieur du cadre « Adversaires ». Les deux sont pourtant deux objets —
  // c'est toute la conception du CIBLE§3.3 : le filtre définit le vivier, jamais le
  // plateau, et il faut un geste explicite pour passer de l'un à l'autre.
  //
  // Depuis le lot 5 les deux ont leur **page**, en pleine largeur, et plus aucun
  // cadre entre eux : le générateur en haut, le plateau dessous, dans un seul
  // enchaînement. La hauteur de la table est plafonnée à une dizaine de lignes
  // avec défilement interne (L5§5.3) — c'est le seul défilement imbriqué autorisé
  // dans l'app, et une table de données est précisément le composant pour lequel
  // cette convention existe : sans lui, la hauteur de la page serait fonction du
  // nombre d'IA, et le cas normal d'une course GT3 en aligne 24.
  //
  // Presentation only: generation and the livery cache live in
  // `$lib/launch/opponentGrid.svelte.ts`, which `Launch.svelte` also drives
  // from elsewhere (presets, saved sessions).
  import { formatRatio } from "$lib/detail/carSpecs";
  import type { CardIndex } from "$lib/library/filters";
  import {
    AI_LEVEL_MAX,
    AI_LEVEL_MIN,
    type Nationality,
    type Opponent,
    type RaceSetup,
    type SkinItem,
  } from "$lib/launch/launch";
  import { previewSrc, type ModCard } from "$lib/library/library";
  import { t } from "$lib/i18n/index.svelte";
  import AnchoredPopover from "$lib/components/filters/AnchoredPopover.svelte";

  let {
    setup,
    carPool,
    skinsByCarId,
    index,
    poolCount,
    nationalityList,
    duplicateDrivers,
    onchoose,
    onregenerate,
    onremove,
    onduplicate,
    onsetlevel,
    onsetcell,
    onsavegrid,
    onloadgrid,
    onopenpicker,
  }: {
    setup: RaceSetup;
    carPool: ModCard[];
    skinsByCarId: Record<string, SkinItem[]>;
    index: CardIndex;
    /** Le vivier, pour le libellé de « Choisir dans le vivier · N voitures » :
     * c'est le geste qui relie les deux blocs. */
    poolCount: number;
    /** Les nationalités que le jeu connaît, avec leur drapeau. */
    nationalityList: Nationality[];
    /** Deux pilotes sous la même identité (SETUP§1.9). Calculé par l'écran et non
     * ici depuis que l'alerte doit aussi remonter sur l'entrée de navigation
     * (L5§1.3) : une seule source, deux lecteurs. */
    duplicateDrivers: boolean;
    onchoose: () => void;
    onregenerate: () => void;
    onremove: (index: number) => void;
    onduplicate: (index: number) => void;
    onsetlevel: (index: number, level: number | null) => void;
    onsetcell: (index: number, patch: Partial<Opponent>) => void;
    onsavegrid: () => void;
    onloadgrid: () => void;
    onopenpicker: (index: number) => void;
  } = $props();

  // Bornes de la force d'une ligne : celles du curseur de Content Manager.
  const RANGE_MIN = AI_LEVEL_MIN;
  const RANGE_MAX = AI_LEVEL_MAX;

  // --- Colonnes : toutes, tout le temps -------------------------------------
  //
  // Le menu de colonnes est parti, et avec lui la préférence qu'il gardait. Il
  // existait parce que le plateau vivait dans une colonne d'écran : à 700 px,
  // huit colonnes ne tenaient pas, et il fallait choisir. La page dédiée a
  // supprimé la contrainte — la table a toute la largeur de l'écran —, donc
  // aussi la question. Un menu qui cache des colonnes dont on a la place est un
  // geste de plus pour un problème qui n'existe plus, et un réglage à retrouver
  // quand on cherche une valeur qui « a disparu ».
  //
  // Corollaire : plus de largeur à économiser, donc plus d'abréviations. `Nat.`
  // et `Str.` étaient les deux seules, et elles ne se lisaient que parce qu'on
  // savait déjà ce qu'elles disaient.

  /** kg/bhp d'un adversaire, `—` quand sa fiche est illisible — jamais estimé.
   * Lu dans l'index du vivier, donc analysé une fois par chargement de liste et
   * pas une fois par ligne rendue. */
  function opponentRatio(carId: string): string {
    const card = carPool.find((c) => c.id_interne === carId);
    return formatRatio(card ? index.ctx.ratioOf(card) : null);
  }

  /** La nationalité **n'est pas un champ libre** : le jeu en tient la liste, et
   * en affiche le drapeau. C'est ce que fait Content Manager, et c'est ce que
   * la cellule faisait passer pour du texte quelconque. */
  const flagByName = $derived(new Map(nationalityList.map((n) => [n.name.toLowerCase(), n.flag])));
  function flagOf(name: string | null | undefined): string | null {
    if (!name) return null;
    return previewSrc(flagByName.get(name.toLowerCase()) ?? null);
  }

  /** Une saisie vidée rend la cellule à `Auto` (§4.1). */
  const orAuto = (v: string): string | null => (v.trim() === "" ? null : v.trim());

  /** La livrée d'une ligne, quand elle est connue. */
  function skinOf(opp: Opponent): SkinItem | undefined {
    return opp.car_skin ? skinsByCarId[opp.car_id]?.find((sk) => sk.id === opp.car_skin) : undefined;
  }

  /** Ce qu'une cellule `Auto` vaut **réellement** : ce que la livrée déclare,
   * et donc ce que le jeu mettra. Afficher un « Auto » creux là où la donnée
   * existe cachait au lecteur le nom qui allait s'afficher en course — y
   * compris quand deux lignes portaient le même. */
  function autoDriver(opp: Opponent): string {
    const sk = skinOf(opp);
    const number = sk?.number ? `${sk.number} ` : "";
    return sk?.driver ? `${number}${sk.driver}` : t("launch.autoCell");
  }
  function autoNationality(opp: Opponent): string {
    return skinOf(opp)?.country ?? t("launch.autoCell");
  }

  /** Lest et bride : 0 à 100, jamais d'`Auto` — « rien » s'y dit par 0. */
  const clamp100 = (v: string): number => Math.max(0, Math.min(100, Math.round(Number(v) || 0)));

  function opponentName(carId: string): string {
    return carPool.find((c) => c.id_interne === carId)?.display_name ?? carId;
  }
  /** Vignette de l'adversaire : `preview.jpg` du skin d'abord, `livery.png`
   * seulement en dernier recours.
   *
   * C'est l'inverse de l'ordre du sélecteur de livrée de la barre latérale, et
   * l'inversion est **locale à ce composant** : les deux ne posent pas la même
   * question. Là-bas c'est « quelle peinture ? », et une pastille de couleur y
   * répond ; ici c'est « quelle voiture ? », et quatre pastilles de couleur n'y
   * répondent pas. La preview montre la voiture *portant* le skin, donc elle
   * répond aux deux à la fois — y compris pour deux adversaires « même
   * voiture », qui doivent rester distinguables. */
  function opponentPreview(opp: Opponent): string | null {
    const skin = opp.car_skin ? skinsByCarId[opp.car_id]?.find((s) => s.id === opp.car_skin) : null;
    const modPreview = carPool.find((c) => c.id_interne === opp.car_id)?.preview ?? null;
    return previewSrc(skin?.preview ?? modPreview ?? skin?.livery ?? null);
  }
  /** Voiture et livrée en toutes lettres, pour l'infobulle de la ligne. */
  function opponentFullName(opp: Opponent): string {
    const skin = opponentSkinName(opp);
    return skin ? `${opponentName(opp.car_id)} · ${skin}` : opponentName(opp.car_id);
  }

  // --- Choix de la livrée d'un adversaire -----------------------------------
  //
  // Il n'y en avait aucun : le `+` dupliquait une ligne avec une autre livrée,
  // `Regenerate` les retirait toutes au sort, mais rien ne permettait d'en
  // désigner une. La vignette est la cible naturelle — c'est l'image de la
  // livrée, donc l'endroit où l'on pense à la changer — et elle évite un
  // bouton de plus dans une ligne qui en porte déjà deux.
  let skinRow = $state<number | null>(null);
  let skinAnchor = $state<HTMLElement | null>(null);
  const skinChoices = $derived(skinRow == null ? [] : (skinsByCarId[setup.opponents[skinRow]?.car_id] ?? []));

  function openSkins(index: number, anchor: HTMLElement) {
    // Deuxième clic sur la même vignette : on referme, comme tout menu.
    if (skinRow === index) {
      closeSkins();
      return;
    }
    skinRow = index;
    skinAnchor = anchor;
  }
  function closeSkins() {
    skinRow = null;
    skinAnchor = null;
  }
  // --- Choix de la nationalité ---------------------------------------------
  //
  // **Un popover maison et non un `<select>`.** Un `<option>` natif ne peut pas
  // porter d'image — le menu déroulé est dessiné par le système, pas par la
  // webview —, donc le drapeau n'y apparaissait jamais alors que c'est lui qui
  // permet de reconnaître un pays d'un coup d'œil. Le projet avait déjà buté
  // dessus (`ImageSelectDropdown`).
  //
  // Ce qu'on perd, c'est la recherche à la frappe du système ; ce qu'on gagne,
  // c'est un vrai champ de recherche — et sur **220 pays** il vaut mieux, parce
  // qu'il cherche n'importe où dans le nom et pas seulement au début : « guinea »
  // rend les trois Guinées, ce qu'une frappe initiale ne sait pas faire.
  let natRow = $state<number | null>(null);
  let natAnchor = $state<HTMLElement | null>(null);
  let natQuery = $state("");
  const natMatches = $derived.by(() => {
    const q = natQuery.trim().toLowerCase();
    return q ? nationalityList.filter((n) => n.name.toLowerCase().includes(q)) : nationalityList;
  });

  function openNat(index: number, anchor: HTMLElement) {
    if (natRow === index) {
      closeNat();
      return;
    }
    natRow = index;
    natAnchor = anchor;
    // Vidée à chaque ouverture : une recherche qui survit d'une ligne à l'autre
    // fait croire à une liste courte.
    natQuery = "";
  }
  function closeNat() {
    natRow = null;
    natAnchor = null;
  }
  function pickNat(name: string | null) {
    if (natRow != null) onsetcell(natRow, { nationality: name });
    closeNat();
  }

  function pickSkin(id: string) {
    if (skinRow != null) onsetcell(skinRow, { car_skin: id });
    closeSkins();
  }

  function opponentSkinName(opp: Opponent): string | undefined {
    return opp.car_skin ? skinsByCarId[opp.car_id]?.find((s) => s.id === opp.car_skin)?.name : undefined;
  }
</script>

<section class="grid-blk">
  <!-- Un problème de configuration qui appelle une action, donc le jaune est à
       sa place ici — contrairement à l'explication de la force, passée au ⓘ. Il
       vit avec le plateau, qui est ce qui le produit. -->
  {#if duplicateDrivers}
    <p class="warnbox dup">{t("launch.duplicateDrivers")}</p>
  {/if}

  <div class="oppo">
    <!-- `Regenerate` is NOT `Fill` with another name: it keeps the cars and
         re-rolls what was drawn on them (skin, strength), where `Fill` draws
         the cars themselves. Two gestures one actually wants separately — the
         grid is right but the liveries repeat, versus the grid is wrong. -->
    <div class="oppo-h lbl">
      <span>{t("launch.gridHeader", { count: setup.opponents.length })}</span>
      <!-- La position de départ a rejoint SESSION OPTIONS (SETUP§2.5) : elle dépend
           du type de session, et tout ce qui en dépend vit là-bas. -->
      <span class="oppo-sp"></span>
      <button class="oppo-regen" type="button" disabled={!setup.opponents.length} onclick={onregenerate}
        >{t("launch.regenerateGrid")}</button
      >
    </div>

    <!-- La table, et elle seule, défile (L5§5.3). L'en-tête de colonnes y est
         collant : une ligne d'en-tête qui sort par le haut au bout de trois
         lignes ne sert à rien. -->
    <div class="oppo-rows">
      <!-- **Une seule police pour toute la rangée d'en-tête.** Les intitulés
           héritaient de la taille de LEUR colonne — 10,5 px pour le nom, 9 pour
           le kg/bhp, 8 pour les autres : trois tailles sur une même ligne, ce
           qui se voit avant même qu'on lise les mots. La taille est donc posée
           sur la rangée, une fois, et prime sur celles des colonnes.
           Les mots sont entiers : la page a la largeur, les abréviations
           n'économisaient plus rien. -->
      <div class="oppo-row oppo-th">
        <span class="oppo-img th-img"></span>
        <!-- Read-only header in the dimmer grey, editable ones in the lighter:
             two greys, no third level, and the row says what can be typed into
             before one tries. -->
        <span class="oppo-n lbl-key ro">{t("columns.name")}</span>
        <span class="oppo-driver lbl-key">{t("launch.colDriver")}</span>
        <span class="oppo-nat lbl-key">{t("launch.colNationality")}</span>
        <span class="oppo-ratio lbl-key ro">{t("launch.colRatio")}</span>
        <span class="oppo-force lbl-key">{t("launch.colStrength")}</span>
        <span class="oppo-bal lbl-key">{t("launch.colBallast")}</span>
        <span class="oppo-res lbl-key">{t("launch.colRestrictor")}</span>
        <span class="th-act"></span>
      </div>
    {#each setup.opponents as opp, i}
      {@const prev = opponentPreview(opp)}
      {@const shownName = opp.nationality ?? skinOf(opp)?.country ?? null}
      <div
        class="oppo-row"
        role="button"
        tabindex="0"
        title={t("launch.opponentEditTooltip")}
        onclick={() => onopenpicker(i)}
        onkeydown={(e) => (e.key === "Enter" || e.key === " ") && onopenpicker(i)}
      >
        <!-- La vignette ouvre le choix de **livrée** : c'est elle qu'elle
             montre, donc l'endroit où l'on pense à la changer. Le reste de la
             ligne ouvre le choix de **voiture** — deux questions, deux cibles,
             et aucun bouton de plus à caser dans la ligne. -->
        <button
          class="oppo-img"
          type="button"
          title={t("launch.pickSkinTooltip")}
          onclick={(e) => {
            e.stopPropagation();
            openSkins(i, e.currentTarget);
          }}>{#if prev}<img src={prev} alt="" />{:else}<span class="mono">🏎</span>{/if}</button
        >
        <!-- Le nom entier au survol. La colonne l'élide, et c'est précisément
             ce qu'on cherche à lire — l'aperçu en grand qui s'ouvrait ici
             recouvrait les lignes voisines pour montrer ce que la vignette
             montrait déjà. -->
        <span class="oppo-n" title={opponentFullName(opp)}
          >{opponentName(opp.car_id)}{#if opponentSkinName(opp)}<span class="oppo-skin"> · {opponentSkinName(opp)}</span
            >{/if}</span
        >
        <!-- Vide = `Auto` : le nom vient alors du `ui_skin.json` de la livrée,
             ce que le jeu fait déjà. Rien à voir avec les mods de tenue de
             pilote, qui sont une apparence 3D sur leur propre axe. -->
        <input
            class="oppo-driver cell"
            class:is-auto={opp.driver_name == null}
            type="text"
            placeholder={autoDriver(opp)}
            value={opp.driver_name ?? ""}
            onclick={(e) => e.stopPropagation()}
            onchange={(e) => onsetcell(i, { driver_name: orAuto(e.currentTarget.value) })}
        />
        <!-- **Le drapeau ET le nom.** Le drapeau seul tenait dans trente
             pixels, à l'époque où la colonne se disputait la largeur avec sept
             autres dans un rail d'écran ; le nom vivait en infobulle, c'est-à-
             dire à peu près nulle part. La page dédiée a la place, et un
             drapeau seul se reconnaît mal au-delà d'une dizaine de pays. Les
             plus longs s'élident, le drapeau reprenant alors la
             reconnaissance. -->
        <button
          class="oppo-nat natcell"
          type="button"
          title={shownName ?? t("launch.autoCell")}
          aria-label={t("launch.colNationality")}
          onclick={(e) => {
            e.stopPropagation();
            openNat(i, e.currentTarget);
          }}
        >
          {#if flagOf(shownName)}
            <img class="flag" src={flagOf(shownName)} alt="" />
          {:else}
            <span class="flag flag-none"></span>
          {/if}
          <span class="nat-name" class:is-auto={!shownName}>{shownName ?? t("launch.autoCell")}</span>
        </button>
        <span class="oppo-ratio mono">{opponentRatio(opp.car_id)}</span>
        <!-- `Auto` is not a value we draw: it is the absence of an override,
             and the game draws inside the global range. Hence a placeholder
             rather than a number — emptying the field is the gesture that puts
             the cell back to `Auto`, which is why no menu is needed here. -->
        <input
          class="oppo-force mono"
          class:is-auto={opp.ai_level == null}
          type="number"
          min={RANGE_MIN}
          max={RANGE_MAX}
          placeholder={t("launch.autoCell")}
          value={opp.ai_level ?? ""}
          title={t("launch.opponentLevelTooltip")}
          onclick={(e) => e.stopPropagation()}
          onchange={(e) => onsetlevel(i, e.currentTarget.value.trim() === "" ? null : Number(e.currentTarget.value))}
        />
        <input
          class="oppo-bal cell mono"
          class:is-zero={!opp.ballast}
          type="number"
          min="0"
          max="100"
          value={opp.ballast}
          onclick={(e) => e.stopPropagation()}
          onchange={(e) => onsetcell(i, { ballast: clamp100(e.currentTarget.value) })}
        />
        <input
          class="oppo-res cell mono"
          class:is-zero={!opp.restrictor}
          type="number"
          min="0"
          max="100"
          value={opp.restrictor}
          onclick={(e) => e.stopPropagation()}
          onchange={(e) => onsetcell(i, { restrictor: clamp100(e.currentTarget.value) })}
        />
        <button
          class="oppo-dup"
          type="button"
          title={t("launch.opponentDuplicateTooltip")}
          onclick={(e) => { e.stopPropagation(); onduplicate(i); }}
        >+</button>
        <button class="oppo-x" type="button" title={t("common.remove")} onclick={(e) => { e.stopPropagation(); onremove(i); }}>✕</button>
      </div>
    {/each}
    </div>
    <!-- The pool count in the label is the point: it says what the filter
         bought — this many rows to read instead of the whole library. -->
    <button class="oppo-add" type="button" disabled={poolCount === 0} onclick={onchoose}
      >{poolCount === 1 ? t("launch.chooseFromPoolOne") : t("launch.chooseFromPool", { count: poolCount })}</button
    >
    {#if natRow != null && natAnchor}
      <AnchoredPopover anchor={natAnchor} minWidth={244} onclose={closeNat}>
        <div class="natpop">
          <!-- `autofocus` : le popover s'ouvre pour chercher, et sans lui il
               faut un second clic avant de pouvoir taper. -->
          <!-- svelte-ignore a11y_autofocus -->
          <input class="input natq" type="text" autofocus placeholder={t("launch.searchCountry")} bind:value={natQuery} />
          <div class="natlist">
            <!-- `Auto` d'abord et toujours visible : c'est le seul choix qui ne
                 se cherche pas par son nom. -->
            <button class="nat" type="button" onclick={() => pickNat(null)}>
              <span class="flag flag-none"></span>
              <span class="nat-n">{natRow != null ? autoNationality(setup.opponents[natRow]) : ""}</span>
            </button>
            {#each natMatches as n (n.code)}
              <button
                class="nat"
                class:on={natRow != null && setup.opponents[natRow]?.nationality === n.name}
                type="button"
                onclick={() => pickNat(n.name)}
              >
                {#if previewSrc(n.flag)}<img class="flag" src={previewSrc(n.flag)} alt="" />{:else}
                  <span class="flag flag-none"></span>{/if}
                <span class="nat-n">{n.name}</span>
              </button>
            {/each}
            {#if !natMatches.length}
              <p class="nat-none">{t("common.noResults")}</p>
            {/if}
          </div>
        </div>
      </AnchoredPopover>
    {/if}

    {#if skinRow != null && skinAnchor}
      <AnchoredPopover anchor={skinAnchor} minWidth={228} onclose={closeSkins}>
        <div class="skins">
          {#if skinChoices.length}
            {#each skinChoices as sk (sk.id)}
              {@const img = previewSrc(sk.livery ?? sk.preview)}
              <button
                class="skin"
                class:on={setup.opponents[skinRow]?.car_skin === sk.id}
                type="button"
                onclick={() => pickSkin(sk.id)}
              >
                <span class="skin-img">{#if img}<img src={img} alt="" />{/if}</span>
                <span class="skin-n">{sk.name}</span>
              </button>
            {/each}
          {:else}
            <p class="skin-none">{t("launch.noSkinsForCar")}</p>
          {/if}
        </div>
      </AnchoredPopover>
    {/if}

    <!-- A grid is worth saving on its own, apart from the session that holds
         it: the same GT3 field on ten tracks (§5). -->
    <div class="oppo-foot">
      <button class="oppo-regen" type="button" disabled={!setup.opponents.length} onclick={onsavegrid}
        >{t("launch.saveGrid")}</button
      >
      <button class="oppo-regen" type="button" onclick={onloadgrid}>{t("launch.loadGrid")}</button>
    </div>
  </div>
</section>

<style>
  /* Le second des deux gris, et il n'y en a pas de troisième : une colonne en
     lecture seule s'annonce plus éteinte que celles qui se saisissent. */
  .oppo-th .ro {
    color: var(--faint);
  }
  /* Liste de livrées du popover : `livery.png` d'abord (le motif seul, lisible
     à 34 px) et la photo en repli — l'inverse de la vignette de ligne, qui
     répond à « quelle voiture ? » et non à « quelle peinture ? ». */
  .skins {
    display: flex;
    flex-direction: column;
    max-height: 320px;
    overflow-y: auto;
    padding: 4px;
  }
  .skin {
    display: flex;
    align-items: center;
    gap: 9px;
    padding: 4px 6px;
    background: transparent;
    border: 1px solid transparent;
    color: var(--txt2);
    font-size: 11px;
    text-align: left;
  }
  .skin:hover {
    background: var(--raised);
    color: var(--txt);
  }
  .skin.on {
    border-color: var(--rosso-border);
    background: var(--rosso-dim);
    color: var(--rosso-bright);
  }
  .skin-img {
    flex: none;
    width: 34px;
    height: 20px;
    border: 1px solid var(--line);
    background: var(--bg);
    overflow: hidden;
  }
  .skin-img img {
    width: 100%;
    height: 100%;
    object-fit: cover;
  }
  .skin-n {
    flex: 1;
    min-width: 0;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }
  .skin-none {
    padding: 8px 6px;
    font-size: 11px;
    color: var(--muted);
  }
  .dup {
    margin-bottom: 10px;
  }
  .oppo-foot {
    display: flex;
    gap: 7px;
    padding: 7px 10px;
    border-top: 1px solid var(--line);
  }
  /* Pushes the start position and `Regenerate` to the right of the title. */
  .oppo-sp {
    flex: 1;
  }
  /* Compteur d'adversaires, tirage et difficulté : tout sur une ligne (retombe
     seulement si la largeur manque). */
  .oppo {
    border: 1px solid var(--line);
  }
  /* Une dizaine de lignes, puis on défile — la hauteur de la page cesse ainsi
     d'être fonction du nombre d'IA (L5§5.3). Une ligne fait 58 px (vignette 45 +
     deux fois 6 de marge + le filet). */
  .oppo-rows {
    max-height: 580px;
    overflow-y: auto;
  }
  /* Collante : sans ça l'en-tête sort par le haut au bout de trois lignes, et
     les colonnes optionnelles redeviennent des nombres sans nom.
     Sélecteur descendant et non `.oppo-th` seul : la rangée porte AUSSI la
     classe `.oppo-row`, qui se pose en `relative` — à spécificité égale c'est
     l'ordre dans la feuille qui tranche, et il donnait « relative ». Constaté
     à l'écran, l'en-tête partait par le haut. */
  .oppo-rows .oppo-th {
    position: sticky;
    top: 0;
    z-index: 1;
  }
  /* Couleur/taille/interlettrage/majuscules viennent de `.lbl` (global,
     harmonisation §chantier libellés) : ne reste ici que le fond en bandeau
     et l'annulation de la marge basse (`.lbl` en prévoit une pour une
     rubrique de carte, pas pour un bandeau suivi directement des lignes). */
  .oppo-h {
    background: var(--raised);
    padding: 6px 10px;
    margin-bottom: 0;
    /* `.lbl` est déjà en flex : reste à écarter les deux bouts. L'espace du
       milieu attend la position de départ (L3). */
    justify-content: space-between;
    gap: 10px;
  }
  /* Bouton secondaire neutre : régénérer le plateau n'est pas ce qui est
     retenu pour la session, donc pas de rouge (§7.2ter). L'action existait
     déjà comme effet de bord d'un changement de voiture — sans aucun moyen de
     la demander. */
  .oppo-regen {
    background: transparent;
    border: 1px solid var(--line);
    color: var(--muted);
    font-size: 10px;
    letter-spacing: 0.1em;
    text-transform: uppercase;
    padding: 4px 10px;
  }
  .oppo-regen:hover:not(:disabled) {
    background: var(--panel2);
    color: var(--txt2);
  }
  .oppo-regen:disabled {
    color: var(--faint2);
    cursor: not-allowed;
  }
  /* `relative` porte la bulle de survol, et son absence ne se voit pas comme
     un défaut de style : un enfant `absolute` se cale sur le premier ancêtre
     positionné, ici le conteneur de défilement de tout l'écran — la bulle
     partait donc en bas de la page, hors champ, et le survol semblait n'avoir
     aucun effet. */
  .oppo-row {
    display: flex;
    align-items: center;
    gap: 9px;
    padding: 6px 10px;
    border-top: 1px solid var(--line);
    background: var(--panel2);
    cursor: pointer;
    position: relative;
  }
  .oppo-row:hover {
    background: var(--raised);
  }
  /* 16:9, le format de `preview.jpg`. La ligne ne grandit que de quelques
     pixels et la surface double : à 34x22 on voyait une couleur, pas une
     voiture. Recadrage plutôt que dézoom — le cadrage Kunos est constant et la
     plupart des mods le reprennent, la voiture est donc toujours au même
     endroit dans l'image. */
  /* Bouton et non plus `div` : elle ouvre le choix de livrée. Rien d'un bouton
     au repos — c'est une image, et le cadre du survol suffit à dire qu'elle se
     clique, comme les cellules de la ligne. */
  /* **Plus grande depuis que l'aperçu au survol a disparu** : elle est devenue
     la seule vue de la voiture, et le plateau en pleine largeur a la place. Le
     16:9 reste celui de `preview.jpg`. Chaque pixel de hauteur ici se paie une
     fois par ligne, donc ce n'est pas un réglage à pousser — 80×45 rallonge la
     ligne d'une quinzaine de pixels, pas du double. */
  .oppo-img {
    padding: 0;
    cursor: pointer;
    width: 80px;
    height: 45px;
    border: 1px solid var(--line);
    background: var(--bg);
    display: flex;
    align-items: center;
    justify-content: center;
    flex: none;
    overflow: hidden;
  }
  .oppo-img img {
    width: 100%;
    height: 100%;
    object-fit: cover;
    object-position: center 60%;
  }
  .oppo-row:hover .oppo-img {
    border-color: var(--faint2);
  }
  /* Plafonné : sans ça toute la largeur gagnée lui revient, et l'écart entre
     lui et le nom de pilote devient assez grand pour qu'on perde la ligne en la
     parcourant des yeux. La place restante va au vide en fin de ligne plutôt
     qu'à une colonne arbitraire. */
  .oppo-n {
    font-size: 12.5px;
    /* Plafonné plus haut qu'avant : la page dédiée a la largeur, et le vide de
       fin de ligne valait une centaine de pixels. Plafonné quand même — sans
       cap, toute la largeur gagnée va au nom et l'écart avec le nom de pilote
       devient assez grand pour qu'on perde la ligne en la parcourant. */
    flex: 0 1 560px;
    min-width: 0;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }
  .oppo-row::after {
    content: "";
    flex: 1;
  }
  .oppo-skin {
    color: var(--muted);
  }
  /* Toutes les cellules optionnelles ont une largeur FIXE et le nom prend ce
     qui reste : ajouter une colonne serre donc le nom. Aucune ne s'étire, sans
     quoi l'alignement d'une colonne à l'autre se perdrait d'une ligne à la
     suivante. */
  .oppo-ratio {
    width: 72px;
    flex: none;
    font-size: 11px;
    color: var(--muted);
    text-align: right;
  }
  .oppo-driver {
    width: 156px;
  }
  /* Le drapeau, l'écart, et de quoi écrire un nom de pays lisible ; les plus
     longs s'élident, le drapeau reprenant alors la reconnaissance. */
  .oppo-nat {
    width: 156px;
  }
  .nat-name {
    min-width: 0;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
    font-size: 12px;
    color: var(--txt);
  }
  /* `Auto` n'est pas un pays : éteint, comme les cellules `Auto` voisines. */
  .nat-name.is-auto {
    color: var(--faint);
  }
  /* Bouton, mais rien d'un bouton au repos : c'est le drapeau qu'on voit, et le
     cadre qui apparaît au survol de la ligne suffit à dire qu'il se clique —
     comme les cellules éditables voisines. */
  .natcell {
    display: flex;
    align-items: center;
    justify-content: flex-start;
    gap: 8px;
    background: transparent;
    border: 1px solid transparent;
    padding: 2px 3px;
    cursor: pointer;
    text-align: left;
  }
  .oppo-row:hover .natcell {
    background: var(--bg);
    border-color: var(--line);
  }
  /* 4:3 comme les PNG du jeu, et un filet : beaucoup de drapeaux ont du blanc
     sur un bord, qui se fondrait dans la ligne. */
  .flag {
    flex: none;
    width: 16px;
    height: 12px;
    object-fit: cover;
    border: 1px solid var(--line);
  }
  /* Transparent et posé sur toute la cellule : le menu natif du système reste,
     avec son clavier et sa recherche à la frappe, sous une cellule qui ne montre
     qu'un drapeau. `color-scheme: dark` est la seule prise sur le menu déroulé,
     rendu par le système — sans lui il s'ouvre en blanc, hors charte, comme le
     sélecteur de date de la météo. */
  .natpop {
    display: flex;
    flex-direction: column;
    padding: 6px;
    gap: 6px;
  }
  .natq {
    font-size: 11px;
  }
  .natlist {
    display: flex;
    flex-direction: column;
    max-height: 300px;
    overflow-y: auto;
  }
  .nat {
    display: flex;
    align-items: center;
    gap: 9px;
    padding: 4px 6px;
    background: transparent;
    border: 1px solid transparent;
    color: var(--txt2);
    font-size: 11px;
    text-align: left;
  }
  .nat:hover {
    background: var(--raised);
    color: var(--txt);
  }
  .nat.on {
    border-color: var(--rosso-border);
    background: var(--rosso-dim);
    color: var(--rosso-bright);
  }
  .nat-n {
    flex: 1;
    min-width: 0;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }
  .nat-none {
    padding: 8px 6px;
    font-size: 11px;
    color: var(--muted);
  }
  /* Place tenue quand la livrée ne déclare aucun pays : sans elle la colonne se
     replierait sur les lignes muettes et la grille danserait. */
  .flag-none {
    border-style: dashed;
  }
  .oppo-bal {
    width: 78px;
    text-align: right;
  }
  .oppo-res {
    width: 92px;
    text-align: right;
  }
  /* Les quatre champs de cellule partagent la même discrétion que la force :
     pas de cadre au repos, il apparaît au survol de la ligne — sans quoi
     chaque ligne devient un formulaire. */
  .cell {
    background: transparent;
    border: 1px solid transparent;
    color: var(--txt);
    font-size: 12px;
    padding: 3px 4px;
    flex: none;
    min-width: 0;
    appearance: textfield;
  }
  .oppo-row:hover .cell {
    background: var(--bg);
    border-color: var(--line);
  }
  .cell:focus {
    background: var(--bg);
    border-color: var(--line);
  }
  .cell::-webkit-outer-spin-button,
  .cell::-webkit-inner-spin-button {
    appearance: none;
    margin: 0;
  }
  /* `Auto` et 0 se lisent comme « non décidé » et « rien » : éteints tous les
     deux, mais le premier est un texte de substitution et le second une vraie
     valeur — d'où deux règles et non une. */
  .cell.is-auto::placeholder {
    color: var(--faint);
  }
  .cell.is-zero {
    color: var(--faint);
  }
  /* Rangée d'en-tête : une ligne du plateau sans ses gestes.
     **La taille est posée ici, sur la rangée**, et non colonne par colonne :
     les intitulés héritaient sinon de la taille de leur colonne — 10,5 px,
     9 px, 8 px sur une même ligne. Le sélecteur descendant prime sur les
     règles de colonne, qui gardent la leur pour les cellules. Couleur, casse
     et interlettrage viennent toujours de `.lbl-key` (global). */
  .oppo-th {
    cursor: default;
    background: var(--bg);
    padding-top: 5px;
    padding-bottom: 5px;
  }
  /* Les cellules saisissables ont un cadre et sa marge intérieure ; les
     intitulés, non. Sans ce rattrapage, chaque en-tête de colonne éditable est
     décalé de cinq pixels par rapport aux valeurs qu'il nomme — visible sur les
     colonnes alignées à droite, où les deux bords devraient coïncider. */
  .oppo-th .oppo-driver,
  .oppo-th .oppo-bal,
  .oppo-th .oppo-res {
    padding: 0 5px;
  }
  .oppo-th .oppo-nat {
    padding: 0 4px;
  }
  .oppo-th span {
    font-size: 10px;
    /* La couleur aussi : `.lbl-key` est GLOBALE, donc moins spécifique que les
       règles de colonne de ce fichier — « Force » ressortait en blanc parce
       que sa colonne se peint en `--txt` pour ses cellules. */
    color: var(--muted);
  }
  .oppo-th:hover {
    background: var(--bg);
  }
  /* **La même largeur que la vignette d'une ligne**, sinon toute la rangée
     d'en-tête est décalée de la différence — trente-deux pixels, assez pour que
     chaque intitulé désigne la colonne d'à côté. */
  .th-img {
    width: 80px;
    border: 0;
    background: transparent;
    height: auto;
  }
  .th-act {
    width: 52px;
    flex: none;
  }
  /* Blanche, et sans cadre au repos : le vert était la seule occurrence de
     cette couleur dans un contrôle, et un cadre permanent faisait de chaque
     ligne un formulaire. Le cadre apparaît au survol de la ligne — c'est là
     qu'il faut savoir que la valeur s'édite, pas avant. */
  .oppo-force {
    width: 74px;
    height: 24px;
    background: transparent;
    border: 1px solid transparent;
    color: var(--txt);
    font-size: 12px;
    text-align: center;
    flex: none;
    appearance: textfield;
  }
  /* Dimmed like any `Auto` cell — the placeholder is what shows, and it must
     read as "not decided", not as a value. */
  .oppo-force.is-auto::placeholder {
    color: var(--faint);
  }
  .oppo-row:hover .oppo-force {
    background: var(--bg);
    border-color: var(--line);
  }
  .oppo-force::-webkit-outer-spin-button,
  .oppo-force::-webkit-inner-spin-button {
    appearance: none;
    margin: 0;
  }
  /* Le focus reste jaune, celui de `global.css` : le rouge qu'il portait ici
     ne se rattache à aucun niveau du barème (§7.2ter). */
  .oppo-force:focus {
    background: var(--bg);
    border-color: var(--line);
  }
  .oppo-dup {
    background: transparent;
    color: var(--muted2);
    font-size: 15px;
    line-height: 1;
    padding: 2px 5px;
    flex: none;
  }
  .oppo-dup:hover {
    background: transparent;
    color: var(--txt);
  }
  .oppo-x {
    background: transparent;
    color: var(--muted2);
    font-size: 14px;
    padding: 2px 5px;
  }
  .oppo-x:hover {
    background: transparent;
    color: var(--rosso-bright);
  }
  /* Action secondaire : aucun niveau du barème ne couvre un libellé rouge
     (§7.2ter), et le rouge de cet écran doit rester au bouton de lancement. */
  .oppo-add {
    background: var(--panel2);
    padding: 9px 12px;
    border-top: 1px solid var(--line);
    color: var(--txt2);
    font-size: 11.5px;
    text-align: left;
    width: 100%;
  }
  .oppo-add:hover {
    background: var(--raised);
  }

</style>
