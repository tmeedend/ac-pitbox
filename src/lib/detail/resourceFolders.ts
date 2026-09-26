// Folders in the resources of a mod, app or pack (§4.6ter): where a proposed
// folder answered "keep" lands, under its own name — and where the user takes
// it back. Removing one is also the answer for the next import of the mod: the
// folder offered again is not imported.
import { invoke } from "@tauri-apps/api/core";

/** Mirrors `pending::KeptFolder`. */
export interface ResourceFolder {
  name: string;
  file_count: number;
  size_bytes: number;
}

/** Whose resources: a car or track (`mod`), an app, a pack. */
export type ResourceFolderSource = "mod" | "app" | "pack";

export function listResourceFolders(id: string, source: ResourceFolderSource): Promise<ResourceFolder[]> {
  return invoke<ResourceFolder[]>("list_resource_folders", { id, source });
}

export function removeResourceFolder(id: string, source: ResourceFolderSource, name: string): Promise<void> {
  return invoke<void>("remove_resource_folder", { id, source, name });
}
