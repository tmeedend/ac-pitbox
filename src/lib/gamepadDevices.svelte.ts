// Which devices are allowed to drive the interface (§7.4), and the one
// question that decides it.
//
// A device drives the UI only if the user designated it, once, explicitly.
// Without an answer it drives nothing — closed by default: a mute device is
// diagnosable (Settings says so), a focus that drifts on its own has no
// obvious recourse. `mapping === "standard"` is *declared* by the device, not
// verified: a wheel in "Xbox mode" or behind an XInput adapter announces
// itself as standard, and the Xbox layout maps axis 1 to up/down — which on a
// wheel is a pedal. Brushing the brake used to scroll the focus with nothing
// on screen explaining it.
//
// Start-up, hot-plug and first install are the *same* event — a visible device
// with no recorded decision — so there is one code path, not three.
//
// Persistence goes through `ui_prefs.json` (golden rule n°6): `localStorage`
// is not guaranteed to hit the disk under WebView2, and a decision that does
// not survive a restart is a question asked again at every launch, which is
// worse than the automatic mode it replaces.

import { getUiPref, peekUiPref, setUiPref, removeUiPref } from "$lib/uiPrefs.svelte";
import { deviceKey, isSameFamily, type DeviceRecord, type NavProfile } from "$lib/gamepadProfile";

/** Global kill switch. Kept separate from the per-device decisions on purpose:
 *  switching gamepad navigation off must not erase what the user answered. */
export const GAMEPAD_ENABLED_KEY = "pitbox.gamepad.enabled";
export const GAMEPAD_DEVICES_KEY = "pitbox.gamepad.devices";
/** Read one last time at start-up, then dropped — see `migrateLegacyMode`. */
const LEGACY_MODE_KEY = "pitbox.gamepadNav.mode";

export interface LiveDevice {
  /** Slot, valid for this frame only — never persisted, never a key. */
  index: number;
  id: string;
  key: string;
  mapping: string;
  axes: number[];
  /** Indices of the buttons currently pressed (technical details panel). */
  pressed: number[];
  buttonCount: number;
  /** `Gamepad.timestamp` only moves on a state change: this is what tells
   *  "asleep" from "gone" in the diagnostics. */
  timestamp: number;
}

/** How often the live list is rebuilt. A diagnostics table read by eye does
 *  not need 60 Hz, and this list is rebuilt from scratch each time. */
const REFRESH_MS = 150;

/** A full rig (base, pedals, handbrake, shifter, button box) enumerates half a
 *  dozen entries within a few hundred ms. Waiting for the burst to settle is
 *  what makes the banner say "5 new devices" once instead of appearing five
 *  times with a wrong count. */
const GROUP_MS = 1000;

export const controllers = $state<{
  live: LiveDevice[];
  /** The setup panel is opened by a click — never on its own (§7.4). */
  setupOpen: boolean;
  /** "Later": answers nothing, the banner comes back at the next start-up.
   *  Never worth a refusal. Not persisted, deliberately. */
  bannerDismissed: boolean;
  /** False while the discovery burst is still growing. */
  settled: boolean;
  /** Device the panel should open straight into calibration for (Settings'
   *  `[Calibrate]`), instead of showing the list. */
  calibrateKey: string | null;
}>({
  live: [],
  setupOpen: false,
  bannerDismissed: false,
  settled: false,
  calibrateKey: null,
});

/** Nothing opens on its own: a modal is only justified when the app cannot go
 *  on without an answer, and here it can (closed by default, nothing moves).
 *  A wheel also gets plugged in *just before* starting a session — a popup
 *  would land at the worst possible moment, every time. */
export function openControllerSetup(calibrateKey: string | null = null): void {
  controllers.calibrateKey = calibrateKey;
  controllers.setupOpen = true;
}

// --- Records -------------------------------------------------------------

// Parsed lazily from the raw string, and only when that string changes: the
// navigation poll reads this every frame (`peekUiPref`, never the async API —
// golden rule n°6), and re-parsing JSON 60 times a second for nothing is the
// kind of waste that shows up as a stutter on a low-end machine.
let rawRecords: string | null = null;
let parsedRecords: Record<string, DeviceRecord> = {};
// `setUiPref` ne met le cache à jour qu'après un `await` : entre l'écriture et
// son atterrissage, `peekUiPref` renvoie encore l'ANCIENNE valeur. Sans ce
// garde, la lecture qui suit immédiatement une décision reparserait ce vieux
// texte et effacerait la décision qu'on vient de prendre, le temps d'un rendu.
let pendingRaw: string | null = null;

export function deviceRecords(): Record<string, DeviceRecord> {
  const raw = peekUiPref(GAMEPAD_DEVICES_KEY);
  if (raw === rawRecords) return parsedRecords;
  if (pendingRaw !== null) {
    if (raw !== pendingRaw) return parsedRecords;
    pendingRaw = null;
  }
  rawRecords = raw;
  parsedRecords = parseRecords(raw);
  return parsedRecords;
}

function parseRecords(raw: string | null): Record<string, DeviceRecord> {
  if (!raw) return {};
  try {
    const parsed = JSON.parse(raw) as Record<string, DeviceRecord>;
    return parsed && typeof parsed === "object" ? parsed : {};
  } catch {
    // A corrupt entry must not take gamepad navigation down with it: an empty
    // set simply means "nothing answered yet", which the banner handles.
    console.warn("gamepad devices: unreadable record, starting over");
    return {};
  }
}

function writeRecords(next: Record<string, DeviceRecord>): void {
  rawRecords = JSON.stringify(next);
  pendingRaw = rawRecords;
  parsedRecords = next;
  setUiPref(GAMEPAD_DEVICES_KEY, rawRecords);
}

export function gamepadEnabled(): boolean {
  return peekUiPref(GAMEPAD_ENABLED_KEY) !== "false";
}

export function setGamepadEnabled(on: boolean): void {
  setUiPref(GAMEPAD_ENABLED_KEY, on ? "true" : "false");
}

function record(key: string, label: string, use: boolean, profile?: NavProfile): DeviceRecord {
  return { key, label, use, profile, answeredAt: new Date().toISOString() };
}

/** Answers for a whole listed set at once — the panel asks "which one", so
 *  every device it listed is answered, not just the chosen one. Siblings of
 *  the chosen device (same vendor/model prefix, e.g. a wheel base and its
 *  button box) are adopted along with it: they are the same wheel. */
export function answerDevices(listed: { key: string; id: string }[], chosenKey: string | null): void {
  const chosen = chosenKey ? listed.find((d) => d.key === chosenKey) : null;
  const next = { ...deviceRecords() };
  for (const d of listed) {
    const use = !!chosen && (d.key === chosen.key || isSameFamily(d.id, chosen.id));
    // Keep an existing calibrated profile: answering the question again must
    // not throw away two minutes of calibration.
    next[d.key] = { ...record(d.key, d.id, use), profile: next[d.key]?.profile };
  }
  writeRecords(next);
}

export function setDeviceUse(key: string, label: string, use: boolean): void {
  const next = { ...deviceRecords() };
  next[key] = { ...record(key, label, use), profile: next[key]?.profile };
  writeRecords(next);
}

export function saveDeviceProfile(key: string, label: string, profile: NavProfile): void {
  const next = { ...deviceRecords() };
  next[key] = { ...record(key, label, true, profile) };
  writeRecords(next);
}

/** Back to "never asked" — the "I got it wrong" button, without which a wrong
 *  answer is final. */
export function forgetDevice(key: string): void {
  const next = { ...deviceRecords() };
  delete next[key];
  writeRecords(next);
}

// --- Detection -----------------------------------------------------------

function snapshot(): LiveDevice[] {
  const out: LiveDevice[] = [];
  for (const gp of navigator.getGamepads?.() ?? []) {
    // `getGamepads()` returns a snapshot with holes, and the objects in it go
    // stale: it is re-read every time, never kept between frames.
    if (!gp?.connected) continue;
    out.push({
      index: gp.index,
      id: gp.id,
      key: deviceKey(gp.id),
      mapping: gp.mapping || "none",
      axes: Array.from(gp.axes),
      pressed: gp.buttons.map((b, i) => (b.pressed ? i : -1)).filter((i) => i >= 0),
      buttonCount: gp.buttons.length,
      timestamp: gp.timestamp,
    });
  }
  return out;
}

/** Devices seen right now that have never been answered — deduplicated by key,
 *  since a wheel can present the same key on two entries. */
export function pendingDevices(): LiveDevice[] {
  const records = deviceRecords();
  const seen = new Set<string>();
  return controllers.live.filter((d) => {
    if (records[d.key] || seen.has(d.key)) return false;
    seen.add(d.key);
    return true;
  });
}

/** The banner shows only once the discovery burst has settled, and never while
 *  the panel it opens is already open. */
export function bannerVisible(): boolean {
  if (!gamepadEnabled() || controllers.setupOpen || controllers.bannerDismissed) return false;
  return controllers.settled && pendingDevices().length > 0;
}

let migrated = false;

/** `pitbox.gamepadNav.mode` (the old automatic/off/forced-id setting), read one
 *  last time and dropped:
 *   - `off`      → global kill switch off, decisions untouched;
 *   - `forced:id`→ that device adopted, without asking anything;
 *   - `auto`/``  → no decision at all, the banner will show up. Costs current
 *                  gamepad users one click — mentioned in the release notes. */
async function migrateLegacyMode(): Promise<void> {
  const mode = await getUiPref(LEGACY_MODE_KEY);
  if (mode == null) return;
  if (mode === "off") {
    setGamepadEnabled(false);
  } else if (mode && mode !== "auto") {
    // The old setting stored the exact `Gamepad.id`, which is all `deviceKey`
    // needs — no need for the device to be plugged in for this to work.
    const key = deviceKey(mode);
    const next = { ...deviceRecords() };
    if (!next[key]) next[key] = record(key, mode, true);
    writeRecords(next);
  }
  removeUiPref(LEGACY_MODE_KEY);
}

/** Starts watching for devices. One loop, mounted once in `AppShell`; returns
 *  a stop function.
 *
 *  `requestAnimationFrame` and not `setInterval`: observed empirically under
 *  WebView2 — a `Gamepad` read outside a rAF loop stays frozen. Note that a
 *  device does not exist until it has been touched (Chromium hides it until
 *  the first input, anti-fingerprinting), so this list can legitimately be
 *  empty with a wheel plugged in and switched on. */
export function startControllerWatch(): () => void {
  let raf = 0;
  let lastRefresh = 0;
  let lastChange = 0;
  let signature = "";
  void migrateLegacyMode().finally(() => (migrated = true));

  function tick(now: number) {
    if (now - lastRefresh >= REFRESH_MS) {
      lastRefresh = now;
      controllers.live = snapshot();
      const next = pendingDevices()
        .map((d) => d.key)
        .sort()
        .join("|");
      if (next !== signature) {
        signature = next;
        controllers.settled = false;
        lastChange = now;
        // A device that appears after a "later" click is a new question, not
        // the one that was postponed: let the banner speak again.
        controllers.bannerDismissed = false;
      } else if (!controllers.settled && migrated && now - lastChange >= GROUP_MS) {
        controllers.settled = true;
      }
    }
    raf = requestAnimationFrame(tick);
  }

  raf = requestAnimationFrame(tick);
  return () => cancelAnimationFrame(raf);
}
