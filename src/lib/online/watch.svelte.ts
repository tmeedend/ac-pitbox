// The background watch of the Online page (SPEC-play-online.md, v2: "Prévenir
// quand un ami se connecte ou qu'un slot se libère"). Started with the shell,
// so it runs whatever screen is open.
//
// - **Friends**: the `/JSON` of the favourites and recent joins only, one
//   request each every two minutes (`watchRules.watchedServers`).
// - **A free slot**: only on the server where the user asked "Notify me",
//   every 30 s; it ends when the slot frees, after an hour, or when the game
//   starts.
// - Paused while the game runs (`ac://running`): the user is driving, and a
//   request a second to a server does not belong in a race.
// - Switched off whole in Settings › General (`StorageKey.onlineWatch`).
//
// An alert lands in the app's notification stack; when the window is not in
// front, Windows shows it too — the stack only speaks to whoever looks at it.
import { isPermissionGranted, requestPermission, sendNotification } from "@tauri-apps/plugin-notification";
import { t } from "$lib/i18n/index.svelte";
import { onAcRunning } from "$lib/launch/launch";
import { StorageKey } from "$lib/storage";
import { peekUiPref } from "$lib/uiPrefs.svelte";
import { friendsOnline } from "./lists";
import { serverDrivers, serverKey, slotCounts, type ServerSummary } from "./online";
import { loadOnlineStore, onlineStore } from "./store.svelte";
import { newSightings, slotFreed, watchedServers } from "./watchRules";

const FRIENDS_EVERY_MS = 2 * 60 * 1000;
/** The first round only sets what is already there: a friend online when
 * Pit Box starts has not just connected. */
const FIRST_ROUND_MS = 15 * 1000;
const SLOT_EVERY_MS = 30 * 1000;
const SLOT_FOR_MS = 60 * 60 * 1000;

export type WatchAlert =
  | { id: number; kind: "friend"; name: string; server: ServerSummary }
  | { id: number; kind: "slot"; car: string | null; server: ServerSummary };

interface SlotWatch {
  server: ServerSummary;
  /** The car the slot is awaited for; `null` for any. */
  car: string | null;
  since: number;
}

const state = $state<{ slot: SlotWatch | null; alerts: WatchAlert[] }>({ slot: null, alerts: [] });
let nextId = 1;
let running = false;
/** Friends seen at the last round, per server; `null` before the first. */
let baseline: Record<string, string[]> | null = null;

/** Whether the background watch is on (Settings › General). On by default. */
export function onlineWatchOn(): boolean {
  return peekUiPref(StorageKey.onlineWatch) !== "0";
}

/** The slot being awaited, if any. Reactive. */
export function slotWatch(): SlotWatch | null {
  return state.slot;
}

/** The alerts not dismissed yet. Reactive. */
export function watchAlerts(): WatchAlert[] {
  return state.alerts;
}

export function dismissAlert(id: number): void {
  state.alerts = state.alerts.filter((a) => a.id !== id);
}

/** "Notify me": waits for a slot on `server`, for `car` or any. Replaces the
 * previous one — one server watched at a time. */
export function watchSlot(server: ServerSummary, car: string | null): void {
  state.slot = { server: $state.snapshot(server) as ServerSummary, car, since: Date.now() };
  void checkSlot();
}

export function stopSlotWatch(): void {
  state.slot = null;
}

function alertText(alert: WatchAlert): { title: string; body: string } {
  return alert.kind === "friend"
    ? { title: t("online.watchFriend", { name: alert.name }), body: alert.server.name }
    : { title: t("online.watchSlot"), body: alert.server.name };
}

/** Into the stack, and to Windows when the window is not in front. */
async function deliver(alert: WatchAlert): Promise<void> {
  state.alerts = [...state.alerts, alert];
  if (document.hasFocus()) return;
  try {
    const granted = (await isPermissionGranted()) || (await requestPermission()) === "granted";
    if (granted) sendNotification(alertText(alert));
  } catch (e) {
    console.warn("online watch: Windows notification failed", e);
  }
}

async function checkFriends(): Promise<void> {
  if (!onlineWatchOn() || running) return;
  await loadOnlineStore();
  const store = onlineStore();
  const servers = watchedServers(store);
  if (store.friends.length === 0 || servers.length === 0) {
    baseline = null;
    return;
  }
  try {
    const now = friendsOnline(await serverDrivers(servers), store);
    if (baseline && !running) {
      for (const seen of newSightings(baseline, now)) {
        const server = servers.find((s) => serverKey(s) === seen.key);
        if (server) void deliver({ id: nextId++, kind: "friend", name: seen.name, server });
      }
    }
    baseline = now;
  } catch (e) {
    console.warn("online watch: friends round failed", e);
  }
}

async function checkSlot(): Promise<void> {
  const watch = state.slot;
  if (!watch || running || !onlineWatchOn()) return;
  if (Date.now() - watch.since > SLOT_FOR_MS) {
    stopSlotWatch();
    return;
  }
  try {
    const counts = await slotCounts(watch.server.ip, watch.server.http_port);
    // Stopped, replaced or the game started while the server answered.
    if (state.slot !== watch || running) return;
    if (slotFreed(counts, watch.car)) {
      stopSlotWatch();
      void deliver({ id: nextId++, kind: "slot", car: watch.car, server: watch.server });
    }
  } catch (e) {
    console.warn("online watch: slot round failed", e);
  }
}

/** Starts the watch for the life of the window; returns its stop. */
export function startOnlineWatch(): () => void {
  const first = setTimeout(() => void checkFriends(), FIRST_ROUND_MS);
  const friends = setInterval(() => void checkFriends(), FRIENDS_EVERY_MS);
  const slot = setInterval(() => void checkSlot(), SLOT_EVERY_MS);
  const stopListening = onAcRunning((now) => {
    running = now;
    if (!now) return;
    // The game started: the awaited slot is moot, and friends seen before
    // the session say nothing of who arrives after it.
    stopSlotWatch();
    baseline = null;
  });
  return () => {
    clearTimeout(first);
    clearInterval(friends);
    clearInterval(slot);
    void stopListening.then((unlisten) => unlisten());
  };
}
