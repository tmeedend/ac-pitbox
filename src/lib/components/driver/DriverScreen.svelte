<script lang="ts">
  // Écran Pilote (docs/SPEC-ecran-pilote.md) : choisir le corps du pilote et
  // sa tenue en trois pièces.
  //
  // **L'asymétrie fondatrice structure l'écran** (§1.3) : le corps est un
  // modèle 3D que la physique de la voiture désigne — il ne se substitue que
  // dans l'aperçu — alors que le casque, la combinaison et les gants ne sont
  // que des images posées dessus, que la livrée choisit déjà et qu'on peut
  // donc choisir à sa place. D'où le corps au-dessus, séparé, et les trois
  // autres en dessous.
  //
  // Le geste central est le survol (§D2) : parcourir la galerie applique
  // chaque option sur le pilote affiché, cliquer l'adopte. Une texture à plat
  // ne dit rien du résultat — c'est tout le problème que cet écran résout.
  import { onMount } from "svelte";
  import { t } from "$lib/i18n/index.svelte";
  import { getModDetail, previewSrc } from "$lib/library";
  import { nav, requestSection } from "$lib/nav.svelte";
  import { listDriverBodies, listDriverChoices, type BodyOption, type DriverChoices } from "$lib/driver";
  import {
    driverOverride,
    resetDriverOutfit,
    setDriverBody,
    setDriverPiece,
    type DriverOverride,
  } from "$lib/driverOverride.svelte";
  import { getUiPrefs, setUiPref } from "$lib/uiPrefs.svelte";
  import { bodyThumb, requestBodyThumb } from "$lib/driverThumbs.svelte";
  import LoadingState from "../LoadingState.svelte";
  import TriCheck, { type TriState } from "../TriCheck.svelte";
  import DriverStage from "./DriverStage.svelte";
  import DriverOutfits from "./DriverOutfits.svelte";

  /** Les quatre pistes, dans l'ordre où elles s'empilent (§5.5). */
  type Lane = "body" | "helmet" | "suit" | "gloves";
  /** Les trois pièces de tenue — le corps n'en est pas une (§1.3). */
  type Piece = "helmet" | "suit" | "gloves";
  const OUTFIT_LANES: Piece[] = ["helmet", "suit", "gloves"];

  /** Ce que la case par défaut vaut : « celui de la livrée » côté tenue,
   * « celui de la voiture » côté corps. La chaîne vide plutôt qu'un `null`
   * parce qu'une case de galerie a toujours une identité, y compris celle-là :
   * le défaut est un choix parmi les autres, pas une case à cocher à part
   * (§6.5). */
  const DEFAULT_ID = "";

  const KEYS = {
    favorites: "pitbox.driver.favorites",
    recents: "pitbox.driver.recents",
    grouped: "pitbox.driver.grouped",
  } as const;

  const prefs: DriverOverride = $derived(driverOverride());

  let bodies = $state<BodyOption[]>([]);
  let choices = $state<DriverChoices | null>(null);
  let carClass = $state<string>("");
  let loading = $state(true);
  /** Piste active. **De session, pas globale** (§13) : on rouvre l'écran sur
   * le casque, qui est ce qu'on vient y changer neuf fois sur dix. */
  let lane = $state<Lane>("helmet");
  /** Ce qu'on essaie en ce moment, `null` au repos. Jamais persisté : le
   * survol est exploratoire par nature. */
  let trying = $state<string | null>(null);
  let query = $state("");
  let grouped = $state(true);
  /** Trois états comme les filtres de la bibliothèque : neutre, uniquement,
   * tout sauf. Une case booléenne ne sait dire que la moitié de ce qu'on veut. */
  let favState = $state<TriState>(0);
  let recentState = $state<TriState>(0);
  let favorites = $state<string[]>([]);
  let recents = $state<string[]>([]);
  /** Bannière d'invalidation : elle se referme au premier choix effectué ou
   * par son propre bouton de marche arrière (§10.2), pas toute seule. */
  let noticeSeen = $state(false);
  /** Voiture de course : l'écran reste accessible, jamais grisé sans un mot
   * (§11.2), et ce bouton ouvre le panneau quand même. */
  let raceAcknowledged = $state(false);

  const carIsRace = $derived(carClass.toLowerCase() === "race");
  const substituted = $derived(choices?.substituted ?? false);
  /** **Seul le tout premier chargement remplace la galerie.** Les suivants la
   * laissent en place et se contentent de l'estomper : changer de corps
   * recharge les trois listes, et démonter la grille pour l'occasion renvoyait
   * le défilement tout en haut — on perdait sa place à chaque clic (bug réel).
   * Même principe que le plateau, qui garde le corps précédent affiché. */
  const firstLoad = $derived(loading && choices == null);

  // --- Chargement ----------------------------------------------------------

  onMount(() => {
    void getUiPrefs(Object.values(KEYS)).then((read) => {
      favorites = parseList(read[KEYS.favorites]);
      recents = parseList(read[KEYS.recents]);
      grouped = read[KEYS.grouped] !== "0";
    });
    void listDriverBodies().then((list) => {
      bodies = list;
    });
  });

  function parseList(raw: string | null): string[] {
    if (!raw) return [];
    try {
      const parsed: unknown = JSON.parse(raw);
      return Array.isArray(parsed) ? parsed.filter((v): v is string => typeof v === "string") : [];
    } catch {
      return [];
    }
  }

  // Les listes dépendent du corps courant, pas de celui de la voiture : c'est
  // lui qui porte les noms de texture, donc lui qui décide de ce qui s'y pose
  // (§1.3). Recalculées à chaque changement de voiture ou de corps.
  $effect(() => {
    const carId = nav.sessionCar?.id ?? null;
    const body = prefs.body;
    if (!carId) {
      choices = null;
      carClass = "";
      loading = false;
      return;
    }
    let cancelled = false;
    loading = true;
    void Promise.all([listDriverChoices(carId, body), getModDetail(carId)])
      .then(([read, detail]) => {
        if (cancelled) return;
        choices = read;
        carClass = detail?.car_class ?? "";
      })
      .catch((e) => console.error("driver: listes indisponibles", e))
      .finally(() => {
        if (!cancelled) loading = false;
      });
    return () => {
      cancelled = true;
    };
  });

  // --- Les options de la piste active --------------------------------------

  interface Cell {
    /** Valeur telle que `skin.ini` l'écrit, ou nom de fichier du corps. */
    id: string;
    /** Ce qu'on affiche sous la case. Ne se traduit pas (§14). */
    label: string;
    thumb: string | null;
  }
  interface Group {
    key: string;
    /** Nom lisible, en tête de groupe (§6.4). */
    name: string;
    /** Identifiant de dossier, en mono à côté du nom. `null` pour un
     * regroupement qui n'en est pas un (l'époque, côté corps). */
    folder: string | null;
    cells: Cell[];
  }

  /** Noms complets des cinq casques de 1969 qui portent celui d'un pilote —
   * table statique, et le seul endroit du produit où le catalogue raconte
   * quelque chose (§6.4). Les seize autres entrées de cette famille sont des
   * couleurs, qui se lisent très bien telles quelles. */
  const HISTORIC: Record<string, string> = {
    amon: "Chris Amon",
    bandini: "Lorenzo Bandini",
    clark: "Jim Clark",
    hill: "Graham Hill",
    ickx: "Jacky Ickx",
  };

  /** La voiture pose le mannequin, donc deux voitures ne donnent pas la même
   * vignette du même corps : elle entre dans la clé, et dans celle de la
   * boucle `{#each}` pour que les cases se redemandent au changement. */
  const carKey = $derived(nav.sessionCar?.id ?? "");

  const options = $derived<Cell[]>(
    lane === "body"
      ? // La vignette d'un corps est un rendu 3D produit à la demande
        // (`driverThumbs`) : `null` tant qu'il n'est pas tombé, et la case
        // affiche son nom en attendant.
        bodies.map((b) => ({ id: b.id, label: b.id, thumb: bodyThumb(carKey + "|" + b.id) }))
      : (choices?.[plural(lane)] ?? []).map((o) => ({
          id: o.id,
          label: readable(o.id),
          thumb: previewSrc(o.thumbnail),
        })),
  );

  /**
   * Action « quand ça devient visible, une fois ».
   *
   * Les vignettes de corps se demandent au défilement et jamais au chargement
   * de la liste : il y en a 45, chacune coûte une conversion la première fois,
   * et personne ne regarde les quarante-cinq. La marge fait démarrer le rendu
   * un peu avant que la case n'arrive à l'écran, pour qu'elle soit peinte
   * quand elle y arrive.
   */
  function whenVisible(node: HTMLElement, onSeen: () => void) {
    const observer = new IntersectionObserver(
      (entries) => {
        if (entries.some((e) => e.isIntersecting)) {
          onSeen();
          observer.disconnect();
        }
      },
      { rootMargin: "240px" },
    );
    observer.observe(node);
    return { destroy: () => observer.disconnect() };
  }

  function plural(piece: Piece): "helmets" | "suits" | "gloves" {
    return piece === "helmet" ? "helmets" : piece === "suit" ? "suits" : "gloves";
  }

  /** Le dernier segment d'un identifiant, qui est ce qui distingue une option
   * de ses sœurs — le premier est déjà en tête de groupe. */
  function readable(id: string): string {
    const leaf = id.split("/").pop() ?? id;
    return HISTORIC[leaf.toLowerCase()] ?? leaf;
  }

  const filtered = $derived(
    options.filter((cell) => {
      const entry = tag(cell.id);
      if (favState !== 0 && favorites.includes(entry) !== (favState === 1)) return false;
      if (recentState !== 0 && recents.includes(entry) !== (recentState === 1)) return false;
      if (!query.trim()) return true;
      const needle = query.trim().toLowerCase();
      return cell.id.toLowerCase().includes(needle) || cell.label.toLowerCase().includes(needle);
    }),
  );

  const groups = $derived<Group[]>(buildGroups(filtered));

  function buildGroups(cells: Cell[]): Group[] {
    if (!grouped) return cells.length ? [{ key: "", name: "", folder: null, cells }] : [];
    const out: Group[] = [];
    for (const cell of cells) {
      const key = lane === "body" ? eraOf(cell.id) : (cell.id.split("/")[0] ?? cell.id);
      let group = out.find((g) => g.key === key);
      if (!group) {
        group =
          lane === "body"
            ? { key, name: t("driver.eraGroup." + key), folder: null, cells: [] }
            : { key, name: key, folder: key, cells: [] };
        out.push(group);
      }
      group.cells.push(cell);
    }
    // Un seul groupe ne regroupe rien : `HELMET_1969` porte ses vingt et un
    // casques à lui seul, et une bannière de groupe unique n'est qu'un filet
    // de plus à lire. Mesuré sur l'installation de référence — 1969 et 1985
    // sont dans ce cas, 1975 et les modernes non.
    return out.length > 1 ? out : cells.length ? [{ key: "", name: "", folder: null, cells }] : [];
  }

  function eraOf(bodyId: string): string {
    return bodies.find((b) => b.id === bodyId)?.era ?? "custom";
  }

  /** Clé de favori/récent : la piste et l'option, parce que `plain/red` existe
   * en combinaison comme en gants. */
  function tag(id: string): string {
    return lane + "|" + id;
  }

  // --- Ce qui est retenu, ce qui est essayé --------------------------------

  const kept = $derived(lane === "body" ? (prefs.body ?? DEFAULT_ID) : (prefs[lane] ?? DEFAULT_ID));
  const applied = $derived(trying ?? kept);
  const appliedCell = $derived(options.find((c) => c.id === applied) ?? null);
  const appliedLabel = $derived(appliedCell?.label ?? defaultLabel());

  /** Ce que le plateau doit échanger pendant un survol : la vignette d'AC, et
   * le nom de fichier de la texture qu'elle remplace — le même, à l'extension
   * près (`HELMET_2012.jpg` à côté de `HELMET_2012.dds`).
   *
   * `null` sur la piste Corps et sur la case par défaut : un corps n'est pas
   * une texture, il demande une vraie conversion (§9.2), et le défaut se
   * rétablit en retirant l'essai plutôt qu'en en posant un autre. */
  const trialTexture = $derived.by(() => {
    if (lane === "body" || trying == null || trying === DEFAULT_ID) return null;
    const thumb = options.find((c) => c.id === trying)?.thumb;
    if (!thumb) return null;
    const name = decodeURIComponent(thumb.split("/").pop() ?? "");
    return name ? { url: thumb, texture: name } : null;
  });

  function defaultLabel(): string {
    if (lane === "body") return choices?.model ?? t("driver.defaultBody");
    return t((substituted ? "driver.none." : "driver.fromLivery.") + lane);
  }

  /** Ce que la case par défaut annonce (§6.5). Elle n'est pas une case à
   * cocher à part : elle se survole et s'adopte comme les autres, et en mode
   * substitué elle retire simplement la pièce au lieu de rendre la main à la
   * livrée — qui n'a plus de destinataire. */
  function defaultCellText(): string {
    if (lane === "body") return t("driver.defaultCell.body");
    return t(substituted ? "driver.defaultCell.none" : "driver.defaultCell.livery");
  }

  function defaultCellName(): string {
    return substituted && lane !== "body" ? t("driver.defaultCell.noneName") : t("driver.defaultCell.name");
  }

  function adopt(id: string) {
    if (lane === "body") {
      setDriverBody(id || null);
      noticeSeen = false;
    } else {
      setDriverPiece(lane, id || null);
      noticeSeen = true;
    }
    remember(tag(id));
  }

  /** Douze derniers essais **adoptés**, pas survolés (§8.4) : le survol est
   * exploratoire par nature et polluerait l'historique. */
  function remember(entry: string) {
    recents = [entry, ...recents.filter((r) => r !== entry)].slice(0, 12);
    setUiPref(KEYS.recents, JSON.stringify(recents));
  }

  function toggleFavorite(id: string) {
    const entry = tag(id);
    favorites = favorites.includes(entry) ? favorites.filter((f) => f !== entry) : [...favorites, entry];
    setUiPref(KEYS.favorites, JSON.stringify(favorites));
  }

  function setGrouped(value: boolean) {
    grouped = value;
    setUiPref(KEYS.grouped, value ? "1" : "0");
  }

  /** Sortie unique (§5.6) : en mode substitué, la livrée n'est pas une
   * destination atteignable sans d'abord rétablir le corps. */
  function exit() {
    if (substituted) {
      setDriverBody(null);
      noticeSeen = false;
    } else {
      resetDriverOutfit();
    }
  }

  // --- Barre d'outils ------------------------------------------------------

  /** Ce que le premier niveau d'un identifiant désigne, donc ce que la
   * bascule de regroupement peut annoncer (§D7, §6.3). Table indexée sur le
   * préfixe, **repli silencieux** sur « famille » pour un pack de mod dont le
   * premier niveau ne veut rien dire de connu. */
  const AXES: [string, string][] = [
    ["helmet_base", "colour"],
    ["helmet_1975", "colour"],
    ["helmet_1985", "colour"],
    ["helmet_1969", "pilot"],
  ];

  const axis = $derived.by(() => {
    // Côté corps, l'axe n'est pas une famille de dossiers mais l'époque des
    // casques que le corps accepte — ce que le libellé doit dire, sans quoi
    // « grouper par époque » ne veut rien dire appliqué à un mannequin.
    if (lane === "body") return "era";
    const first = options[0]?.id.split("/")[0]?.toLowerCase() ?? "";
    return AXES.find(([prefix]) => first.startsWith(prefix))?.[1] ?? "family";
  });

  /** Compteur : le nombre, et la cause du filtrage — jamais le filtre seul
   * (§8.3). Il porte implicitement l'avertissement que changer de corps
   * changera ce nombre. */
  const countLabel = $derived(
    options.length
      ? t("driver.count." + lane, { count: String(options.length) })
      : t("driver.countEmpty." + lane),
  );
  const causeLabel = $derived(
    choices ? t("driver.cause", { body: choices.model, era: t("driver.era." + (choices.era ?? "custom")) }) : "",
  );
  /** Le nom lisible du corps courant, pour le bandeau du panneau. */
  const bodyLabel = $derived(choices?.model ?? "—");

  /** Ce qui tombe quand on substitue le corps (§10.2) — et rien que ça : une
   * puce dont l'objet n'a pas réellement été perdu ne s'affiche pas. */
  const noticeItems = $derived(
    [
      choices?.helmets.length === 0 ? t("driver.notice.helmet") : "",
      t("driver.notice.outfit"),
      t("driver.notice.race"),
    ].filter(Boolean),
  );
  const showNotice = $derived(substituted && !noticeSeen);
</script>

<div class="screen">
  <header class="head">
    <h2 class="lbl-screen">{t("driver.title")}</h2>
    <p class="sub">{t("driver.subtitle")}</p>
  </header>

  {#if !nav.sessionCar}
    <div class="empty">
      <p>{t("driver.noCar.title")}</p>
      <p class="hint">{t("driver.noCar.body")}</p>
      <button class="btn" type="button" onclick={() => requestSection("cars")}>{t("driver.noCar.pick")}</button>
    </div>
  {:else if carIsRace && !raceAcknowledged}
    <div class="empty">
      <p>{t("driver.race.title")}</p>
      <p class="hint">{t("driver.race.body")}</p>
      <button class="btn" type="button" onclick={() => (raceAcknowledged = true)}>{t("driver.race.anyway")}</button>
    </div>
  {:else}
    <div class="toolbar">
      <input class="input search" placeholder={t("driver.searchPlaceholder")} bind:value={query} />
      <span class="seg-lbl lbl-key mono">{t("driver.display")}</span>
      <div class="seg">
        <button class:on={grouped} type="button" onclick={() => setGrouped(true)}>{t("driver.groupBy." + axis)}</button>
        <button class:on={!grouped} type="button" onclick={() => setGrouped(false)}>{t("driver.groupAll")}</button>
      </div>
      <TriCheck
        label={t("driver.favorites")}
        bind:value={favState}
        titleInclude={t("driver.favOnly")}
        titleExclude={t("driver.favExcluded")}
        titleNeutral={t("driver.favNeutral")}
      />
      <TriCheck
        label={t("driver.recents")}
        bind:value={recentState}
        titleInclude={t("driver.recentOnly")}
        titleExclude={t("driver.recentExcluded")}
        titleNeutral={t("driver.recentNeutral")}
      />
      <div class="spacer"></div>
      <span class="count mono">{countLabel} · {causeLabel}</span>
    </div>

    <div class="body">
      <section class="blk fitting">
        <div class="blk-h">
          <span class="blk-t">{t("driver.fittingTitle")}</span>
          <span class="blk-n">{bodyLabel}</span>
        </div>

        <DriverStage
          carId={nav.sessionCar.id}
          skinId={nav.sessionCar.skin}
          outfit={{ model: prefs.body, suit: prefs.suit, gloves: prefs.gloves, helmet: prefs.helmet }}
          {lane}
          trial={trialTexture}
          applied={appliedLabel}
          sample={appliedCell?.thumb ?? null}
          trying={trying != null}
          {substituted}
        />

        <div class="blk-b lanes">
          <div class="blk-sub">{t("driver.laneGroup.commands")}</div>
          <button class="lane" class:on={lane === "body"} type="button" onclick={() => (lane = "body")}>
            <span class="lbl-key">{t("driver.lane.body")}</span>
            <span class="v mono">{bodyLabel}</span>
            <span class="n mono">{substituted ? t("driver.bodySubstituted") : t("driver.bodyFromCar")}</span>
          </button>

          <div class="blk-sub follows">{t("driver.laneGroup.follows")}</div>
          {#each OUTFIT_LANES as piece (piece)}
            <button class="lane" class:on={lane === piece} type="button" onclick={() => (lane = piece)}>
              <span class="lbl-key">{t("driver.lane." + piece)}</span>
              {#if prefs[piece]}
                <span class="v mono">{readable(prefs[piece])}</span>
              {:else}
                <span class="v def">{t((substituted ? "driver.none." : "driver.fromLivery.") + piece)}</span>
              {/if}
              <span class="n mono">{choices?.[plural(piece)].length ?? 0}</span>
            </button>
          {/each}

          <button class="btn btn-ghost reset" type="button" onclick={exit}>
            {substituted ? t("driver.reset.body") : t("driver.reset.livery")}
          </button>

          <DriverOutfits />
        </div>
      </section>

      <section class="gallery" class:busy={loading && !firstLoad}>
        {#if showNotice}
          <div class="warnbox notice">
            <div>
              <div class="notice-t">{t("driver.notice.title")}</div>
              <ul>
                {#each noticeItems as item (item)}<li>{item}</li>{/each}
              </ul>
            </div>
            <div class="acts">
              <button class="btn" type="button" onclick={() => ((lane = "helmet"), (noticeSeen = true))}>
                {t("driver.notice.choose")}
              </button>
              <button class="btn" type="button" onclick={exit}>{t("driver.notice.revert")}</button>
            </div>
          </div>
        {/if}

        {#if firstLoad}
          <LoadingState />
        {:else if lane === "helmet" && (choices?.helmets.length ?? 0) === 0}
          <div class="empty">
            <p>{t("driver.ownHelmet.title")}</p>
            <p class="hint">{t("driver.ownHelmet.body", { body: choices?.model ?? "" })}</p>
            <div class="row-acts">
              <button class="btn" type="button" onclick={() => (lane = "suit")}>{t("driver.ownHelmet.toSuit")}</button>
              <button class="btn" type="button" onclick={() => (lane = "body")}>{t("driver.ownHelmet.toBody")}</button>
            </div>
          </div>
        {:else if !groups.length}
          <p class="noresult">
            {t("driver.noResults", { query })}
            <button class="btn btn-ghost" type="button" onclick={() => (query = "")}>{t("driver.clearSearch")}</button>
          </p>
        {:else}
          {#each groups as group, gi (group.key)}
            {#if group.name}
              <div class="grp">
                <span class="sec-t">{group.name}</span>
                {#if group.folder}<span class="grp-id mono">{group.folder}</span>{/if}
                <span class="grp-n mono">{group.cells.length}</span>
              </div>
            {/if}
            <div class="grid">
              {#if gi === 0}
                <!-- Le défaut est un choix parmi les autres, en première
                     position du premier groupe, toujours (§6.5). -->
                <button
                  class="cell special"
                  class:sel={kept === DEFAULT_ID}
                  type="button"
                  onmouseenter={() => (trying = DEFAULT_ID)}
                  onmouseleave={() => (trying = null)}
                  onfocus={() => (trying = DEFAULT_ID)}
                  onblur={() => (trying = null)}
                  onclick={() => adopt(DEFAULT_ID)}
                >
                  <span class="art">{defaultCellText()}</span>
                  <span class="nm-row"><span class="nm">{defaultCellName()}</span></span>
                </button>
              {/if}
              {#each group.cells as cell (cell.id + "|" + carKey)}
                <button
                  class="cell"
                  class:sel={kept === cell.id}
                  type="button"
                  use:whenVisible={() =>
                    lane === "body" && requestBodyThumb(carKey, nav.sessionCar?.skin ?? null, cell.id)}
                  onmouseenter={() => (trying = cell.id)}
                  onmouseleave={() => (trying = null)}
                  onfocus={() => (trying = cell.id)}
                  onblur={() => (trying = null)}
                  onclick={() => adopt(cell.id)}
                  title={cell.id}
                >
                  <span class="art" class:rendering={lane === "body" && !cell.thumb}>
                    {#if cell.thumb}<img src={cell.thumb} alt="" />{:else}<span class="noart">{cell.label}</span>{/if}
                  </span>
                  <span class="nm-row">
                    <span class="nm mono">{cell.label}</span>
                    <span
                      class="fav"
                      class:on={favorites.includes(tag(cell.id))}
                      role="button"
                      tabindex="-1"
                      title={t("common.favorite")}
                      onclick={(e) => (e.stopPropagation(), toggleFavorite(cell.id))}
                      onkeydown={(e) => e.key === "Enter" && (e.stopPropagation(), toggleFavorite(cell.id))}
                    >{favorites.includes(tag(cell.id)) ? "♥" : "♡"}</span>
                  </span>
                </button>
              {/each}
            </div>
          {/each}
        {/if}
      </section>
    </div>
  {/if}
</div>

<style>
  /* Mise en page et couleurs reprises du design system (`global.css`) : même
     en-tête d'écran que les Add-ons, même barre d'outils que la vue
     transversale, mêmes cartes `.blk` que partout ailleurs. Ne restent ici que
     les deux choses que cet écran est seul à avoir : la colonne d'essayage
     fixe, et la grille d'échantillons. */
  .screen {
    display: flex;
    flex-direction: column;
    height: 100%;
    min-height: 0;
  }

  .head {
    margin-bottom: 18px;
  }
  .sub {
    color: var(--muted);
    font-size: 12px;
    margin-top: 6px;
    line-height: 1.5;
    max-width: 540px;
  }

  .toolbar {
    display: flex;
    align-items: center;
    gap: 12px;
    margin-bottom: 14px;
    flex-wrap: wrap;
  }
  .search {
    width: 200px;
    flex: none;
  }
  .seg {
    display: flex;
    border: 1px solid var(--line);
  }
  .seg button {
    background: var(--panel2);
    color: var(--muted);
    padding: 6px 14px;
    font-size: 11px;
    border-right: 1px solid var(--line);
  }
  .seg button:last-child {
    border-right: none;
  }
  .seg button.on {
    background: var(--rosso);
    color: #fff;
  }
  /* Couleur/taille/interlettrage viennent de `.lbl-key` : ne restent ici que
     les majuscules, que la classe globale ne couvre pas. */
  .seg-lbl {
    text-transform: uppercase;
  }
  .spacer {
    flex: 1;
  }
  .count {
    color: var(--faint);
    font-size: 11px;
    text-align: right;
    max-width: 320px;
  }

  /* --- corps de l'écran --- */
  .body {
    display: flex;
    gap: 14px;
    flex: 1;
    min-height: 0;
  }
  /* Le panneau d'essayage est fixe, seule la galerie défile (§4) : le pilote
     ne quitte jamais le champ de vision. */
  .fitting {
    width: 392px;
    flex: 0 0 392px;
    display: flex;
    flex-direction: column;
    min-height: 0;
    margin-bottom: 0;
    overflow: auto;
  }

  /* --- les pistes (§5.5) --- */
  .lanes {
    border-top: 1px solid var(--line);
  }
  .follows {
    margin-top: 14px;
  }
  .lane {
    display: grid;
    grid-template-columns: 92px 1fr auto;
    align-items: center;
    gap: 10px;
    width: 100%;
    padding: 8px 10px;
    border: 1px solid transparent;
    border-left: 2px solid transparent;
    background: transparent;
    text-align: left;
  }
  .lane + .lane {
    margin-top: 2px;
  }
  .lane:hover {
    background: var(--raised);
  }
  /* Bordure gauche en accent : un des trois seuls emplois du rouge saturé sur
     cet écran (§15). */
  .lane.on {
    background: var(--panel);
    border-color: var(--line);
    border-left-color: var(--rosso);
  }
  .lane .v {
    font-size: 12px;
    color: var(--txt2);
    white-space: nowrap;
    overflow: hidden;
    text-overflow: ellipsis;
  }
  .lane .v.def {
    color: var(--faint);
    font-style: italic;
  }
  .lane .n {
    font-size: 11px;
    color: var(--faint);
  }
  .reset {
    width: 100%;
    justify-content: center;
    margin-top: 12px;
    font-size: 11px;
  }

  /* --- galerie (§6, §7) --- */
  .gallery {
    flex: 1;
    min-width: 0;
    overflow: auto;
    scrollbar-gutter: stable;
    padding-right: 4px;
  }
  /* Rechargement en cours : la galerie s'estompe mais **reste en place**,
     défilement compris. */
  .gallery.busy {
    opacity: 0.55;
  }

  .notice {
    display: flex;
    gap: 12px;
    align-items: flex-start;
    margin-bottom: 14px;
  }
  .notice-t {
    font-size: 12px;
  }
  .notice ul {
    margin: 6px 0 0;
    padding-left: 16px;
    font-size: 11.5px;
    line-height: 1.6;
    opacity: 0.85;
  }
  .acts {
    margin-left: auto;
    display: flex;
    gap: 8px;
    flex: 0 0 auto;
  }

  .grp {
    display: flex;
    align-items: baseline;
    gap: 10px;
    margin: 20px 0 9px;
  }
  .grp:first-child {
    margin-top: 0;
  }
  .grp-id,
  .grp-n {
    font-size: 10px;
    color: var(--faint);
  }
  .grp::after {
    content: "";
    flex: 1;
    height: 1px;
    background: var(--line);
    align-self: center;
  }

  .grid {
    display: grid;
    grid-template-columns: repeat(auto-fill, minmax(104px, 1fr));
    gap: 9px;
  }
  .cell {
    display: block;
    /* Ne pas s'étirer à la hauteur de la rangée : une case est carrée, et une
       case étirée déforme tout ce qu'elle contient. */
    align-self: start;
    min-width: 0;
    border: 0;
    padding: 0;
    background: transparent;
    text-align: left;
  }
  /* Carré plein, filet franc, aucune ombre : l'échantillon doit être
     lisiblement une image et non un aperçu du résultat (§7.3). */
  .cell .art {
    display: block;
    aspect-ratio: 1;
    border: 1px solid var(--line);
    overflow: hidden;
    background: var(--panel2);
    filter: saturate(0.86) brightness(0.94);
    transition: filter 0.12s, border-color 0.12s;
  }
  .cell .art img {
    display: block;
    width: 100%;
    height: 100%;
    object-fit: cover;
  }
  /* Le remplissage n'appartient qu'au texte qui est *dans* la case : lui
     donner une hauteur alors qu'elle a déjà un rapport de forme la faisait
     déborder sur ses voisines (bug réel). */
  .noart,
  .cell.special .art {
    display: flex;
    align-items: center;
    justify-content: center;
    text-align: center;
    padding: 8px;
    font-size: 10px;
    color: var(--faint);
    line-height: 1.4;
  }
  .noart {
    height: 100%;
  }
  /* Vignette de corps pas encore rendue : la case porte son nom sur une trame
     discrète, et se remplit quand le rendu tombe. Pas d'animation d'attente —
     quarante-cinq cases qui pulsent ensemble font une galerie illisible. */
  .cell .art.rendering {
    background: var(--card);
  }
  .cell.special .art {
    background: repeating-linear-gradient(
      135deg,
      var(--panel2),
      var(--panel2) 5px,
      var(--card) 5px,
      var(--card) 10px
    );
    filter: none;
  }
  .nm-row {
    display: flex;
    align-items: center;
    gap: 6px;
    margin-top: 6px;
  }
  .nm {
    flex: 1;
    min-width: 0;
    font-size: 10px;
    color: var(--faint);
    white-space: nowrap;
    overflow: hidden;
    text-overflow: ellipsis;
  }
  .cell:hover .art,
  .cell:focus-visible .art {
    filter: none;
    border-color: var(--faint);
  }
  .cell:hover .nm,
  .cell:focus-visible .nm {
    color: var(--muted);
  }
  .cell.sel .art {
    filter: none;
    border-color: var(--rosso);
    box-shadow: inset 0 0 0 1px var(--rosso);
  }
  .cell.sel .nm {
    color: var(--txt2);
  }
  /* Le cœur de la bibliothèque, **sous** l'image et non dessus. La spec écarte
     le bouton flottant parce que l'échantillon est montré entier, sans
     découpe : c'est la texture elle-même qu'on juge, et un bouton posé dans un
     coin en cache un morceau. On garde son argument et on prend le glyphe du
     reste de l'app, pour que « mettre en favori » soit partout le même geste
     sur la même icône. */
  .fav {
    flex: 0 0 auto;
    font-size: 12px;
    line-height: 1;
    color: var(--faint);
  }
  .fav.on,
  .fav:hover {
    color: var(--rosso-bright);
  }

  .empty {
    color: var(--muted);
    text-align: center;
    padding: 50px 0;
  }
  .empty .hint {
    font-size: 12px;
    color: var(--faint);
    margin: 8px auto 16px;
    max-width: 420px;
    line-height: 1.6;
  }
  .row-acts {
    display: flex;
    gap: 8px;
    justify-content: center;
  }
  .noresult {
    font-size: 12px;
    color: var(--muted);
    margin: 18px 0 0;
  }
</style>
