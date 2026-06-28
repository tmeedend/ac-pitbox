// Pont typé vers les profils (§7).
import { invoke } from "@tauri-apps/api/core";

export interface ProfileRow {
  id: string;
  name: string;
  entry_count: number;
}

export interface ApplyReport {
  activated: number;
  deactivated: number;
  errors: string[];
}

export function listProfiles(): Promise<ProfileRow[]> {
  return invoke<ProfileRow[]>("list_profiles");
}

export function createProfile(name: string): Promise<string> {
  return invoke<string>("create_profile", { name });
}

export function applyProfile(id: string): Promise<ApplyReport> {
  return invoke<ApplyReport>("apply_profile", { id });
}

export function deleteProfile(id: string): Promise<void> {
  return invoke<void>("delete_profile", { id });
}
