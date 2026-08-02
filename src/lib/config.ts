// Pont typé vers le backend Rust pour la configuration (§12).
import { invoke } from "@tauri-apps/api/core";

export interface Prefs {
  show_mod_file_tags: boolean;
  tracking_panel_open: boolean;
  library_view: "gallery" | "table";
  default_cm_preset: string | null;
  /** Langue forcée ("fr", "en"…) ; `null` = langue système. */
  language: string | null;
  /** Niveau de zoom de l'interface, en % (ex. 125) ; `null` = 100. */
  ui_zoom: number | null;
  /** Scène de l'aperçu 3D (`content/showroom/<id>`) ; `null` = la plus légère. */
  showroom_scene: string | null;
  /** Extraction des fichiers annexes du mod à l'import (§4.6) — jamais reposée
   * à chaque import : "none" | "info_only" (défaut) | "all". */
  resource_extraction_mode: "none" | "info_only" | "all";
  /** Conserve l'archive/dossier source de chaque mod importé, en plus du
   * contenu extrait (§10/§11). Défaut : false. */
  keep_source_archive: boolean;
}

export interface AppConfig {
  ac_install_path: string | null;
  library_path: string | null;
  content_manager_exe: string | null;
  sevenzip_exe: string | null;
  quickbms_exe: string | null;
  acd_bms_script: string | null;
  prefs: Prefs;
}

export interface Check {
  ok: boolean;
  level: "required" | "optional";
  message: string;
}

export interface ConfigValidation {
  ac_install: Check;
  content_dir: Check;
  content_writable: Check;
  library: Check;
  content_manager: Check;
  sevenzip: Check;
  quickbms: Check;
  is_valid: boolean;
}

export function emptyConfig(): AppConfig {
  return {
    ac_install_path: null,
    library_path: null,
    content_manager_exe: null,
    sevenzip_exe: null,
    quickbms_exe: null,
    acd_bms_script: null,
    prefs: {
      show_mod_file_tags: true,
      tracking_panel_open: true,
      library_view: "gallery",
      default_cm_preset: null,
      language: null,
      ui_zoom: null,
      showroom_scene: null,
      resource_extraction_mode: "info_only",
      keep_source_archive: false,
    },
  };
}

// Les champs vides côté UI doivent partir en `null` (pas en chaîne vide).
function clean(cfg: AppConfig): AppConfig {
  const norm = (v: string | null) => (v && v.trim() !== "" ? v.trim() : null);
  return {
    ...cfg,
    ac_install_path: norm(cfg.ac_install_path),
    library_path: norm(cfg.library_path),
    content_manager_exe: norm(cfg.content_manager_exe),
    sevenzip_exe: norm(cfg.sevenzip_exe),
    quickbms_exe: norm(cfg.quickbms_exe),
    acd_bms_script: norm(cfg.acd_bms_script),
  };
}

export function getConfig(): Promise<AppConfig> {
  return invoke<AppConfig>("get_config");
}

export function saveConfig(cfg: AppConfig): Promise<void> {
  return invoke<void>("save_config", { config: clean(cfg) });
}

export function validateConfig(cfg: AppConfig): Promise<ConfigValidation> {
  return invoke<ConfigValidation>("validate_config", { config: clean(cfg) });
}

export interface DetectedPaths {
  ac_install_path: string | null;
  content_manager_exe: string | null;
  sevenzip_exe: string | null;
}

export function autodetectPaths(): Promise<DetectedPaths> {
  return invoke<DetectedPaths>("autodetect_paths");
}
