//! Statut en piste d'Assetto Corsa, via sa mémoire partagée officielle
//! (`Local\acpmf_graphics`) — l'API que tous les tableaux de bord tiers
//! (SimHub, CrewChief…) utilisent déjà pour ça, pas une astuce maison.
//!
//! Pourquoi pas les logs (piste envisagée initialement) : leur format n'est
//! pas stable d'une version à l'autre et les lire coûte de l'I/O disque à
//! chaque scrutation — sensible pendant la course, justement le moment où on
//! scrute le plus. Une lecture de mémoire partagée est de l'ordre de la
//! microseconde et ne touche jamais le disque.
//!
//! `acs.exe` exporte trois mappings (`_physics`, `_graphics`, `_static`) dès
//! son initialisation, y compris pendant l'écran de chargement — seul
//! `_graphics.Status` nous intéresse ici. Struct de référence (SDK AC,
//! `Pack = 4`) :
//! ```text
//! struct Graphics {
//!     int32 PacketId;   // offset 0
//!     int32 Status;     // offset 4  <- AC_STATUS : OFF=0, REPLAY=1, LIVE=2, PAUSE=3
//!     int32 Session;    // offset 8
//!     ...               // ~800 octets au total, sans intérêt ici
//! }
//! ```
//! On ne mappe que les 8 premiers octets : pas besoin du reste.

use windows::core::w;
use windows::Win32::Foundation::CloseHandle;
use windows::Win32::System::Memory::{MapViewOfFile, OpenFileMappingW, UnmapViewOfFile, FILE_MAP_READ};

const AC_LIVE: i32 = 2;
const HEADER_BYTES: usize = 8;

/// `None` tant que le mapping n'existe pas (AC pas encore initialisé, ou
/// déjà refermé) — jamais une erreur, c'est l'état attendu la majorité du
/// temps où l'app tourne.
fn read_status() -> Option<i32> {
    unsafe {
        let handle = OpenFileMappingW(FILE_MAP_READ.0, false, w!("Local\\acpmf_graphics")).ok()?;
        let view = MapViewOfFile(handle, FILE_MAP_READ, 0, 0, HEADER_BYTES);
        let status = if view.Value.is_null() {
            None
        } else {
            let bytes = std::slice::from_raw_parts(view.Value as *const u8, HEADER_BYTES);
            Some(i32::from_ne_bytes(bytes[4..8].try_into().expect("4-byte slice")))
        };
        if !view.Value.is_null() {
            let _ = UnmapViewOfFile(view);
        }
        let _ = CloseHandle(handle);
        status
    }
}

/// Vrai uniquement quand la voiture est réellement pilotable sur la piste —
/// faux pendant le chargement, en pause, ou en replay (§16.4). C'est ce
/// signal, pas le lancement du process, qui doit couper/baisser la musique.
pub fn is_live() -> bool {
    read_status() == Some(AC_LIVE)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn is_live_is_false_without_a_running_ac_process() {
        // Pas de vrai `acs.exe` sur la machine de test : le mapping n'existe
        // pas, `is_live` doit répondre faux plutôt que planter/paniquer.
        assert!(!is_live());
    }
}
