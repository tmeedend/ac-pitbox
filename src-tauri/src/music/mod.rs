//! Module Musique (`docs/spec-module-musique_2.md`, SPEC §16).
//!
//! Moteur audio à N ambiances avec crossfade (`engine.rs`) — une ambiance par
//! source, dossier ou fichier —, détection du lancement d'AC et coupure de
//! session (`watch.rs`, `ac_status.rs`), normalisation RMS et cache d'index
//! (`index.rs`), bandes-son Steam proposées comme raccourcis (`steam.rs`).
//! Pack par défaut embarqué (§16.1, deux pistes Pixabay Content License,
//! crédits dans `assets/music/CREDITS.md`).
//!
//! **Ce qui joue sur quel écran se décide côté front** (`src/lib/music/`) :
//! la résolution a besoin de la fiche affichée et de l'ancre posée par l'index
//! de la bibliothèque, que le backend ne voit pas. Le moteur reçoit le
//! résultat (`engine::Context`) et l'applique.
//!
//! La spec cible NAudio/C# ; ce module est sa transposition Rust avec
//! `rodio` (voir `engine.rs` pour le détail des écarts assumés).

#[cfg(windows)]
pub mod ac_status;
#[cfg(not(windows))]
pub mod ac_status {
    //! Repli non-Windows : jamais exécuté pour de vrai (l'app cible Windows
    //! uniquement), garde seulement le reste du crate compilable ailleurs.
    pub fn is_live() -> bool {
        false
    }
}
pub mod config;
pub mod engine;
pub mod index;
pub mod scan;
pub mod steam;
pub mod tags;
pub mod watch;

pub use config::MusicConfig;
pub use engine::{EngineCommand, MusicEngineHandle, PreviewHandle};
