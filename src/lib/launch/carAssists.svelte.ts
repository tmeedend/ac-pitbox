// ABS and traction control (L5§2.2) — the electronics of the car being
// driven.
//
// **Why they left the settings screen.** They sat in a `DRIVING AIDS` block
// next to damage and tyre wear, among the rules of the session. They are not
// rules: the setting only exists because the car has the hardware, which is
// exactly what the `Factory` line says out loud ("the Audi TT Cup has both ABS
// and traction control"). A setting goes where its subject is, and their
// subject is the car — so they live in the car card of the session column,
// folded under `PERFORMANCE` with the ballast and the restrictor.
//
// **Why a store, like `playerHandicap`.** That card is on screen at all times;
// the settings screen that owns `RaceSetup` is mounted only while it is open.
// Same single direction of travel: this store is the live value, the settings
// screen copies it into `setup`, and only a load (preset, saved session) writes
// back here.
//
// The ideal line does **not** follow: it is not a capability of the car but a
// display aid, and it stays with the rules of the session.

import type { AssistLevel } from "./launch";

export const carAssists = $state<{ abs: AssistLevel; tractionControl: AssistLevel }>({
  abs: "factory",
  tractionControl: "factory",
});

/** Written by a load (the settings screen's own mount, a per-type preset, a
 * saved session) — never by an effect facing the one that reads it. */
export function setCarAssists(abs: AssistLevel, tractionControl: AssistLevel): void {
  carAssists.abs = abs;
  carAssists.tractionControl = tractionControl;
}

/** Whether anything here departs from what the real car has. `PERFORMANCE`
 * summarises itself with this: `Factory` on both is "nothing posed". */
export function assistsTouched(): boolean {
  return carAssists.abs !== "factory" || carAssists.tractionControl !== "factory";
}
