// Favourites and recent joins, persisted in `online.json` by the Rust side
// (`online/store.rs`, golden rule 6). Loaded once, written whole on every
// change through `durableWriter`: retried once, then shown by `PrefsToast`.
import { invoke } from "@tauri-apps/api/core";
import { durableWriter, type WriteFailure } from "$lib/durableWrite.svelte";
import { EMPTY_STORE, parseStore, recordJoin, toggleFavourite, type OnlineStore } from "./lists";
import type { ServerSummary } from "./online";

const state = $state<{ store: OnlineStore }>({ store: EMPTY_STORE });
let loading: Promise<void> | null = null;
/** Whether the file was read. Until it is, nothing is written: a write sends
 * the whole store, so writing after a failed read would replace the user's
 * favourites with the empty list this module starts from. */
let read = false;

const writer = durableWriter<OnlineStore>("save_online_store", (store) => invoke<void>("save_online_store", { store }));

/** Reads `online.json` the first time it is asked, and never again: after
 * that, the state here is the truth and the file follows it. */
export function loadOnlineStore(): Promise<void> {
  loading ??= invoke<unknown>("get_online_store")
    .then((raw) => {
      state.store = parseStore(raw);
      read = true;
    })
    .catch((e) => {
      console.error("get_online_store", e);
      // Asked again next time rather than never.
      loading = null;
    });
  return loading;
}

export function onlineStore(): OnlineStore {
  return state.store;
}

function update(next: OnlineStore): void {
  state.store = next;
  if (read) writer.save($state.snapshot(state.store));
  else console.error("online.json was never read: change kept on screen, not written");
}

export function toggleFavouriteServer(server: ServerSummary): void {
  update(toggleFavourite(state.store, $state.snapshot(server)));
}

export function recordRecentJoin(server: ServerSummary, car: string): void {
  update(recordJoin(state.store, $state.snapshot(server), car, new Date().toISOString()));
}

export function onlineStoreWriteFailure(): WriteFailure {
  return writer.failure;
}
