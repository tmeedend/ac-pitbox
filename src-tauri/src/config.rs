//! Configuration de l'application (§12 de la spec).
//!
//! Fichier de config LOCAL, distinct de la base d'overlay (métadonnées des mods)
//! et de la base de règles (`default-tag-rules.json`). Stocké dans le dossier de
//! config de l'app (`app_config_dir()/config.json`).

use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};
use tauri::{AppHandle, Manager};

/// Préférences persistantes de l'app (§10bis).
/// Présentes dès maintenant pour que le modèle de données puisse les porter ;
/// l'UI dédiée arrivera avec les lots concernés.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct Prefs {
    /// Afficher les tags issus du fichier mod (`tags_from_mod`). Défaut : oui.
    pub show_mod_file_tags: bool,
    /// État global (ouvert/fermé) du panneau de suivi versions/historique.
    pub tracking_panel_open: bool,
    /// Vue bibliothèque par défaut : "gallery" | "table".
    pub library_view: String,
    /// Preset CM graphique/FFB à appliquer au lancement (nom du preset).
    pub default_cm_preset: Option<String>,
    /// Langue forcée par l'utilisateur ("fr", "en"…). `None` = langue système.
    pub language: Option<String>,
    /// Niveau de zoom de l'interface, en % (ex. 125). `None` = 100 (défaut).
    /// Utile sur les écrans haute résolution si la mise à l'échelle Windows
    /// n'est pas correctement reprise par la webview.
    pub ui_zoom: Option<u32>,
    /// Scène utilisée par l'aperçu 3D (`content/showroom/<id>`, nom de dossier).
    /// `None` = `showroom::DEFAULT_SHOWROOM`, la plus légère.
    pub showroom_scene: Option<String>,
    /// Extraction des fichiers annexes du mod à l'import (§4.6) : "none" |
    /// "info_only" (défaut) | "all". Jamais reposée à chaque import — voir
    /// `resources::ExtractionMode::parse`.
    pub resource_extraction_mode: String,
    /// Conservation de l'archive/dossier source d'un mod à l'import (§10/§11),
    /// en plus du contenu extrait en bibliothèque. Défaut : non (cohérent avec
    /// l'absence d'historique de versions/couches). Si activé, rend disponible
    /// l'action « Réinstaller depuis l'archive source » sur la fiche du mod.
    pub keep_source_archive: bool,
}

impl Default for Prefs {
    fn default() -> Self {
        Self {
            show_mod_file_tags: true,
            tracking_panel_open: true,
            library_view: "gallery".into(),
            default_cm_preset: None,
            language: None,
            ui_zoom: None,
            showroom_scene: None,
            resource_extraction_mode: "info_only".into(),
            keep_source_archive: false,
        }
    }
}

/// Chemins + préférences. Tous les chemins sont optionnels tant que la
/// première configuration n'est pas terminée.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(default)]
pub struct AppConfig {
    /// Dossier d'install Assetto Corsa (contient `content/`).
    pub ac_install_path: Option<PathBuf>,
    /// Bibliothèque = source de vérité des fichiers (~300 Go).
    pub library_path: Option<PathBuf>,
    /// Exécutable Content Manager (pour le lancement).
    pub content_manager_exe: Option<PathBuf>,
    /// 7-Zip (extraction rar/zip/7z).
    pub sevenzip_exe: Option<PathBuf>,
    /// QuickBMS — optionnel, requis uniquement pour l'export acd.bms.
    pub quickbms_exe: Option<PathBuf>,
    /// Script acd.bms — idem, optionnel.
    pub acd_bms_script: Option<PathBuf>,
    pub prefs: Prefs,
}

fn config_file(app: &AppHandle) -> Result<PathBuf, String> {
    let dir = app
        .path()
        .app_config_dir()
        .map_err(|e| format!("dossier de config indisponible : {e}"))?;
    Ok(dir.join("config.json"))
}

/// Charge la config ; renvoie une config vide si le fichier n'existe pas
/// encore ou est illisible (premier démarrage).
pub fn load(app: &AppHandle) -> AppConfig {
    let Ok(path) = config_file(app) else {
        return AppConfig::default();
    };
    match std::fs::read_to_string(&path) {
        Ok(s) => serde_json::from_str(&s).unwrap_or_default(),
        Err(_) => AppConfig::default(),
    }
}

/// Persiste la config. Crée le dossier de config au besoin, et crée la
/// bibliothèque si son chemin est renseigné mais le dossier absent.
pub fn save(app: &AppHandle, cfg: &AppConfig) -> Result<(), String> {
    if let Some(lib) = &cfg.library_path {
        if !lib.exists() {
            std::fs::create_dir_all(lib)
                .map_err(|e| format!("impossible de créer la bibliothèque : {e}"))?;
        }
    }
    let path = config_file(app)?;
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent).map_err(|e| e.to_string())?;
    }
    let json = serde_json::to_string_pretty(cfg).map_err(|e| e.to_string())?;
    std::fs::write(&path, json).map_err(|e| format!("écriture config échouée : {e}"))?;
    Ok(())
}

// --- Validation (§12) -------------------------------------------------------

/// Résultat de validation d'un chemin précis.
#[derive(Debug, Clone, Serialize)]
pub struct Check {
    pub ok: bool,
    /// "required" | "optional"
    pub level: String,
    pub message: String,
}

impl Check {
    fn req(ok: bool, message: impl Into<String>) -> Self {
        Self { ok, level: "required".into(), message: message.into() }
    }
    fn opt(ok: bool, message: impl Into<String>) -> Self {
        Self { ok, level: "optional".into(), message: message.into() }
    }
}

#[derive(Debug, Clone, Serialize)]
pub struct ConfigValidation {
    pub ac_install: Check,
    pub content_dir: Check,
    pub content_writable: Check,
    pub library: Check,
    pub content_manager: Check,
    pub sevenzip: Check,
    pub quickbms: Check,
    /// Vrai si tous les `required` sont OK (la config peut être validée).
    pub is_valid: bool,
}

/// Teste l'accès en écriture en créant puis supprimant un fichier sonde.
fn is_writable(dir: &Path) -> bool {
    if !dir.is_dir() {
        return false;
    }
    let probe = dir.join(".pitbox_write_test.tmp");
    match std::fs::File::create(&probe) {
        Ok(_) => {
            let _ = std::fs::remove_file(&probe);
            true
        }
        Err(_) => false,
    }
}

fn is_dir(p: &Option<PathBuf>) -> bool {
    p.as_ref().is_some_and(|p| p.is_dir())
}

fn is_file(p: &Option<PathBuf>) -> bool {
    p.as_ref().is_some_and(|p| p.is_file())
}

// Les messages sont des CLÉS i18n (résolues côté frontend, `src/lib/i18n`),
// pas du texte affichable — la validation reste indépendante de la langue.
pub fn validate(cfg: &AppConfig) -> ConfigValidation {
    // Dossier AC
    let ac_ok = is_dir(&cfg.ac_install_path);
    let ac_install = Check::req(
        ac_ok,
        if ac_ok { "config.acInstallOk" } else { "config.acInstallMissing" },
    );

    // content/
    let content = cfg.ac_install_path.as_ref().map(|p| p.join("content"));
    let content_ok = content.as_ref().is_some_and(|p| p.is_dir());
    let content_dir = Check::req(
        content_ok,
        if content_ok { "config.contentDirOk" } else { "config.contentDirMissing" },
    );

    // content/ accessible en écriture (prérequis aux junctions)
    let writable_ok = content.as_ref().is_some_and(|p| is_writable(p));
    let content_writable = Check::req(
        writable_ok,
        if writable_ok { "config.contentWritableOk" } else { "config.contentWritableMissing" },
    );

    // Bibliothèque : OK si le dossier existe, ou si son parent existe (sera créé à l'enregistrement).
    let lib_ok = is_dir(&cfg.library_path)
        || cfg
            .library_path
            .as_ref()
            .and_then(|p| p.parent())
            .is_some_and(|p| p.is_dir());
    let library = Check::req(
        lib_ok,
        if is_dir(&cfg.library_path) {
            "config.libraryOk"
        } else if lib_ok {
            "config.libraryWillCreate"
        } else {
            "config.libraryInvalid"
        },
    );

    // Content Manager
    let cm_ok = is_file(&cfg.content_manager_exe);
    let content_manager = Check::req(
        cm_ok,
        if cm_ok { "config.cmOk" } else { "config.cmMissing" },
    );

    // 7-Zip
    let sz_ok = is_file(&cfg.sevenzip_exe);
    let sevenzip = Check::req(
        sz_ok,
        if sz_ok { "config.sevenzipOk" } else { "config.sevenzipMissing" },
    );

    // QuickBMS — optionnel (export uniquement). OK si non renseigné OU fichier valide.
    let qb_set = cfg.quickbms_exe.is_some();
    let qb_ok = !qb_set || is_file(&cfg.quickbms_exe);
    let quickbms = Check::opt(
        qb_ok,
        if !qb_set {
            "config.quickbmsUnset"
        } else if qb_ok {
            "config.quickbmsOk"
        } else {
            "config.quickbmsMissing"
        },
    );

    let is_valid = ac_ok && content_ok && writable_ok && lib_ok && cm_ok && sz_ok;

    ConfigValidation {
        ac_install,
        content_dir,
        content_writable,
        library,
        content_manager,
        sevenzip,
        quickbms,
        is_valid,
    }
}
