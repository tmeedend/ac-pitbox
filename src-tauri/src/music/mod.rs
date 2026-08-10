//! Module Musique du mode Big Picture (`docs/spec-module-musique_2.md`).
//!
//! Périmètre de cette première implémentation ("noyau du module", décidé avec
//! l'utilisateur) : moteur audio à deux ambiances avec crossfade, détection du
//! lancement d'AC, coupure/duck de session, sélection de dossier par
//! Parcourir, normalisation RMS + cache d'index (§3.4, `index.rs`).
//! Pack par défaut embarqué (§16.1, deux pistes Pixabay Content License,
//! crédits dans `assets/music/CREDITS.md`) déposé dans les dossiers par
//! défaut à leur création (`config::ensure_default_dirs`) — l'utilisateur
//! reste libre de les remplacer ou de les vider, jamais réécrits ensuite.
//! **Écartée pour de bon** (pas juste reportée, décidé avec l'utilisateur) :
//! la détection automatique des bandes-son Steam (§3.2) — le sélecteur de
//! dossier par Parcourir suffit.
//!
//! La spec cible NAudio/C# ; ce module est sa transposition Rust avec
//! `rodio` (voir `engine.rs` pour le détail des écarts assumés).

#[cfg(windows)]
pub mod ac_status;
#[cfg(not(windows))]
pub mod ac_status {
    //! Repli non-Windows : jamais exécuté pour de vrai (l'app cible Windows
    //! uniquement, §16.4), garde seulement le reste du crate compilable
    //! ailleurs (ex. build frontend en CI, qui n'appelle pas `cargo build`
    //! mais partage le même arbre de sources).
    pub fn is_live() -> bool {
        false
    }
}
pub mod config;
pub mod engine;
pub mod index;
pub mod scan;
pub mod watch;

pub use config::MusicConfig;
pub use engine::{EngineCommand, MusicEngineHandle, PreviewHandle};
