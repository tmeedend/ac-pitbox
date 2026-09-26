// The ways into the game folder screen from elsewhere (DOSSIER§3.2), and the
// way out of it towards a sheet (DOSSIER§8.2).
//
// A way in is a state set before navigating and consumed by the screen once
// mounted - like `brandFocus` for the Brands tab: the tab is `nav.section`,
// there is no route parameter to carry it.
import { nav, openInSection, requestSection } from "$lib/shell/nav.svelte";
import type { Owner, OwnerRef } from "./gamestate";

export const gameFolderFocus = $state<{
  /** Pose Provenance on this element. */
  owner: (OwnerRef & { name: string }) | null;
  /** Pose State = drift. */
  drifts: boolean;
}>({ owner: null, drifts: false });

/** "Show in the game folder", from a mod's sheet: the tree pruned to what it
 * lays. */
export async function showInGameFolder(owner: OwnerRef, name: string): Promise<void> {
  gameFolderFocus.owner = { ...owner, name };
  gameFolderFocus.drifts = false;
  if (!(await requestSection("gamefolder"))) gameFolderFocus.owner = null;
}

/** "See the drifts", from the Maintenance tab next door. */
export async function showGameFolderDrifts(): Promise<void> {
  gameFolderFocus.owner = null;
  gameFolderFocus.drifts = true;
  if (!(await requestSection("gamefolder"))) gameFolderFocus.drifts = false;
}

/** Opens the sheet of `owner`, wherever it lives. A pack has none. */
export async function openOwnerSheet(owner: Owner): Promise<void> {
  switch (owner.kind) {
    case "car":
      await openInSection("cars", owner.id);
      return;
    case "track":
      await openInSection("tracks", owner.id);
      return;
    case "app":
      nav.openSheet = { kind: "app", id: owner.id };
      if (!(await requestSection("apps"))) nav.openSheet = null;
      return;
    case "other":
    case "sound":
    case "skin":
    case "trackSkin":
      nav.openSheet = { kind: owner.kind === "trackSkin" ? "skin" : owner.kind, id: owner.id };
      if (!(await requestSection("others"))) nav.openSheet = null;
      return;
    case "pack":
      return;
  }
}

export function hasSheet(owner: Owner): boolean {
  return owner.kind !== "pack";
}
