// Who drives where, to find friends on the Online page (SPEC-play-online.md,
// case 1, "Amis présents"). The lobby names nobody: each busy server is asked
// for its `/JSON` (`online/drivers.rs`), which costs a request per server —
// so the scan only runs once someone has been marked, and again after each
// load of the list.
import { untrack } from "svelte";
import { friendsOnline } from "./lists";
import { serverDrivers, type ServerDrivers, type ServerSummary } from "./online";
import { onlineStore } from "./store.svelte";

export class FriendsScan {
  #servers: () => ServerSummary[];
  /** The list the last scan was started for: a new load scans again. */
  #scannedFor: ServerSummary[] | null = null;

  /** Every busy server that answered, with its drivers. */
  scan = $state<ServerDrivers[]>([]);
  scanning = $state(false);

  /** Per server key, the friends connected there. */
  readonly friends = $derived.by(() => friendsOnline(this.scan, onlineStore()));

  /** `servers`: the list as loaded. */
  constructor(servers: () => ServerSummary[]) {
    this.#servers = servers;
  }

  /** Scans after each load, for the life of the calling component — to call
   * during its initialisation. */
  watch(): void {
    // Every dependency read before the exit (CLAUDE.md): the list, and whether
    // anyone is marked — marking the first friend starts the scan.
    $effect(() => {
      const list = this.#servers();
      const wanted = onlineStore().friends.length > 0;
      if (!wanted || list.length === 0 || this.#scannedFor === list) return;
      this.#scannedFor = list;
      untrack(() => void this.#run(list));
    });
  }

  async #run(list: ServerSummary[]) {
    this.scanning = true;
    try {
      this.scan = await serverDrivers(list.filter((s) => s.clients > 0));
    } catch (e) {
      console.error("online_server_drivers", e);
    } finally {
      this.scanning = false;
    }
  }
}
