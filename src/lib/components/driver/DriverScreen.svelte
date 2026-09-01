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
  import LoadingState from "../LoadingState.svelte";
  import DriverStage from "./DriverStage.svelte";

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
  let onlyFavorites = $state(false);
  let onlyRecents = $state(false);
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

  const options = $derived<Cell[]>(
    lane === "body"
      ? bodies.map((b) => ({ id: b.id, label: b.id, thumb: null }))
      : (choices?.[plural(lane)] ?? []).map((o) => ({
          id: o.id,
          label: readable(o.id),
          thumb: previewSrc(o.thumbnail),
        })),
  );

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
      if (onlyFavorites && !favorites.includes(tag(cell.id))) return false;
      if (onlyRecents && !recents.includes(tag(cell.id))) return false;
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
            ? { key, name: t("driver.era." + key), folder: null, cells: [] }
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
  <div class="head">
    <span class="lbl-screen">{t("driver.title")}</span>
    <span class="sub">{t("driver.subtitle")}</span>
  </div>

  {#if !nav.sessionCar}
    <div class="empty">
      <div class="h">{t("driver.noCar.title")}</div>
      <p class="p">{t("driver.noCar.body")}</p>
      <button class="btn" type="button" onclick={() => requestSection("cars")}>{t("driver.noCar.pick")}</button>
    </div>
  {:else if carIsRace && !raceAcknowledged}
    <div class="empty">
      <div class="h">{t("driver.race.title")}</div>
      <p class="p">{t("driver.race.body")}</p>
      <button class="btn" type="button" onclick={() => (raceAcknowledged = true)}>{t("driver.race.anyway")}</button>
    </div>
  {:else}
    <div class="toolbar">
      <label class="field">
        <span class="lbl-key">{t("driver.search")}</span>
        <input class="input" type="search" bind:value={query} placeholder={t("driver.searchPlaceholder")} />
      </label>
      <div class="field">
        <span class="lbl-key">{t("driver.display")}</span>
        <div class="seg">
          <button class="sg" class:on={grouped} type="button" onclick={() => setGrouped(true)}>
            {t("driver.groupBy." + axis)}
          </button>
          <button class="sg" class:on={!grouped} type="button" onclick={() => setGrouped(false)}>
            {t("driver.groupAll")}
          </button>
        </div>
      </div>
      <label class="chk">
        <input type="checkbox" bind:checked={onlyFavorites} />
        <span>{t("driver.favorites")}</span>
      </label>
      <label class="chk">
        <input type="checkbox" bind:checked={onlyRecents} />
        <span>{t("driver.recents")}</span>
      </label>
      <div class="count">
        <b>{countLabel}</b>
        <span>{causeLabel}</span>
      </div>
    </div>

    <div class="body">
      <section class="fitting">
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

        <div class="lanes">
          <div class="lane-sep">{t("driver.laneGroup.commands")}</div>
          <button class="lane body-lane" class:on={lane === "body"} type="button" onclick={() => (lane = "body")}>
            <span class="k">{t("driver.lane.body")}</span>
            <span class="v mono">{choices?.model ?? "—"}</span>
            <span class="n">{substituted ? t("driver.bodySubstituted") : t("driver.bodyFromCar")}</span>
          </button>

          <div class="lane-sep">{t("driver.laneGroup.follows")}</div>
          {#each OUTFIT_LANES as piece (piece)}
            <button class="lane" class:on={lane === piece} type="button" onclick={() => (lane = piece)}>
              <span class="k">{t("driver.lane." + piece)}</span>
              {#if prefs[piece]}
                <span class="v mono">{readable(prefs[piece])}</span>
              {:else}
                <span class="v def">{t((substituted ? "driver.none." : "driver.fromLivery.") + piece)}</span>
              {/if}
              <span class="n">{choices?.[plural(piece)].length ?? 0}</span>
            </button>
          {/each}

          <button class="reset" type="button" onclick={exit}>
            {substituted ? t("driver.reset.body") : t("driver.reset.livery")}
          </button>
        </div>
      </section>

      <section class="gallery">
        {#if showNotice}
          <div class="notice">
            <div class="notice-txt">
              <div class="t">{t("driver.notice.title")}</div>
              <ul>
                {#each noticeItems as item (item)}<li>{item}</li>{/each}
              </ul>
            </div>
            <div class="acts">
              <button class="mini on" type="button" onclick={() => ((lane = "helmet"), (noticeSeen = true))}>
                {t("driver.notice.choose")}
              </button>
              <button class="mini" type="button" onclick={exit}>{t("driver.notice.revert")}</button>
            </div>
          </div>
        {/if}

        {#if loading}
          <LoadingState />
        {:else if lane === "helmet" && (choices?.helmets.length ?? 0) === 0}
          <div class="empty">
            <div class="h">{t("driver.ownHelmet.title")}</div>
            <p class="p">{t("driver.ownHelmet.body", { body: choices?.model ?? "" })}</p>
            <div class="row-acts">
              <button class="btn" type="button" onclick={() => (lane = "suit")}>{t("driver.ownHelmet.toSuit")}</button>
              <button class="btn" type="button" onclick={() => (lane = "body")}>{t("driver.ownHelmet.toBody")}</button>
            </div>
          </div>
        {:else if !groups.length}
          <p class="noresult">
            {t("driver.noResults", { query })}
            <button class="link" type="button" onclick={() => (query = "")}>{t("driver.clearSearch")}</button>
          </p>
        {:else}
          {#each groups as group, gi (group.key)}
            {#if group.name}
              <div class="grp">
                <span>{group.name}</span>
                {#if group.folder}<span class="id mono">{group.folder}</span>{/if}
                <span class="n">{group.cells.length}</span>
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
                  <span class="nm">{defaultCellName()}</span>
                </button>
              {/if}
              {#each group.cells as cell (cell.id)}
                <button
                  class="cell"
                  class:sel={kept === cell.id}
                  class:fav={favorites.includes(tag(cell.id))}
                  type="button"
                  onmouseenter={() => (trying = cell.id)}
                  onmouseleave={() => (trying = null)}
                  onfocus={() => (trying = cell.id)}
                  onblur={() => (trying = null)}
                  onclick={() => adopt(cell.id)}
                  oncontextmenu={(e) => (e.preventDefault(), toggleFavorite(cell.id))}
                  title={cell.id}
                >
                  <span class="art">
                    {#if cell.thumb}<img src={cell.thumb} alt="" />{:else}<span class="noart">{cell.label}</span>{/if}
                  </span>
                  <span class="nm mono">{cell.label}</span>
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
  .screen {
    display: flex;
    flex-direction: column;
    height: 100%;
    min-height: 0;
  }

  .head {
    display: flex;
    align-items: baseline;
    gap: 14px;
    padding: 0 0 12px;
  }
  .sub {
    font-size: 11.5px;
    color: var(--muted);
    max-width: 560px;
  }

  /* --- barre d'outils (§8.1) --- */
  .toolbar {
    display: flex;
    align-items: flex-end;
    gap: 18px;
    padding: 0 0 14px;
    border-bottom: 1px solid var(--line);
  }
  .field {
    display: flex;
    flex-direction: column;
    gap: 5px;
  }
  .field .input {
    min-width: 210px;
  }
  .seg {
    display: flex;
    border: 1px solid var(--line);
    border-radius: 2px;
    overflow: hidden;
    height: 32px;
  }
  .sg {
    display: flex;
    align-items: center;
    padding: 0 12px;
    font-size: 11px;
    color: var(--muted);
    background: var(--raised);
    border: 0;
    border-right: 1px solid var(--line);
    cursor: pointer;
  }
  .sg:last-child {
    border-right: 0;
  }
  .sg.on {
    background: var(--card);
    color: var(--txt);
  }
  .chk {
    display: flex;
    align-items: center;
    gap: 8px;
    height: 32px;
    font-size: 12px;
    color: var(--muted);
    cursor: pointer;
  }
  /* Deux lignes, alignées à droite, repli plutôt que troncature — le risque
     est la largeur en allemand, pas le sens (§8.3). */
  .count {
    margin-left: auto;
    text-align: right;
    font-size: 11px;
    color: var(--faint);
    line-height: 1.6;
    max-width: 240px;
    display: flex;
    flex-direction: column;
  }
  .count b {
    color: var(--muted);
    font-weight: 500;
  }

  /* --- corps de l'écran --- */
  .body {
    display: flex;
    flex: 1;
    min-height: 0;
  }
  /* Le panneau d'essayage est fixe, seule la galerie défile (§4) : le pilote
     ne quitte jamais le champ de vision. */
  .fitting {
    width: 392px;
    flex: 0 0 392px;
    border-right: 1px solid var(--line);
    background: var(--panel);
    display: flex;
    flex-direction: column;
    min-height: 0;
  }

  /* --- les pistes (§5.5) --- */
  .lanes {
    border-top: 1px solid var(--line);
    padding: 12px 12px 14px;
  }
  .lane-sep {
    display: flex;
    align-items: center;
    gap: 8px;
    margin: 2px 2px 8px;
    font-size: 9px;
    letter-spacing: 0.2em;
    color: var(--faint);
  }
  .lane-sep::after {
    content: "";
    flex: 1;
    height: 1px;
    background: var(--line);
  }
  /* L'intertitre qui suit une piste, pas celui de tête. */
  .lane + .lane-sep {
    margin-top: 12px;
  }
  .lane {
    display: grid;
    grid-template-columns: 96px 1fr auto;
    align-items: center;
    gap: 10px;
    width: 100%;
    padding: 9px 10px;
    border-radius: 2px;
    border: 1px solid transparent;
    border-left: 2px solid transparent;
    background: transparent;
    text-align: left;
    cursor: pointer;
  }
  .lane + .lane {
    margin-top: 2px;
  }
  .body-lane {
    background: var(--card);
    border-color: var(--line);
    border-left-color: var(--faint);
  }
  /* Bordure gauche en accent : un des trois seuls emplois du rouge saturé
     sur cet écran (§15). */
  .lane.on {
    background: var(--raised);
    border-color: var(--line);
    border-left-color: var(--rosso);
  }
  .lane .k {
    font-size: 9.5px;
    letter-spacing: 0.2em;
    color: var(--faint);
  }
  .lane .v {
    font-size: 12.5px;
    color: var(--txt);
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
    display: block;
    width: 100%;
    margin-top: 12px;
    padding-top: 11px;
    text-align: center;
    font-size: 10.5px;
    letter-spacing: 0.12em;
    color: var(--muted);
    border: 0;
    border-top: 1px solid var(--line);
    background: transparent;
    cursor: pointer;
  }
  .reset:hover {
    color: var(--txt);
  }

  /* --- galerie (§6, §7) --- */
  .gallery {
    flex: 1;
    min-width: 0;
    overflow: auto;
    padding: 0 20px 30px;
  }
  .notice {
    display: flex;
    gap: 12px;
    align-items: flex-start;
    margin: 16px 0 4px;
    padding: 11px 13px;
    border: 1px solid var(--line);
    border-left: 2px solid var(--orange);
    background: var(--card);
    border-radius: 2px;
  }
  .notice .t {
    font-size: 12px;
    color: var(--orange);
  }
  .notice ul {
    margin: 6px 0 0;
    padding-left: 16px;
    font-size: 11.5px;
    color: var(--muted);
    line-height: 1.6;
  }
  .acts {
    margin-left: auto;
    display: flex;
    gap: 8px;
    flex: 0 0 auto;
  }
  .mini {
    font-size: 10.5px;
    letter-spacing: 0.1em;
    color: var(--muted);
    border: 1px solid var(--line);
    background: transparent;
    border-radius: 2px;
    padding: 5px 9px;
    white-space: nowrap;
    cursor: pointer;
  }
  .mini.on {
    border-color: var(--rosso-border);
    color: var(--rosso-bright);
  }

  .grp {
    display: flex;
    align-items: baseline;
    gap: 9px;
    margin: 22px 0 9px;
    font-size: 12.5px;
    color: var(--muted);
  }
  .grp .id {
    font-size: 10px;
    color: var(--faint);
  }
  .grp .n {
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
    border: 0;
    padding: 0;
    background: transparent;
    text-align: left;
    cursor: pointer;
  }
  /* Carré plein, filet franc, aucune ombre : l'échantillon doit être
     lisiblement une image et non un aperçu du résultat (§7.3). */
  .cell .art {
    display: block;
    aspect-ratio: 1;
    border: 1px solid var(--line);
    overflow: hidden;
    background: var(--card);
    filter: saturate(0.86) brightness(0.94);
    transition: filter 0.12s;
  }
  .cell .art img {
    display: block;
    width: 100%;
    height: 100%;
    object-fit: cover;
  }
  .noart,
  .cell.special .art {
    display: flex;
    align-items: center;
    justify-content: center;
    text-align: center;
    height: 100%;
    padding: 8px;
    font-size: 10px;
    color: var(--faint);
    line-height: 1.4;
  }
  .cell.special .art {
    background: repeating-linear-gradient(
      135deg,
      var(--card),
      var(--card) 5px,
      var(--raised) 5px,
      var(--raised) 10px
    );
    filter: none;
  }
  .cell .nm {
    display: block;
    margin-top: 6px;
    font-size: 10px;
    color: var(--faint);
    white-space: nowrap;
    overflow: hidden;
    text-overflow: ellipsis;
  }
  .cell:hover .art,
  .cell:focus-visible .art {
    filter: none;
    border-color: var(--rosso-border);
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
    color: var(--txt);
  }
  /* L'image est le contenu : on ne pose rien dessus, l'étoile préfixe le nom
     (§7.3). */
  .cell.fav .nm::before {
    content: "★ ";
    color: var(--rosso-border);
  }

  .empty {
    border: 1px dashed var(--line);
    border-radius: 2px;
    padding: 34px 18px;
    text-align: center;
    margin-top: 22px;
  }
  .empty .h {
    font-size: 13px;
    color: var(--txt);
    margin-bottom: 6px;
  }
  .empty .p {
    font-size: 11.5px;
    color: var(--faint);
    line-height: 1.6;
    max-width: 380px;
    margin: 0 auto 16px;
  }
  .row-acts {
    display: flex;
    gap: 8px;
    justify-content: center;
  }
  .noresult {
    font-size: 11.5px;
    color: var(--muted);
    margin: 18px 0 0;
  }
  .link {
    border: 0;
    background: transparent;
    color: var(--rosso-bright);
    font-size: 11.5px;
    cursor: pointer;
    padding: 0 0 0 6px;
  }
</style>
