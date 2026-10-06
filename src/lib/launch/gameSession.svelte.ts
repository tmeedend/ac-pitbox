// Is a session running? (SESSION§2.4) One answer for the whole app: the
// launch button of the session column and the banner of the settings screen
// read it, and neither may start a second game while one runs.
//
// The banner used to say "sent to Content Manager, the game is starting" and
// stay there: it reported that the request had left, then nothing — not the
// game starting, not the game ending, and the button stayed live under it.
// Pit Box has watched the game's process from the start (`music/watch.rs`,
// `ac://running`), so the state follows the game itself:
//
// - `starting`: the request went to CM, the game is not seen yet. Measured,
//   CM takes 0.8 to 4.1 s to start `acs.exe` (`online/session_layers.rs`).
//   Past `STARTING_FOR_MS` the wait ends on `stalled` — a CM dialog, a refusal
//   — and the button comes back: the backend refuses a second session anyway
//   if the game shows up meanwhile.
// - `running`: the game's process is there, whoever started it — a session
//   launched from Content Manager itself is one too.
// - `idle`: neither.
import { invoke } from "@tauri-apps/api/core";
import { onAcRunning } from "./launch";

export type GamePhase = "idle" | "starting" | "running";

/** Thirty times CM's measured worst; a cold CM or a slow disk still fits. */
const STARTING_FOR_MS = 30_000;

export const gameSession = $state<{ phase: GamePhase; stalled: boolean }>({ phase: "idle", stalled: false });

let startingTimer: ReturnType<typeof setTimeout> | null = null;

function clearStarting() {
  if (startingTimer) clearTimeout(startingTimer);
  startingTimer = null;
}

function setRunning(running: boolean) {
  clearStarting();
  gameSession.stalled = false;
  gameSession.phase = running ? "running" : "idle";
}

/** A launch request has just left for Content Manager. */
export function markStarting(): void {
  if (gameSession.phase === "running") return;
  clearStarting();
  gameSession.phase = "starting";
  gameSession.stalled = false;
  startingTimer = setTimeout(() => {
    startingTimer = null;
    if (gameSession.phase !== "starting") return;
    gameSession.phase = "idle";
    gameSession.stalled = true;
  }, STARTING_FOR_MS);
}

/** Whether a new session may be sent now. */
export function canStartSession(): boolean {
  return gameSession.phase === "idle";
}

/**
 * Follows the game for the life of the window; returns its stop.
 *
 * Listens first, then asks: the watch announces its first state before the
 * window exists, so that one is lost, and an announcement arriving while the
 * question is out is newer than its answer.
 */
export function startGameSessionWatch(): () => void {
  let heard = false;
  const unlisten = onAcRunning((running) => {
    heard = true;
    setRunning(running);
  });
  void unlisten
    .then(() => invoke<boolean>("is_game_running"))
    .then((running) => {
      if (!heard) setRunning(running);
    })
    .catch((e) => console.warn("is_game_running", e));
  return () => {
    clearStarting();
    void unlisten.then((stop) => stop());
  };
}
