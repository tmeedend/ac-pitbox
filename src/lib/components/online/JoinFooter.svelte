<script lang="ts">
  // The foot of a server's panel: everything about joining it
  // (SPEC-play-online.md, "Détail d'un serveur" and "Contenu manquant").
  // The level of joining with the chosen car, what blocks, the layers set
  // aside for the session, the content fetched first, and the button that
  // follows all of it.
  //
  // Out of `ServerDetail`, which keeps what the server is (where, when, which
  // cars, who) — this changes with how a join is prepared, that with what a
  // server tells. The panel mounts it again for each server (`{#key}`), which
  // is what clears the error, the choices and the "joined" line.
  import { t } from "$lib/i18n/index.svelte";
  import { errorText } from "$lib/errors";
  import { openUrl } from "@tauri-apps/plugin-opener";
  import { joinServer, type CarSlots, type ServerDetail, type ServerSummary } from "$lib/online/online";
  import { carName, trackTitle, type Looks } from "$lib/online/looks";
  import { joinState, layersToSetAside } from "$lib/online/readiness";
  import { fetchNeeded, type Needed } from "$lib/online/prepare.svelte";
  import { blockerText, levelText } from "$lib/online/labels";
  import { recordRecentJoin } from "$lib/online/store.svelte";
  import { passwordMatches } from "$lib/online/password";
  import { launcherGate, unsourced, webSearchUrl } from "$lib/online/outside";
  import { gameSession, markStarting } from "$lib/launch/gameSession.svelte";
  import LayersNotice from "./LayersNotice.svelte";

  interface Props {
    /** The list's entry: what Recent keeps (its name is the lobby's cleaned
     * one, its country the lobby's geolocation). */
    server: ServerSummary;
    detail: ServerDetail | null;
    /** The car picked in the panel. */
    chosen: CarSlots | null;
    looks: Looks;
    loading: boolean;
    password: string;
    /** Reads the server again, keeping what was chosen. */
    refresh: () => Promise<void>;
    /** Content is being fetched: the panel holds its own re-reads meanwhile. */
    busy: boolean;
  }
  let {
    server,
    detail,
    chosen,
    looks,
    loading,
    password = $bindable(),
    refresh,
    busy = $bindable(),
  }: Props = $props();

  let joining = $state(false);
  let joinError = $state("");
  let joined = $state(false);
  /** A source gave a page: what was opened in the browser. */
  let browserFor = $state<string | null>(null);

  const live = $derived(detail?.summary ?? server);
  /** Every car the server runs, the ones nobody takes (AI traffic) included:
   * the game loads them all, so all of them are needed. */
  const allCars = $derived(detail ? [...detail.cars, ...detail.other_cars] : []);
  /** Joining with the chosen car and every other one — on a booking server
   * the car is picked in CM, but the others are needed all the same. */
  const readiness = $derived(joinState(live, live.booking ? null : chosen, allCars));

  /** The layers of the track and of the chosen car (SPEC-play-online.md,
   * "Couches et versions"), and what the user chose to do with them. */
  let keepCertain = $state(false);
  let dropPossible = $state(false);
  const conflicts = $derived([...(detail?.track_layers ?? []), ...(live.booking ? [] : (chosen?.layers ?? []))]);
  const setAside = $derived(layersToSetAside(conflicts, { keepCertain, dropPossible }));

  /** The button follows the level (SPEC-play-online.md): JOIN when all is in
   * the game, PREPARE & JOIN when the join first lays something from the
   * library into it. */
  const joinLabel = $derived(
    live.booking
      ? t("online.openInCm")
      : readiness.level === "oneClick" || setAside.length > 0
        ? t("online.prepareJoin")
        : t("online.join"),
  );

  /** What "Prepare & join" fetches first: the track and the chosen car, when
   * missing or outdated with a source (`online/content.rs`). */
  const needed = $derived.by((): Needed[] => {
    const items: Needed[] = [];
    if (detail?.track_fetch.needed) {
      items.push({ kind: "Track", id: live.track.id, name: trackTitle(looks, live.track), fetch: detail.track_fetch });
    }
    // The chosen car first, then every other one the server needs.
    const cars = [...(chosen && !live.booking ? [chosen] : []), ...allCars.filter((c) => c.id !== chosen?.id)];
    for (const c of cars) {
      if (c.fetch.needed) items.push({ kind: "Car", id: c.id, name: carName(looks, c.id), fetch: c.fetch });
    }
    return items;
  });

  // The page in the browser has done its job once what it was for is here:
  // the archive dropped, imported, the panel read again — the line asking to
  // download it goes. It stayed, and read as if nothing had happened
  // (reported).
  $effect(() => {
    const name = browserFor;
    const stillNeeded = needed.some((n) => n.name === name);
    if (name && !stillNeeded) browserFor = null;
  });

  /** Content nobody but the user can find: no archive kept, no link from the
   * server, nothing in the registry. Searched on the web by its id, the
   * archive then dropped on Pit Box like any other. */
  const toSearch = $derived(
    unsourced([
      ...(detail
        ? [{ id: live.track.id, name: trackTitle(looks, live.track), level: live.track_level ?? "download", fetch: detail.track_fetch }]
        : []),
      ...allCars.map((c) => ({ id: c.id, name: carName(looks, c.id), level: c.level, fetch: c.fetch })),
    ]),
  );
  /** A few names, then a count: a No Hesi server misses thirty cars. */
  const SEARCH_SHOWN = 5;

  /** A server that lets in only the players of its community's launcher. */
  const gate = $derived(launcherGate(detail?.links ?? []));

  function openPage(url: string) {
    openUrl(url).catch((e) => console.error("openUrl", e));
  }

  /** Why the button cannot join yet, or `null` when it can. */
  const blocker = $derived.by(() => {
    if (!live.booking && live.clients >= live.max_clients) return t("online.serverFull");
    if (!live.booking && !chosen) return t("online.pickCar");
    // Picked to wait for one of its slots (« Notify me »), not to join now.
    if (!live.booking && chosen && chosen.free === 0) return t("online.carFull");
    if (readiness.level === "blocked") return readiness.blockers[0] ? blockerText(readiness.blockers[0]) : levelText("blocked");
    if (readiness.level === "download") {
      if ((live.track_level ?? "download") === "download") return t("online.trackMissing");
      const missing = readiness.missingCars;
      return missing.length > 1 ? t("online.carsMissing", { count: missing.length }) : t("online.carMissing");
    }
    return null;
  });

  /** Fetches what is needed first; `false` when the join cannot go on —
   * the reason is then on screen. */
  async function prepare(): Promise<boolean> {
    if (!needed.length) return true;
    busy = true;
    browserFor = null;
    try {
      const out = await fetchNeeded(needed, live.name);
      if (out.status === "browser") {
        browserFor = out.name;
        openPage(out.url);
        return false;
      }
      if (out.status === "busy") joinError = t("online.prepareBusy");
      if (out.status !== "ready") return false;
      const fetched = needed.map((n) => n.id.toLowerCase());
      await refresh();
      // What was missing must be here now — an import that stopped on a
      // question has brought nothing in yet. Only that: an update just
      // fetched may still read as older, authors' version labels rarely
      // follow the server's, and asking again would download it forever.
      const freshCars = detail ? [...detail.cars, ...detail.other_cars] : [];
      const missing =
        (fetched.includes(live.track.id.toLowerCase()) && !live.track_available) ||
        freshCars.some((c) => fetched.includes(c.id.toLowerCase()) && !c.available);
      if (missing) {
        joinError = t("online.prepareIncomplete");
        return false;
      }
      return true;
    } finally {
      busy = false;
    }
  }

  async function join() {
    joining = true;
    joinError = "";
    joined = false;
    const carId = live.booking ? "" : (chosen?.id ?? "");
    try {
      // Checked before anything is fetched or launched, where the server
      // allows it: a wrong password otherwise costs the game's whole loading
      // to be refused at the end of it.
      const check = detail?.extended?.password_check;
      if (live.password && check && !(await passwordMatches(check, password))) {
        joinError = t("online.passwordWrong");
        return;
      }
      if (!(await prepare())) return;
      const others = allCars.map((c) => c.id).filter((id) => id !== carId);
      await joinServer(live, carId, password || null, setAside, others);
      markStarting();
      joined = true;
      recordRecentJoin(server, carId);
    } catch (e) {
      joinError = errorText(e);
    } finally {
      joining = false;
    }
  }
</script>

<footer class="foot">
  {#if live.password}
    <input class="input" type="password" autocomplete="off" placeholder={t("online.passwordPlaceholder")} bind:value={password} />
  {/if}
  {#if readiness.level === "blocked" && readiness.blockers.length > 1}
    <!-- The button names the first reason; the others are here. -->
    {#each readiness.blockers.slice(1) as b, i (i)}<p class="why">{blockerText(b)}</p>{/each}
  {/if}
  {#if conflicts.length && readiness.level !== "blocked"}
    <LayersNotice {conflicts} bind:keepCertain bind:dropPossible />
  {/if}
  {#if browserFor}
    <!-- The page asks something of a person; the archive, once dropped on
         the window, is recognised and the panel reads its levels again. -->
    <p class="warnbox">{t("online.prepareBrowser", { name: browserFor })}</p>
  {/if}
  {#if gate}
    <!-- Not a block: a player registered with them may get in. What CM's own
         join meets too (measured: « handshake failed »). -->
    <div class="warnbox gate">
      <p>{t("online.launcherGate", { name: gate.name })}</p>
      <button class="btn" type="button" onclick={() => openPage(gate.url)}>{t("online.launcherGateOpen", { name: gate.name })}</button>
    </div>
  {/if}
  {#if toSearch.length}
    <div class="search">
      <p class="why">{t("online.noSource")}</p>
      <ul>
        {#each toSearch.slice(0, SEARCH_SHOWN) as w (w.id)}
          <li>
            <span class="name" title={w.id}>{w.name}</span>
            <button class="btn" type="button" onclick={() => openPage(webSearchUrl(w.id))}>{t("online.searchWeb")}</button>
          </li>
        {/each}
      </ul>
      {#if toSearch.length > SEARCH_SHOWN}
        <p class="why">{t("online.noSourceMore", { count: toSearch.length - SEARCH_SHOWN })}</p>
      {/if}
    </div>
  {/if}
  {#if joinError}<p class="errbox">{joinError}</p>{/if}
  {#if joined}<p class="ok">{t("online.joined")}</p>{/if}
  <!-- One game at a time (SESSION§2.4): a join is a session like the others,
       and the button says why it waits rather than failing on click. -->
  <button
    class="btn btn-primary join"
    type="button"
    disabled={!!blocker || joining || loading || gameSession.phase !== "idle"}
    title={gameSession.phase === "running" ? t("session.runningTooltip") : undefined}
    onclick={join}
  >
    {busy
      ? t("online.preparing")
      : joining
        ? t("online.joining")
        : gameSession.phase === "running"
          ? t("session.running")
          : gameSession.phase === "starting"
            ? t("session.starting")
            : (blocker ?? joinLabel)}
  </button>
</footer>

<style>
  .foot {
    display: flex;
    flex-direction: column;
    gap: 8px;
    padding: 12px 16px 16px;
    border-top: 1px solid var(--line);
  }
  .why {
    color: var(--muted);
    font-size: 11.5px;
  }
  .gate {
    display: flex;
    flex-direction: column;
    gap: 8px;
    align-items: flex-start;
  }
  .search ul {
    display: flex;
    flex-direction: column;
    gap: 4px;
    margin-top: 4px;
  }
  .search li {
    display: flex;
    align-items: center;
    gap: 8px;
    font-size: 12px;
    color: var(--txt2);
  }
  .search .name {
    flex: 1;
    min-width: 0;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }
  .ok {
    color: var(--green);
    font-size: 12px;
  }
  .join {
    width: 100%;
    padding: 10px;
    font-size: 12px;
    letter-spacing: 0.06em;
    text-transform: uppercase;
  }
</style>
