// What the Online page shows of the lobby (SPEC-play-online.md, "Filtres de
// base"): the search, the chips and the list they leave — and the ping sweep
// a ping token sets off, since a server it has not measured cannot be kept.
//
// Out of `Online.svelte` because it is the part of the page that moves most:
// a chip added, a rule on one's own tabs, a filter that costs network. The
// page keeps what is the page's — loading the lobby, the open server, the
// ways in from elsewhere — and hands this class what it reads through getters
// (`FilterHost`), the way `OpponentGrid` reads its screen.
import { untrack } from "svelte";
import { t } from "$lib/i18n/index.svelte";
import { gameCountryByIso2, withCountryLabels } from "$lib/flags.svelte";
import type { FilterMap, FilterOption } from "$lib/library/filters";
import { searchServers, sortServers } from "./filters";
import { tabServers, type OnlineStore, type OnlineTab } from "./lists";
import { carName, searchText, type Looks } from "./looks";
import { serverKey, type ServerSummary } from "./online";
import { measureAll, pingOf, pingsLeft, stopMeasuring } from "./pings.svelte";
import { filterCategories, serverCategories } from "./categories";
import { trackCategories } from "./trackCategories.svelte";
import { withTrackCategoryLabels } from "$lib/library/trackCategoryTable.svelte";
import {
  asksPing,
  BOOL_KEYS,
  CATEGORY_KEY,
  DEFAULT_PINNED,
  DEFAULT_TOKENS,
  onlineTokenDefs,
  parseTokens,
  serializeTokens,
  splitPing,
  tokenOptions,
  tokenPredicate,
  trackLabel,
  trackValue,
  TRACK_KEY,
  type TokenContext,
} from "./tokens";

/** What the filter reads from the page. Getters: every one of them changes
 * under it. */
export interface FilterHost {
  readonly servers: ServerSummary[];
  readonly looks: Looks;
  readonly tab: OnlineTab;
  readonly store: OnlineStore;
  /** Per server key, the friends connected there. */
  readonly friends: Record<string, string[]>;
}

const withoutBools = (tokens: FilterMap) =>
  Object.fromEntries(Object.entries(tokens).filter(([k]) => !BOOL_KEYS.includes(k)));
const onlyBools = (tokens: FilterMap) => Object.fromEntries(Object.entries(tokens).filter(([k]) => BOOL_KEYS.includes(k)));

export class ServerFilter {
  // Assigned by the constructor, after the field initialisers: the derived
  // fields that read it go through `$derived.by`, whose function only runs
  // when the value is first read.
  #host: FilterHost;

  /** A gesture of the moment: not remembered. */
  search = $state("");
  /** The servers an included category leaves out for an unknown category,
   * shown anyway: the toggle under the list, a gesture of the moment too. */
  showUnknownCategories = $state(false);
  tokens = $state<FilterMap>({ ...DEFAULT_TOKENS });
  pinned = $state<string[]>([...DEFAULT_PINNED]);

  /** Track names, read off the servers that name them, one's own snapshots
   * included: a token restored for a server the lobby no longer lists keeps
   * its name. */
  readonly #trackNames = $derived.by(() => {
    const names: Record<string, string> = {};
    const { store, servers, looks } = this.#host;
    const own = [...store.favourites, ...store.recents.map((r) => r.server)];
    for (const s of [...own, ...servers]) names[trackValue(s.track)] = trackLabel(looks, s.track);
    return names;
  });

  /** The catalogue: without the yes/no chips on one's own tabs, whose
   * servers stay listed full, locked or empty — that is when one looks for
   * them. */
  readonly defs = $derived.by(() =>
    withTrackCategoryLabels(
      withCountryLabels(
        onlineTokenDefs(
          {
            track: (v) => this.#trackNames[v] ?? v,
            car: (v) => carName(this.#host.looks, v),
            ping: (v) => t("online.pingUnder", { ms: v }),
          },
          this.#host.tab === "all",
        ),
      ),
    ),
  );

  /** Each server's categories, resolved once per list and knowledge: the
   * filter asks for them on every change of chip, over 9 000 servers. */
  readonly #categories = $derived.by(() => {
    const { looks } = this.#host;
    const knowledge = trackCategories.knowledge;
    const memo = new Map<string, string[] | null>();
    return (s: ServerSummary): string[] | null => {
      const key = serverKey(s);
      if (!memo.has(key)) memo.set(key, filterCategories(serverCategories(s, looks, knowledge)));
      return memo.get(key) ?? null;
    };
  });

  readonly #ctx: TokenContext = $derived.by(() => ({
    looks: this.#host.looks,
    categoriesOf: this.#categories,
    showUnknownCategories: this.showUnknownCategories,
    pingOf: (s) => pingOf(serverKey(s)),
    // The game's English name is what flags and labels are keyed on. Fills
    // in once the game's table is loaded (the bar loads it).
    countryName: (iso2) => gameCountryByIso2(iso2)?.name ?? null,
  }));

  /** The servers the search and the tab leave, before the chips. */
  readonly #searched = $derived.by(() => {
    const { servers, looks, tab, store, friends } = this.#host;
    const text = (s: ServerSummary) => searchText(looks, s);
    return searchServers(tab === "all" ? servers : tabServers(tab, servers, store, friends), this.search, text);
  });

  /** An included category: the servers of unknown category are left out. */
  readonly #categoryIncluded = $derived.by(() => {
    const st = this.tokens[CATEGORY_KEY];
    return st?.type === "val" && st.values.some((v) => v.sign > 0);
  });

  /** Every filter but the ping, the servers of unknown category let through:
   * what a ping token has to measure, so that the line under the list counts
   * them as the list would show them. */
  readonly #loose = $derived.by(() => {
    const ctx = { ...this.#ctx, showUnknownCategories: true };
    const kept = this.#searched.filter(tokenPredicate(this.defs, splitPing(this.tokens).rest, ctx));
    return this.#host.tab === "all" ? sortServers(kept, new Set(Object.keys(this.#host.friends))) : kept;
  });

  /** The same with the unknown categories left out, as an included category
   * wants. Filtered from `#loose`, whose order it keeps. */
  readonly #strict = $derived.by(() => {
    if (!this.#categoryIncluded) return this.#loose;
    const ctx = { ...this.#ctx, showUnknownCategories: false };
    return this.#loose.filter(tokenPredicate(this.defs, splitPing(this.tokens).rest, ctx));
  });

  readonly #wantsPing = $derived(asksPing(this.tokens));
  readonly #pingOk = $derived(tokenPredicate(this.defs, splitPing(this.tokens).ping, this.#ctx));
  readonly #withPing = (list: ServerSummary[]) => (this.#wantsPing ? list.filter(this.#pingOk) : list);

  /** Servers an included category leaves out only because theirs is
   * unknown — shown or not: what the line under the list counts. 0 without
   * such a chip. */
  readonly unknownCount = $derived.by(() =>
    this.#categoryIncluded ? this.#withPing(this.#loose).length - this.#withPing(this.#strict).length : 0,
  );

  /** The servers to show, in the default order (friends, joinable, players). */
  readonly shown = $derived(this.#withPing(this.showUnknownCategories ? this.#loose : this.#strict));

  /** Servers the ping sweep has yet to ask, 0 without a ping token. */
  readonly measuring = $derived(this.#wantsPing ? pingsLeft() : 0);

  constructor(host: FilterHost) {
    this.#host = host;
  }

  /** What the bar holds: on one's own tabs, the tokens without the yes/no
   * chips it does not offer there. The bar must not see them — its « Clear
   * all » would show with no chip in view and wipe the All tab's « Not full » —
   * and what it writes back keeps them. */
  get barTokens(): FilterMap {
    return this.#host.tab === "all" ? this.tokens : withoutBools(this.tokens);
  }

  set barTokens(next: FilterMap) {
    this.tokens = this.#host.tab === "all" ? next : { ...next, ...onlyBools(this.tokens) };
  }

  /** The suggestions of a token, counted over the whole lobby. */
  optionsFor(key: string): FilterOption[] {
    const def = this.defs.find((d) => d.key === key);
    return def ? tokenOptions(def, this.#host.servers, this.#ctx) : [];
  }

  /** Poses the track token on these layouts (a track sheet's way in). */
  showTrack(layouts: string[]): void {
    const values = layouts.map((value) => ({ value, sign: 1 as const }));
    this.tokens = { ...this.tokens, [TRACK_KEY]: { type: "val", values, op: "or" } };
  }

  /** Takes back the chips and pins stored under `StorageKey.onlineFilters`. */
  restore(raw: string | null): void {
    ({ tokens: this.tokens, pinned: this.pinned } = parseTokens(raw));
  }

  /** What is stored under `StorageKey.onlineFilters`. */
  serialized(): string {
    return serializeTokens(this.tokens, this.pinned);
  }

  /**
   * Starts the ping sweep for the life of the calling component — to call
   * during its initialisation. A ping token cannot keep a server it has not
   * measured: the candidates are measured — those of unknown category an
   * included one leaves out too, for the line under the list to count them
   * truly —, top of the list first, and show as
   * they answer. It waits for the typing to settle, like the visible rows
   * wait for the scroll, and stops with the page: nobody reads the list then.
   */
  sweepPings(): void {
    $effect(() => {
      const list = this.#loose;
      if (!this.#wantsPing) {
        stopMeasuring();
        return;
      }
      const timer = setTimeout(() => untrack(() => void measureAll(list)), 400);
      return () => clearTimeout(timer);
    });
    $effect(() => () => stopMeasuring());
  }
}
