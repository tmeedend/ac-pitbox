// Pont typé vers le backend Rust pour la configuration (§12).
import { invoke } from "@tauri-apps/api/core";

export interface Prefs {
  tracking_panel_open: boolean;
  library_view: "gallery" | "table";
  default_cm_preset: string | null;
  /** Langue forcée ("fr", "en"…) ; `null` = langue système. */
  language: string | null;
  /** Niveau de zoom de l'interface, en % (ex. 125) ; `null` = 100. */
  ui_zoom: number | null;
  /** Zoom appliqué en plus de `ui_zoom` en mode Big Picture ; `null` = reprend `ui_zoom`. */
  bigpicture_zoom: number | null;
  /** Scène de l'aperçu 3D (`content/showroom/<id>`) ; `null` = la plus légère. */
  showroom_scene: string | null;
  /** Extraction des fichiers annexes du mod à l'import (§4.5.2) — jamais reposée
   * à chaque import : "none" | "info_only" (défaut) | "all". */
  resource_extraction_mode: "none" | "info_only" | "all";
  /** Conserve l'archive/dossier source de chaque mod importé, en plus du
   * contenu extrait (§10/§11). Défaut : false. */
  keep_source_archive: boolean;
  /** Mécanisme de déploiement dans content/ (§2) : "hardlink" (défaut, même
   * disque requis) | "symlink" (junction, tout disque, mode développeur ou
   * élévation requis). Un mod à couche(s) active(s) reste toujours en
   * hardlinks quel que soit ce réglage (une junction ne fusionne pas). */
  deploy_mode: "hardlink" | "symlink";
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
  deploy_mode: Check;
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
      tracking_panel_open: true,
      library_view: "gallery",
      default_cm_preset: null,
      language: null,
      ui_zoom: null,
      bigpicture_zoom: null,
      showroom_scene: null,
      resource_extraction_mode: "info_only",
      keep_source_archive: false,
      deploy_mode: "hardlink",
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
  /** Suggestion de bibliothèque dans le dossier utilisateur — rien n'y existe
   * forcément encore, c'est une proposition de nom, pas une détection. */
  library_path: string | null;
}

export function autodetectPaths(): Promise<DetectedPaths> {
  return invoke<DetectedPaths>("autodetect_paths");
}

/** Ouvre la page Windows « Pour les développeurs » (§2, prérequis symlink). */
export function openDeveloperModeSettings(): Promise<void> {
  return invoke<void>("open_developer_mode_settings");
}
