// A generic "the library may have changed" signal: activation, deactivation,
// deletion, import. Open views subscribe to it to resync without having to
// know WHO changed WHAT — the same mechanism as `importState.version` before
// it, which only covered the import.
//
// Real bug fixed by this generalisation: deactivating a mod from its sheet
// only refreshed the session column's "mod deactivated" warning
// (`SessionColumn.svelte`) on the next change of selection — the effect that
// loads it was subscribed to the chosen mod's id only, never to its
// activation, which can change while the id stays put.
import { untrack } from "svelte";

let value = $state(0);

export function libraryVersion(): number {
  return value;
}

export function bumpLibraryVersion(): void {
  value++;
}

/** Calls `reload` on every library change after the caller was created —
 * never for the state it was created with, which it loaded itself. For a
 * view that loads at mount and must only re-read afterwards; a view that
 * loads *through* its effect reads `libraryVersion()` there instead.
 *
 * Must be called while a component initialises (it creates an `$effect`).
 * `reload` runs untracked: what it reads does not re-run it, so a guard
 * inside it (`if (detail) …`) skips a change rather than waiting for the
 * guard to open — the change is then simply absorbed. Three screens wrote
 * this by hand, each with its own bookkeeping, in the very place where an
 * early `return` in an `$effect` silently drops a dependency (CLAUDE.md, « Un
 * $effect ne s'abonne qu'à ce qu'il a lu »). */
export function onLibraryChange(reload: () => void): void {
  let seen = untrack(() => value);
  $effect(() => {
    const version = value;
    if (version === seen) return;
    seen = version;
    untrack(reload);
  });
}
