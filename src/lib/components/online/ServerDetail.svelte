<script lang="ts">
  // One server, beside the list (SPEC-play-online.md, "Détail d'un serveur"),
  // in the order one decides: where, when, with what — then join. Asked from
  // the server itself, so the car slots are live rather than the lobby's.
  //
  // The skin under a car is the one the server will impose (its first free
  // slot): a skin chosen here would be ignored in game, so none is offered.
  //
  // Joining itself — level, layers, content fetched first, the button — is
  // the foot's (`JoinFooter`): this panel says what the server is.
  import { untrack } from "svelte";
  import { t } from "$lib/i18n/index.svelte";
  import { errorText } from "$lib/errors";
  import { serverDetail, serverKey, type CarSlots, type Fetch, type ServerDetail, type ServerSummary } from "$lib/online/online";
  import { carName, layoutLook, layoutName, trackName, trackTitle, type Looks } from "$lib/online/looks";
  import { isFavourite, isFriend } from "$lib/online/lists";
  import { onLibraryChange } from "$lib/library/libraryVersion.svelte";
  import { pingOf } from "$lib/online/pings.svelte";
  import SessionTimeline from "./SessionTimeline.svelte";
  import ServerExtras from "./ServerExtras.svelte";
  import JoinFooter from "./JoinFooter.svelte";
  import NotifyMe from "./NotifyMe.svelte";
  import ServerCountry from "./ServerCountry.svelte";
  import LevelTag from "./LevelTag.svelte";
  import { onlineStore, toggleFavouriteServer, toggleFriendName } from "$lib/online/store.svelte";
  import { previewSrc } from "$lib/library/library";
  import { shareLink } from "$lib/online/link";

  interface Props {
    server: ServerSummary;
    looks: Looks;
    /** The car last joined with on this server: picked again when it can
     * still be taken — "rejoindre à l'identique est un clic". */
    preferredCar: string | null;
    /** A password carried by a pasted connection link, typed in for you. */
    presetPassword?: string | null;
    onclose: () => void;
  }
  let { server, looks, preferredCar, presetPassword = null, onclose }: Props = $props();

  const favourite = $derived(isFavourite(onlineStore(), serverKey(server)));

  let detail = $state<ServerDetail | null>(null);
  let loading = $state(false);
  let error = $state("");
  let car = $state<string | null>(null);
  let password = $state("");
  /** The foot is fetching content: re-reads wait for it to finish. */
  let preparing = $state(false);

  /** The server the panel was last loaded for. A refresh of the list hands a
   * new object for the same server: that must not wipe the car picked and the
   * password typed. */
  let loadedKey = "";

  // Read the key first: the effect must subscribe to the server before any
  // early exit (CLAUDE.md, "Un $effect ne s'abonne qu'à ce qu'il a lu").
  $effect(() => {
    const key = serverKey(server);
    if (key === loadedKey) return;
    loadedKey = key;
    untrack(() => void load(key));
  });

  // A link pasted for the server already open does not reload the panel:
  // its password still has to land in the field.
  $effect(() => {
    const preset = presetPassword;
    if (preset) password = preset;
  });

  async function load(key: string) {
    const { ip, http_port } = server;
    detail = null;
    error = "";
    car = null;
    password = presetPassword ?? "";
    loading = true;
    try {
      const d = await serverDetail(ip, http_port);
      if (key !== serverKey(server)) return; // another server was picked meanwhile
      detail = d;
      // The car of the last join comes first; otherwise a single car that can
      // be taken is the obvious choice; among several, the pick is the user's.
      const takeable = d.cars.filter(canTake);
      const again = takeable.find((c) => c.id.toLowerCase() === preferredCar?.toLowerCase());
      if (again) car = again.id;
      else if (takeable.length === 1) car = takeable[0].id;
    } catch (e) {
      if (key === serverKey(server)) error = errorText(e);
    } finally {
      if (key === serverKey(server)) loading = false;
    }
  }

  /** Both versions are known and differ (SPEC-play-online.md, "Versions à
   * côté du choix") — an integrity check kicks a mismatched car. */
  function versionsDiffer(f: Fetch): boolean {
    return !!f.installed_version && !!f.server_version && f.installed_version !== f.server_version;
  }

  /** A car the panel can pick for you: one with a free slot that can be
   * driven here. Any car with a free slot can still be picked by hand — the
   * button then says why it cannot join, which is what one wants to know. */
  function canTake(c: CarSlots): boolean {
    return c.available && c.free > 0;
  }

  const live = $derived(detail?.summary ?? server);

  /** The link was just copied: the button says so for a moment. */
  let copied = $state(false);
  async function copyLink() {
    try {
      await navigator.clipboard.writeText(shareLink(server.ip, server.http_port));
      copied = true;
      setTimeout(() => (copied = false), 2000);
    } catch (e) {
      console.error("copy the server link", e);
    }
  }
  /** The lobby geolocates servers; a server's own /INFO often cannot. */
  const country = $derived(live.country ?? server.country);
  const banner = $derived(layoutLook(looks, live.track));
  /** The cars one can drive first, each group in the server's order. */
  const cars = $derived(detail ? [...detail.cars.filter((c) => c.available), ...detail.cars.filter((c) => !c.available)] : []);

  const chosen = $derived(cars.find((c) => c.id === car) ?? null);

  /** Reads the server again without losing what was chosen — after content
   * arrived, the levels have moved. */
  async function refresh() {
    const key = serverKey(server);
    try {
      const d = await serverDetail(server.ip, server.http_port);
      if (key === serverKey(server)) detail = d;
    } catch (e) {
      console.error("online_server_detail", e);
    }
  }

  // An archive dropped on the window — the way content from a page in the
  // browser arrives — changes the library: the panel reads its levels again.
  // Not while the foot is fetching: it reads the server itself once done.
  onLibraryChange(() => {
    if (detail && !preparing) void refresh();
  });
</script>

<aside class="panel">
  <!-- Where first, in large: the track's photo is what one recognises. A
       track that is not here gets the empty frame, dimmed by `missing`. -->
  <div class="banner" class:missing={!live.track_available}>
    {#if banner?.preview}<img src={previewSrc(banner.preview)} alt="" />{/if}
    {#if banner?.outline}<img class="outline" src={previewSrc(banner.outline)} alt="" />{/if}
    <span class="banner-name">{trackTitle(looks, live.track)}</span>
  </div>
  <header class="head">
    <div class="title">
      <h3 title={server.name}>{server.name}</h3>
      <!-- The names one reads, as in the table; the ids on hover. -->
      <p class="where" title={live.track.kunos_id}>
        <span>{trackName(looks, live.track)}</span>
        {#if layoutName(looks, live.track)}<span class="sub">{layoutName(looks, live.track)}</span>{/if}
      </p>
    </div>
    <button
      class="fav"
      class:on={favourite}
      type="button"
      title={favourite ? t("online.unfavourite") : t("online.favourite")}
      aria-label={favourite ? t("online.unfavourite") : t("online.favourite")}
      aria-pressed={favourite}
      onclick={() => toggleFavouriteServer(server)}>{favourite ? "★" : "☆"}</button
    >
    <!-- The link to send a friend (case 1 of the spec): CM's share link,
         which a friend opens with CM, or pastes into Pit Box. -->
    <button
      class="share"
      class:done={copied}
      type="button"
      title={copied ? t("online.linkCopied") : t("online.copyLink")}
      aria-label={copied ? t("online.linkCopied") : t("online.copyLink")}
      onclick={copyLink}>{copied ? "✓" : "⧉"}</button
    >
    <button class="close" type="button" title={t("common.close")} aria-label={t("common.close")} onclick={onclose}>✕</button>
  </header>

  <div class="facts mono">
    <span class="players">{live.clients} / {live.max_clients}</span>
    {#if live.track.csp_min_build}<span>{t("online.csp", { build: live.track.csp_min_build })}</span>{/if}
    {#if country}<ServerCountry code={country} named />{/if}
    {#if pingOf(serverKey(server)) !== undefined}<span>{t("online.ping", { ms: pingOf(serverKey(server)) ?? 0 })}</span>{/if}
    <span class="addr">{live.ip}:{live.http_port}</span>
    {#if detail && versionsDiffer(detail.track_fetch)}
      <span class="version">
        {t("online.versions", {
          installed: detail.track_fetch.installed_version ?? "",
          server: detail.track_fetch.server_version ?? "",
        })}
      </span>
    {/if}
  </div>

  <SessionTimeline server={live} />

  <div class="body">
    {#if loading}
      <p class="muted">{t("common.loading")}</p>
    {:else if error}
      <p class="errbox">{error}</p>
    {:else if detail}
      {#if !live.booking}
        <h4 class="lbl">{t("online.cars")}</h4>
        <ul class="cars">
          {#each cars as c (c.id)}
            <li>
              <button
                type="button"
                class="car"
                class:on={car === c.id}
                class:full={c.free === 0}
                aria-pressed={car === c.id}
                onclick={() => (car = c.id)}
              >
                <span class="photo">
                  {#if c.preview}<img src={previewSrc(c.preview)} alt="" loading="lazy" />{/if}
                </span>
                <span class="car-name" title={c.id}>{carName(looks, c.id)}</span>
                <span class="skin mono">
                  {#if versionsDiffer(c.fetch)}
                    <span class="version">
                      {t("online.versions", {
                        installed: c.fetch.installed_version ?? "",
                        server: c.fetch.server_version ?? "",
                      })}
                    </span>
                  {:else}
                    {c.skin ?? ""}
                  {/if}
                </span>
                <span class="slots mono" class:none={c.free === 0}>
                  {t("online.slots", { free: c.free, total: c.total })}
                </span>
                {#if c.level !== "ready"}
                  <span class="tag"><LevelTag level={c.level} dlc={c.dlc} /></span>
                {/if}
              </button>
            </li>
          {/each}
        </ul>
      {/if}

      <!-- The cars no player takes (an AssettoServer's AI traffic): not a
           choice, but needed all the same — what « N cars missing » counts
           when the cars above are all here. -->
      {#if detail.other_cars.length}
        <h4 class="lbl">{t("online.otherCars")}</h4>
        <ul class="others">
          {#each detail.other_cars as c (c.id)}
            <li>
              <span class="other-name" title={c.id}>{carName(looks, c.id)}</span>
              {#if c.level !== "ready"}
                <span class="tag"><LevelTag level={c.level} dlc={c.dlc} /></span>
              {/if}
            </li>
          {/each}
        </ul>
      {/if}

      <h4 class="lbl">{t("online.drivers")}</h4>
      {#if detail.drivers.length}
        <ul class="drivers">
          {#each detail.drivers as d, i (i)}
            {@const friend = isFriend(onlineStore(), d.name)}
            <li>
              <!-- A click on a name marks a friend (SPEC-play-online.md, "Détail
                   d'un serveur"); the star says which way the click goes. -->
              <button
                class="driver"
                class:friend
                type="button"
                title={friend ? t("online.unfriend") : t("online.befriend")}
                aria-pressed={friend}
                onclick={() => toggleFriendName(d.name)}
              >
                <span class="mark">{friend ? "★" : "☆"}</span>{d.name}
              </button>
              <span class="sub">{carName(looks, d.car)}</span>
            </li>
          {/each}
        </ul>
      {:else}
        <p class="muted">{t("online.noDrivers")}</p>
      {/if}

      <ServerExtras {detail} />
    {/if}
  </div>

  <NotifyMe server={live} {chosen} />
  {#key serverKey(server)}
    <JoinFooter {server} {detail} {chosen} {looks} {loading} {refresh} bind:password bind:busy={preparing} />
  {/key}
</aside>

<style>
  .panel {
    display: flex;
    flex-direction: column;
    min-height: 0;
    height: 100%;
    border-left: 1px solid var(--line);
    background: var(--panel2);
  }
  .banner {
    position: relative;
    height: 150px;
    flex: none;
    background: var(--card);
    overflow: hidden;
  }
  .banner img {
    position: absolute;
    inset: 0;
    width: 100%;
    height: 100%;
    object-fit: cover;
  }
  /* The layout's outline over its photo, as on the session column's track
     card: lower right, small, never covering the name. */
  .banner img.outline {
    inset: auto 10px 10px auto;
    width: 90px;
    height: 70px;
    object-fit: contain;
  }
  .banner.missing img {
    opacity: 0.35;
  }
  .banner-name {
    position: absolute;
    left: 14px;
    bottom: 10px;
    right: 110px;
    color: var(--txt);
    font-size: 15px;
    font-weight: 600;
    text-shadow: 0 1px 4px rgba(0, 0, 0, 0.85);
  }
  .head {
    display: flex;
    align-items: flex-start;
    gap: 10px;
    padding: 12px 16px 10px;
  }
  .title {
    flex: 1;
    min-width: 0;
  }
  h3 {
    font-size: 14px;
    font-weight: 600;
    color: var(--txt);
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }
  .where {
    margin-top: 4px;
    font-size: 12px;
    color: var(--txt2);
    display: flex;
    gap: 8px;
    min-width: 0;
    white-space: nowrap;
    overflow: hidden;
    text-overflow: ellipsis;
  }
  .sub {
    color: var(--muted);
  }
  .close {
    background: none;
    border: none;
    color: var(--muted);
    font-size: 13px;
    padding: 2px 4px;
  }
  .close:hover {
    color: var(--txt);
  }
  .share {
    background: none;
    border: none;
    color: var(--muted);
    font-size: 14px;
    line-height: 1;
    padding: 0 4px;
  }
  .share:hover {
    color: var(--txt);
  }
  .share.done {
    color: var(--green);
  }
  .fav {
    background: none;
    border: none;
    color: var(--muted);
    font-size: 16px;
    line-height: 1;
    padding: 0 4px;
  }
  .fav:hover,
  .fav.on {
    color: var(--yellow);
  }
  .facts {
    display: flex;
    flex-wrap: wrap;
    gap: 6px 14px;
    padding: 0 16px 12px;
    font-size: 11px;
    color: var(--muted);
    border-bottom: 1px solid var(--line);
  }
  .facts .players {
    color: var(--txt);
    font-size: 13px;
  }
  .body {
    flex: 1;
    min-height: 0;
    overflow-y: auto;
    padding: 12px 16px;
  }
  .lbl {
    margin: 6px 0 8px;
  }
  .cars,
  .drivers {
    list-style: none;
    display: flex;
    flex-direction: column;
    gap: 4px;
    margin-bottom: 14px;
  }
  .car {
    display: grid;
    grid-template-columns: 64px 1fr auto;
    grid-template-areas:
      "photo id slots"
      "photo skin tag";
    align-items: center;
    gap: 2px 10px;
    width: 100%;
    padding: 7px 10px;
    background: var(--card);
    border: 1px solid var(--line);
    color: var(--txt2);
    text-align: left;
  }
  .car:hover {
    border-color: var(--rosso-border);
  }
  .car.on {
    border-color: var(--rosso);
    background: var(--rosso-dim);
  }
  /* A car without a free slot stays dim, but can be picked: to wait for a
     slot of it (« Notify me »). The join itself says it is full. */
  .car.full {
    opacity: 0.45;
  }
  .car.full.on {
    opacity: 0.8;
  }
  .photo {
    grid-area: photo;
    width: 64px;
    height: 36px;
    background: var(--panel2);
    overflow: hidden;
  }
  .photo img {
    width: 100%;
    height: 100%;
    object-fit: cover;
    display: block;
  }
  .car-name {
    grid-area: id;
    font-size: 12px;
    color: var(--txt);
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }
  .skin {
    grid-area: skin;
    font-size: 10.5px;
    color: var(--muted);
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }
  .slots {
    grid-area: slots;
    font-size: 11px;
    color: var(--green);
    text-align: right;
  }
  .slots.none {
    color: var(--muted);
  }
  /* The colours are `LevelTag`'s, the same as the list's state column. */
  .tag {
    grid-area: tag;
    font-size: 10px;
    text-align: right;
  }
  /* A version gap with the server goes orange (SPEC-play-online.md,
     "Versions à côté du choix"). */
  .version {
    color: var(--orange);
  }
  .others li {
    display: flex;
    justify-content: space-between;
    gap: 10px;
    font-size: 12px;
    color: var(--txt2);
    padding: 3px 0;
  }
  .other-name {
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }
  .others .tag {
    grid-area: auto;
    flex: none;
  }
  .drivers li {
    display: flex;
    justify-content: space-between;
    align-items: center;
    gap: 10px;
    font-size: 12px;
    color: var(--txt2);
  }
  .driver {
    display: flex;
    align-items: center;
    gap: 6px;
    min-width: 0;
    background: none;
    border: none;
    padding: 0;
    color: var(--txt2);
    font-size: 12px;
    text-align: left;
  }
  .driver .mark {
    color: var(--faint);
  }
  .driver:hover .mark {
    color: var(--muted);
  }
  .driver.friend,
  .driver.friend .mark {
    color: var(--green);
  }
  .muted {
    color: var(--muted);
    font-size: 12px;
  }
</style>
