//! Persistance des grilles d'adversaires enregistrées (§5) — fichier dédié
//! (`saved_grids.json`), écriture synchrone, même patron que
//! `saved_sessions.rs`.
//!
//! **Une grille et une session sont deux objets, pas deux vues du même.** Une
//! session enregistre tout — duo, météo, heure, options — plus une **copie**
//! de sa grille ; une grille n'enregistre que les adversaires, et c'est
//! précisément ce qui la rend rejouable ailleurs : le même plateau GT3 sur dix
//! circuits. La copie n'est pas une commodité d'implémentation : un lien ferait
//! que modifier une grille changerait en silence toutes les sessions qui la
//! citent.
//!
//! Un objet unique ne suffirait pas non plus dans l'autre sens — appliquer une
//! grille à une session **en cours de configuration**, sans perdre la météo,
//! l'heure et le type de session, est le cas réel, et les presets de grille de
//! Content Manager n'ont d'ailleurs aucune session attachée (§6).
//!
//! Structure opaque côté Rust : le schéma (`SavedGrid`, clé = le nom) appartient
//! au frontend (`savedGrids.ts`). Volontairement **pas** rangé par type de
//! session, contrairement aux sessions enregistrées : une grille ne dépend pas
//! du type de course qu'on va faire avec, c'est tout son intérêt.

use std::path::PathBuf;

use tauri::{AppHandle, Manager};

fn file(app: &AppHandle) -> Option<PathBuf> {
    app.path().app_config_dir().ok().map(|d| d.join("saved_grids.json"))
}

/// Objet vide si le fichier n'existe pas encore ou est illisible — premier
/// démarrage, ou fichier corrompu : jamais bloquant.
pub fn load(app: &AppHandle) -> serde_json::Value {
    let Some(path) = file(app) else {
        return serde_json::json!({});
    };
    match std::fs::read_to_string(&path) {
        Ok(s) => serde_json::from_str(&s).unwrap_or_else(|_| serde_json::json!({})),
        Err(_) => serde_json::json!({}),
    }
}

pub fn save(app: &AppHandle, all: &serde_json::Value) -> Result<(), String> {
    let path = file(app).ok_or("dossier de config indisponible")?;
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent).map_err(|e| e.to_string())?;
    }
    let json = serde_json::to_string_pretty(all).map_err(|e| e.to_string())?;
    std::fs::write(&path, json).map_err(|e| format!("écriture saved_grids.json échouée : {e}"))
}
