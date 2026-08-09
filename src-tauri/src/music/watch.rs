//! Détection du lancement d'Assetto Corsa et de la fin du chargement (§16.4
//! du SPEC) : polling léger toutes les 500ms pour la présence du process
//! (`acs.exe`/`AssettoCorsa.exe`, `sysinfo`), toutes les 1000ms pour le
//! statut "en piste" (mémoire partagée AC, `ac_status::is_live`) une fois le
//! process détecté — une simple lecture mémoire, sans coût mesurable même
//! pendant la course.
//!
//! Distinction volontaire entre les deux : le process tourne dès l'écran de
//! chargement, mais la musique de préparation de course (GRID) doit
//! continuer à jouer jusqu'à ce que la voiture soit réellement pilotable.
//! C'est `AC_LIVE` (pas le lancement du process) qui déclenche la coupure.

use std::sync::mpsc::Sender;
use std::time::Duration;

use sysinfo::{ProcessesToUpdate, System};

use super::ac_status;
use super::engine::EngineCommand;

const PROCESS_NAMES: [&str; 2] = ["acs.exe", "assettocorsa.exe"];

fn ac_running(sys: &System) -> bool {
    sys.processes().values().any(|p| {
        let name = p.name().to_string_lossy();
        PROCESS_NAMES.iter().any(|n| name.eq_ignore_ascii_case(n))
    })
}

/// Démarre le thread de surveillance, pour toute la durée de vie de l'app
/// (coût négligeable) — y compris quand Big Picture n'est pas actif, le
/// moteur ignore lui-même les commandes hors état pertinent.
pub fn spawn(tx: Sender<EngineCommand>) {
    std::thread::spawn(move || {
        let mut sys = System::new_all();
        sys.refresh_processes(ProcessesToUpdate::All, true);
        let mut running = ac_running(&sys);
        // Reprise d'état (§4/§16.4) : le process peut déjà tourner au
        // démarrage de ce thread ; le statut "en piste" n'est en revanche
        // jamais annoncé rétroactivement ici (rien ne jouait encore côté
        // moteur tant que Big Picture n'a pas été ouvert, `AcProcessStarted`
        // suffit comme garde), juste mémorisé pour détecter la transition.
        let mut live = running && ac_status::is_live();
        if running && tx.send(EngineCommand::AcProcessStarted).is_err() {
            return; // canal fermé : l'app se ferme, plus rien à surveiller
        }
        loop {
            std::thread::sleep(Duration::from_millis(if running { 1000 } else { 500 }));
            sys.refresh_processes(ProcessesToUpdate::All, true);
            let now_running = ac_running(&sys);
            if now_running != running {
                let cmd = if now_running { EngineCommand::AcProcessStarted } else { EngineCommand::AcProcessStopped };
                if tx.send(cmd).is_err() {
                    return;
                }
                running = now_running;
                if !running {
                    live = false;
                }
            }
            if running {
                let now_live = ac_status::is_live();
                if now_live != live {
                    let cmd = if now_live { EngineCommand::EnterSession } else { EngineCommand::ExitSession };
                    if tx.send(cmd).is_err() {
                        return;
                    }
                    live = now_live;
                }
            }
        }
    });
}
