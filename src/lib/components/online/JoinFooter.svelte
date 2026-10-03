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
  import { fetchNeeded, type Needed } from "$lib/online/prepare";
  import { blockerText, levelText } from "$lib/online/labels";
  import { recordRecentJoin } from "$lib/online/store.svelte";
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
  /** Joining with the chosen car — on a booking server the car is picked in
   * CM, so the server's own level (its best car) stands in. */
  const readiness = $derived(joinState(live, live.booking ? null : chosen));

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
    if (!live.booking && chosen?.fetch.needed) {
      items.push({ kind: "Car", id: chosen.id, name: carName(looks, chosen.id), fetch: chosen.fetch });
    }
    return items;
  });

  /** Why the button cannot join yet, or `null` when it can. */
  const blocker = $derived.by(() => {
    if (!live.booking && live.clients >= live.max_clients) return t("online.serverFull");
    if (!live.booking && !chosen) return t("online.pickCar");
    if (readiness.level === "blocked") return readiness.blockers[0] ? blockerText(readiness.blockers[0]) : levelText("blocked");
    if (readiness.level === "download") {
      return (live.track_level ?? "download") === "download" ? t("online.trackMissing") : t("online.carMissing");
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
      const out = await fetchNeeded(needed);
      if (out.status === "browser") {
        browserFor = out.name;
        openUrl(out.url).catch((e) => console.error("openUrl", e));
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
      const missing =
        (fetched.includes(live.track.id.toLowerCase()) && !live.track_available) ||
        (!!chosen && fetched.includes(chosen.id.toLowerCase()) && !chosen.available);
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
      if (!(await prepare())) return;
      await joinServer(live, carId, password || null, setAside);
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
  {#if joinError}<p class="errbox">{joinError}</p>{/if}
  {#if joined}<p class="ok">{t("online.joined")}</p>{/if}
  <button class="btn btn-primary join" type="button" disabled={!!blocker || joining || loading} onclick={join}>
    {busy ? t("online.preparing") : joining ? t("online.joining") : (blocker ?? joinLabel)}
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
