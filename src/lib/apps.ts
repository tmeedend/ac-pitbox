// Pont typé vers les commandes Apps (§12bis.4).
import { invoke } from "@tauri-apps/api/core";

export interface AppItem {
  id: string;
  source_archive: string | null;
  imported_at: string;
  active: boolean;
}

export function listApps(): Promise<AppItem[]> {
  return invoke<AppItem[]>("list_apps");
}

export function activateApp(id: string): Promise<void> {
  return invoke<void>("activate_app", { id });
}

export function deactivateApp(id: string): Promise<void> {
  return invoke<void>("deactivate_app", { id });
}
