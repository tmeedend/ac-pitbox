//! Moteur audio (§4, §5 de la spec) : deux lecteurs qui se relaient pour le
//! crossfade, une machine à états MENU/GRID/SESSION, une piste par ambiance
//! (mémorisant sa position pour un retour agréable).
//!
//! Écart assumé vs la spec : celle-ci décrit une chaîne NAudio explicite
//! (`MixingSampleProvider`, `WdlResamplingSampleProvider`…) parce que c'est ce
//! que NAudio expose. `rodio` (le choix retenu pour la transposition Rust,
//! voir `mod.rs`) mixe et rééchantillonne déjà en interne au sein d'un même
//! `OutputStream` : chaque `Sink` est juste une piste de plus dans son mixeur.
//! Le crossfade lui-même (§5.2, gains à puissance constante) reste identique
//! bit pour bit — c'est la seule partie du pipeline qui ne se déduit pas de
//! la bibliothèque, donc la seule qui mérite d'être écrite à la main ici.
//!
//! Écart §5.3 (préchargement) : `rodio` ne connaît la durée totale d'une
//! piste à l'avance que pour certains formats en décodage direct (WAV/FLAC
//! typiquement ; un MP3 décodé par `minimp3` renvoie `None`). `index.rs`
//! (§3.4) comble l'essentiel de cet écart : la durée exacte, calculée une
//! fois au premier scan du dossier en sous-produit du calcul RMS, est
//! préférée à celle du décodeur en direct — donc le vrai recouvrement
//! `crossfade_ms + 500ms` s'applique aussi aux MP3, dès qu'ils ont été
//! indexés. Le repli sur `Sink::empty()` (la piste précédente est alors déjà
//! silencieuse, le crossfade se comporte comme un simple fondu d'entrée) ne
//! reste utile que pour une piste dont l'indexation elle-même a échoué
//! (fichier corrompu, §9) — un cas marginal plutôt que la majorité des MP3.

use std::collections::HashMap;
use std::fs::File;
use std::io::BufReader;
use std::path::{Path, PathBuf};
use std::sync::mpsc::{self, Receiver, Sender};
use std::sync::Mutex;
use std::time::{Duration, Instant};

use rodio::{Decoder, OutputStream, OutputStreamHandle, Sink, Source};
use tauri::AppHandle;

use super::config::MusicConfig;
use super::index::{self, IndexedTrack};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Ambience {
    Menu,
    Grid,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum State {
    /// Big Picture inactif : rien ne joue.
    Idle,
    Playing(Ambience),
    /// AC est lancé : musique coupée ou en fond selon `sessionBehavior`.
    Session,
}

pub enum EngineCommand {
    UpdateConfig(MusicConfig),
    EnterBigPicture,
    ExitBigPicture,
    EnterMenu,
    EnterGrid,
    /// La voiture est réellement pilotable sur la piste (mémoire partagée
    /// AC, `AC_LIVE` — voir `ac_status.rs`) : c'est ce signal, pas le
    /// lancement du process, qui doit couper/baisser la musique. Le
    /// chargement de session continue de jouer l'ambiance GRID normalement.
    EnterSession,
    /// Fin de la conduite (retour aux stands/résultats, `AC_LIVE` quitté) ou
    /// filet de sécurité sur fermeture du process (voir `AcProcessStopped`).
    ExitSession,
    /// `acs.exe`/`AssettoCorsa.exe` vient de démarrer — aucun effet sur la
    /// lecture (le chargement n'est pas la course), juste un repère d'état
    /// pour `enter_big_picture` (§4, "reprise d'état").
    AcProcessStarted,
    /// Le process s'est fermé — filet de sécurité qui force la sortie de
    /// session même si la mémoire partagée n'a pas signalé la transition
    /// (ex. fermeture brutale d'AC pendant que la voiture était en piste).
    AcProcessStopped,
}

/// Poignée exposée comme état Tauri managé. `Sender` seul n'est garanti
/// `Sync` que depuis peu selon la version de std ; le `Mutex` le garantit
/// sans dépendre de ça.
pub struct MusicEngineHandle(Mutex<Sender<EngineCommand>>);

impl MusicEngineHandle {
    pub fn send(&self, cmd: EngineCommand) {
        if let Ok(tx) = self.0.lock() {
            // Le thread moteur ne se termine que si le canal est fermé, ce
            // qui n'arrive qu'à la fermeture de l'app : un échec d'envoi ici
            // ne peut normalement pas se produire en usage réel.
            let _ = tx.send(cmd);
        }
    }

    /// Poignée déportée dans le thread de surveillance AC (`watch.rs`), qui
    /// n'a besoin que d'émettre des commandes, pas de la synchronisation
    /// supplémentaire du `Mutex` à chaque envoi.
    pub(crate) fn clone_sender(&self) -> Sender<EngineCommand> {
        self.0.lock().expect("music engine sender mutex poisoned").clone()
    }
}

/// Démarre le thread moteur (propriétaire de l'`OutputStream` WASAPI, jamais
/// en mode exclusif — c'est le comportement par défaut de `rodio`/`cpal`,
/// voir §5.1 de la spec) et renvoie la poignée à manager côté Tauri.
pub fn spawn(app: AppHandle, initial_config: MusicConfig) -> MusicEngineHandle {
    let (tx, rx) = mpsc::channel();
    std::thread::spawn(move || run(app, initial_config, rx));
    MusicEngineHandle(Mutex::new(tx))
}

fn run(app: AppHandle, initial_config: MusicConfig, rx: Receiver<EngineCommand>) {
    let (_stream, handle) = match OutputStream::try_default() {
        Ok(v) => v,
        Err(e) => {
            log::warn!("music: aucun périphérique audio disponible, module musique inactif : {e}");
            return;
        }
    };
    let mut engine = Engine {
        app,
        handle,
        config: initial_config,
        slots: [Slot::empty(), Slot::empty()],
        active_slot: 0,
        playlists: HashMap::new(),
        state: State::Idle,
        fade: None,
        ac_process_running: false,
        big_picture_active: false,
        pre_session_ambience: None,
    };
    let tick = Duration::from_millis(30);
    loop {
        match rx.recv_timeout(tick) {
            Ok(cmd) => engine.handle_command(cmd),
            Err(mpsc::RecvTimeoutError::Timeout) => {}
            Err(mpsc::RecvTimeoutError::Disconnected) => break,
        }
        engine.advance_fade();
        engine.check_track_end();
    }
}

struct Slot {
    sink: Option<Sink>,
    ambience: Option<Ambience>,
    started_at: Option<Instant>,
    total_duration: Option<Duration>,
    /// Correction de gain de normalisation en dB, telle que calculée par
    /// `index.rs` (§3.4) pour la piste chargée dans ce slot — brute, pas
    /// encore convertie en facteur linéaire ni filtrée par `config.normalize`
    /// (fait à la lecture par `Engine::slot_gain_linear`, pour qu'activer/
    /// désactiver la normalisation dans les réglages s'entende tout de suite
    /// sur la piste en cours, pas seulement à la suivante).
    gain_db: f32,
}

impl Slot {
    fn empty() -> Self {
        Self {
            sink: None,
            ambience: None,
            started_at: None,
            total_duration: None,
            gain_db: 0.0,
        }
    }
}

/// Ordre de lecture d'une ambiance + position mémorisée pour reprendre là où
/// elle en était au retour (§5.4).
struct Playlist {
    tracks: Vec<IndexedTrack>,
    order: Vec<IndexedTrack>,
    pos: usize,
    elapsed_at_pause: Duration,
}

impl Playlist {
    fn load(tracks: Vec<IndexedTrack>, shuffle: bool) -> Self {
        let order = if shuffle {
            shuffle_order(&tracks, None)
        } else {
            tracks.clone()
        };
        Self {
            tracks,
            order,
            pos: 0,
            elapsed_at_pause: Duration::ZERO,
        }
    }

    fn current(&self) -> Option<IndexedTrack> {
        self.order.get(self.pos).cloned()
    }

    /// Passe à la piste suivante, en rejouant une nouvelle permutation quand
    /// la liste est épuisée (jamais un tirage à chaque piste, §5.4).
    fn advance(&mut self, shuffle: bool) -> Option<IndexedTrack> {
        if self.order.is_empty() {
            return None;
        }
        let previous = self.current();
        self.pos += 1;
        if self.pos >= self.order.len() {
            self.order = if shuffle {
                shuffle_order(&self.tracks, previous.as_ref())
            } else {
                self.tracks.clone()
            };
            self.pos = 0;
        }
        self.current()
    }
}

fn apply_no_repeat_constraint(order: &mut [IndexedTrack], last_played: Option<&IndexedTrack>) {
    if order.len() < 2 {
        return;
    }
    if let Some(last) = last_played {
        if order[0].path == last.path {
            order.swap(0, 1);
        }
    }
}

/// Permutation Fisher-Yates (§5.4) + contrainte : si la première piste de la
/// nouvelle permutation est la dernière jouée, l'échanger avec la seconde.
pub fn shuffle_order(tracks: &[IndexedTrack], last_played: Option<&IndexedTrack>) -> Vec<IndexedTrack> {
    let mut order = tracks.to_vec();
    fastrand::shuffle(&mut order);
    apply_no_repeat_constraint(&mut order, last_played);
    order
}

/// Crossfade à puissance constante (§5.2). `t` ∈ [0,1]. Invariant :
/// `gain_out² + gain_in² == 1`, contrairement à un fondu linéaire qui produit
/// un creux de volume audible au milieu de la transition.
pub fn crossfade_gains(t: f32) -> (f32, f32) {
    let t = t.clamp(0.0, 1.0) * std::f32::consts::FRAC_PI_2;
    (t.cos(), t.sin())
}

/// Fondu simple, courbe quadratique (§5.2) — l'oreille perçoit le volume de
/// façon logarithmique, une rampe linéaire "part" trop vite.
pub fn fade_in_gain(t: f32) -> f32 {
    let t = t.clamp(0.0, 1.0);
    t * t
}

pub fn fade_out_gain(t: f32) -> f32 {
    let t = t.clamp(0.0, 1.0);
    (1.0 - t) * (1.0 - t)
}

/// Volume d'un `SingleFade` à l'instant `t` ∈ [0,1] entre `from` et `to`.
/// `fade_in_gain` est une progression 0→1 (directement utilisable dans le
/// lerp `from + (to-from)*progress`) ; `fade_out_gain` est déjà une courbe de
/// gain 1→0, pas une progression — la combiner au même lerp inverserait le
/// sens du fondu (silence au début, plein volume juste avant la coupure nette
/// de `stop_at_end`). Elle se combine donc avec `to + (from-to)*gain` à la
/// place.
fn single_fade_volume(from: f32, to: f32, t: f32) -> f32 {
    if to >= from {
        from + (to - from) * fade_in_gain(t)
    } else {
        to + (from - to) * fade_out_gain(t)
    }
}

fn elapsed_fraction(start: Instant, duration: Duration) -> f32 {
    if duration.is_zero() {
        return 1.0;
    }
    (start.elapsed().as_secs_f32() / duration.as_secs_f32()).min(1.0)
}

#[derive(Clone, Copy)]
enum ActiveFade {
    Crossfade {
        out_slot: usize,
        in_slot: usize,
        start: Instant,
        duration: Duration,
    },
    SingleFade {
        slot: usize,
        start: Instant,
        duration: Duration,
        from: f32,
        to: f32,
        stop_at_end: bool,
    },
}

fn open_track(path: &Path) -> Option<Decoder<BufReader<File>>> {
    let file = File::open(path).ok()?;
    Decoder::new(BufReader::new(file)).ok()
}

struct Engine {
    app: AppHandle,
    handle: OutputStreamHandle,
    config: MusicConfig,
    slots: [Slot; 2],
    active_slot: usize,
    playlists: HashMap<Ambience, Playlist>,
    state: State,
    fade: Option<ActiveFade>,
    /// `acs.exe`/`AssettoCorsa.exe` tourne, chargement compris — distinct de
    /// `state == Session` qui ne devient vrai qu'à `AC_LIVE` (voiture
    /// réellement en piste). Sert uniquement de garde à `enter_big_picture`.
    ac_process_running: bool,
    /// Le mode Big Picture est ouvert à l'écran — mis à jour en même temps
    /// que les commandes `EnterBigPicture`/`ExitBigPicture`, y compris
    /// quand `enter_big_picture` reste silencieux (AC déjà lancé). Distinct
    /// de `state != Idle` : sert à `exit_session` à savoir s'il doit
    /// vraiment reprendre une ambiance ou rester silencieux (§19).
    big_picture_active: bool,
    /// Ambiance active juste avant `enter_session` (menu ou grid selon
    /// l'écran affiché au lancement) — `exit_session` y revient plutôt que
    /// de retomber systématiquement sur MENU (§4/§19).
    pre_session_ambience: Option<Ambience>,
}

impl Engine {
    fn handle_command(&mut self, cmd: EngineCommand) {
        match cmd {
            EngineCommand::UpdateConfig(cfg) => {
                // Playlist en cache indexée par ambiance (`ensure_playlist`) :
                // si le dossier *effectif* a changé, l'ancienne — vide ou pas
                // — doit être oubliée, sinon un dossier vide au premier
                // lancement de Big Picture reste mémorisé comme "vide" pour
                // toujours, même après avoir pointé vers un dossier valide et
                // enregistré. Bug réel : ça obligeait à redémarrer l'app pour
                // que le nouveau dossier soit pris en compte. Comparaison sur
                // le dossier *effectif* (pas juste `menu_folder`/`grid_folder`
                // bruts) : basculer `use_custom_folders` change l'ambiance
                // réellement jouée sans forcément changer ces deux champs.
                let old_menu = self.folder_for(Ambience::Menu);
                let old_grid = self.folder_for(Ambience::Grid);
                let was_enabled = self.config.enabled;
                self.config = cfg;
                if self.folder_for(Ambience::Menu) != old_menu {
                    self.playlists.remove(&Ambience::Menu);
                }
                if self.folder_for(Ambience::Grid) != old_grid {
                    self.playlists.remove(&Ambience::Grid);
                }
                // Un changement de volume dans les réglages doit s'entendre
                // tout de suite, pas seulement à la prochaine transition.
                if self.fade.is_none() {
                    self.set_slot_volume(self.active_slot, self.effective_volume());
                }
                // La case « activer la musique » aussi : cochée ou décochée
                // pendant qu'on est DÉJÀ en Big Picture, elle ne faisait
                // jusqu'ici que régler le sort de la prochaine entrée — donc
                // rien du tout, à l'oreille, pour qui la coche depuis le mode
                // lui-même (bug signalé). Les deux fonctions appelées portent
                // déjà toutes leurs gardes (AC lancé, session en cours, rien
                // qui joue) : rien à dupliquer ici.
                if was_enabled != self.config.enabled {
                    if self.config.enabled {
                        // `big_picture_active` et pas `state != Idle` : c'est
                        // justement parce que rien ne joue qu'on rallume.
                        if self.big_picture_active {
                            self.enter_big_picture();
                        }
                    } else {
                        // Sans toucher à `big_picture_active` : le mode reste
                        // ouvert, c'est la musique qu'on vient de couper — et
                        // c'est ce qui permet de la rallumer sans sortir.
                        self.exit_big_picture();
                    }
                }
            }
            EngineCommand::EnterBigPicture => {
                self.big_picture_active = true;
                self.enter_big_picture();
            }
            EngineCommand::ExitBigPicture => {
                self.exit_big_picture();
                self.big_picture_active = false;
            }
            EngineCommand::EnterMenu => self.switch_ambience(Ambience::Menu),
            EngineCommand::EnterGrid => self.switch_ambience(Ambience::Grid),
            EngineCommand::EnterSession => self.enter_session(),
            EngineCommand::ExitSession => self.exit_session(),
            EngineCommand::AcProcessStarted => self.ac_process_running = true,
            EngineCommand::AcProcessStopped => {
                self.ac_process_running = false;
                if self.state == State::Session {
                    // Filet de sécurité : la mémoire partagée n'a pas
                    // signalé la sortie de AC_LIVE (fermeture brutale) —
                    // sans ça la musique resterait coupée indéfiniment.
                    self.exit_session();
                }
            }
        }
    }

    fn effective_volume(&self) -> f32 {
        self.config.volume.clamp(0.0, 1.0)
    }

    fn folder_for(&self, amb: Ambience) -> PathBuf {
        match amb {
            Ambience::Menu => self.config.effective_menu_folder(&self.app),
            Ambience::Grid => self.config.effective_grid_folder(&self.app),
        }
    }

    fn new_playlist(&self, amb: Ambience) -> Playlist {
        Playlist::load(index::indexed_tracks(&self.folder_for(amb)), self.config.shuffle)
    }

    fn ensure_playlist(&mut self, amb: Ambience) {
        if !self.playlists.contains_key(&amb) {
            let pl = self.new_playlist(amb);
            self.playlists.insert(amb, pl);
        }
    }

    /// Facteur linéaire de la correction de normalisation (§3.4) — toujours
    /// appliqué, pas de réglage pour le désactiver (décidé avec
    /// l'utilisateur : aucune raison de vouloir des sauts de volume entre
    /// pistes hétérogènes).
    fn slot_gain_linear(&self, slot: usize) -> f32 {
        10f32.powf(self.slots[slot].gain_db / 20.0)
    }

    fn set_slot_volume(&mut self, slot: usize, vol: f32) {
        let gain = self.slot_gain_linear(slot);
        if let Some(sink) = &self.slots[slot].sink {
            sink.set_volume((vol * gain).max(0.0));
        }
    }

    fn stop_slot(&mut self, slot: usize) {
        // Le Drop du Sink coupe la lecture et relâche le verrou sur le
        // fichier (§9 — sans ça, l'utilisateur ne peut plus renommer ni
        // supprimer ses pistes tant que l'app tourne).
        self.slots[slot] = Slot::empty();
    }

    /// Volume "ambiant" courant d'un slot (avant gain de normalisation),
    /// dans le même espace que les champs `from`/`to` de `ActiveFade` — lu
    /// depuis le `Sink` plutôt que supposé, pour qu'un nouveau fondu démarré
    /// en interrompant un autre reparte de là où le son en est vraiment,
    /// pas d'un volume plein qu'il n'avait peut-être pas encore atteint.
    fn current_ambient_volume(&self, slot: usize) -> f32 {
        let gain = self.slot_gain_linear(slot);
        let raw = self.slots[slot].sink.as_ref().map(|s| s.volume()).unwrap_or(0.0);
        if gain > 0.0 {
            raw / gain
        } else {
            0.0
        }
    }

    /// Résout proprement un fondu en cours avant d'en démarrer un autre.
    /// Sans ça, une transition interrompue par une autre (ex. sortie de Big
    /// Picture pendant un crossfade menu/grid) laisse le slot abandonné
    /// jouer indéfiniment à son dernier volume — plus aucun code ne le
    /// touche jamais, ni pour le stopper ni pour le refader. Bug réel :
    /// s'entendait comme un décrochage suivi d'un retour de son à la sortie
    /// du mode Big Picture.
    fn cancel_fade(&mut self) {
        match self.fade.take() {
            Some(ActiveFade::Crossfade { out_slot, .. }) => self.stop_slot(out_slot),
            Some(ActiveFade::SingleFade { slot, stop_at_end, .. }) if stop_at_end => self.stop_slot(slot),
            _ => {}
        }
    }

    fn save_elapsed(&mut self, amb: Ambience) {
        let started = self.slots[self.active_slot].started_at;
        if let (Some(started), Some(pl)) = (started, self.playlists.get_mut(&amb)) {
            pl.elapsed_at_pause = started.elapsed();
        }
    }

    fn start_track_in_slot(&mut self, slot: usize, amb: Ambience, track: &IndexedTrack, resume_at: Duration) -> bool {
        let Ok(sink) = Sink::try_new(&self.handle) else {
            log::warn!("music: impossible de créer un lecteur audio");
            return false;
        };
        let Some(decoder) = open_track(&track.path) else {
            // Piste corrompue/illisible (§9) : on journalise et on laisse
            // l'appelant décider de la suite plutôt que de faire planter la
            // lecture pour tout le dossier.
            log::warn!("music: piste illisible, ignorée : {}", track.path.display());
            return false;
        };
        // Durée indexée (§3.4/index.rs) préférée à `total_duration()` : fiable
        // même pour les MP3, où le décodeur en direct répond souvent `None`
        // (§5.3 — comble l'écart documenté en tête de ce fichier).
        let total_duration = track.duration.or_else(|| decoder.total_duration());
        sink.set_volume(0.0);
        sink.append(decoder);
        if resume_at > Duration::ZERO && sink.try_seek(resume_at).is_err() {
            log::warn!(
                "music: reprise de position impossible sur {}, redémarre du début",
                track.path.display()
            );
        }
        self.slots[slot] = Slot {
            sink: Some(sink),
            ambience: Some(amb),
            started_at: Some(Instant::now()),
            total_duration,
            gain_db: track.gain_db,
        };
        true
    }

    /// Crossfade vers `target` : reprend la piste mémorisée (`advance =
    /// false`, changement d'ambiance MENU↔GRID) ou passe à la suivante
    /// (`advance = true`, fin de piste).
    fn begin_crossfade(&mut self, target: Ambience, advance: bool) {
        if let Some(prev_amb) = self.slots[self.active_slot].ambience {
            self.save_elapsed(prev_amb);
        }
        // Avant de choisir `next_slot` : un fondu interrompu doit être résolu
        // (slot abandonné stoppé) avant qu'on décide où charger la piste
        // suivante, sinon `cancel_fade` risquerait d'effacer le slot qu'on
        // vient tout juste d'y écrire.
        self.cancel_fade();
        self.ensure_playlist(target);
        let track = if advance {
            self.playlists.get_mut(&target).unwrap().advance(self.config.shuffle)
        } else {
            self.playlists.get(&target).unwrap().current()
        };
        let Some(track) = track else {
            log::warn!("music: dossier vide ou introuvable pour l'ambiance {target:?}, transition ignorée");
            return;
        };
        let resume_at = if advance {
            Duration::ZERO
        } else {
            self.playlists.get(&target).unwrap().elapsed_at_pause
        };
        let next_slot = 1 - self.active_slot;
        if !self.start_track_in_slot(next_slot, target, &track, resume_at) {
            return;
        }
        self.fade = Some(ActiveFade::Crossfade {
            out_slot: self.active_slot,
            in_slot: next_slot,
            start: Instant::now(),
            duration: Duration::from_millis(self.config.crossfade_ms as u64),
        });
        self.active_slot = next_slot;
    }

    fn switch_ambience(&mut self, target: Ambience) {
        if !matches!(self.state, State::Playing(_)) {
            // Session en cours ou Big Picture inactif : une bascule menu/grid
            // n'a de sens que pendant la navigation Big Picture (§4).
            return;
        }
        if self.state == State::Playing(target) {
            return;
        }
        self.begin_crossfade(target, false);
        self.state = State::Playing(target);
    }

    fn check_track_end(&mut self) {
        if self.fade.is_some() {
            return;
        }
        let State::Playing(amb) = self.state else { return };
        let slot = &self.slots[self.active_slot];
        let Some(sink) = &slot.sink else { return };
        let should_advance = match (slot.started_at, slot.total_duration) {
            (Some(started), Some(total)) => {
                let margin = Duration::from_millis(self.config.crossfade_ms as u64 + 500);
                total
                    .checked_sub(margin)
                    .is_some_and(|threshold| started.elapsed() >= threshold)
            }
            _ => sink.empty(),
        };
        if should_advance {
            self.begin_crossfade(amb, true);
        }
    }

    fn advance_fade(&mut self) {
        let Some(fade) = self.fade else { return };
        match fade {
            ActiveFade::Crossfade {
                out_slot,
                in_slot,
                start,
                duration,
            } => {
                let t = elapsed_fraction(start, duration);
                let (gain_out, gain_in) = crossfade_gains(t);
                let vol = self.effective_volume();
                self.set_slot_volume(out_slot, gain_out * vol);
                self.set_slot_volume(in_slot, gain_in * vol);
                if t >= 1.0 {
                    self.stop_slot(out_slot);
                    self.fade = None;
                }
            }
            ActiveFade::SingleFade {
                slot,
                start,
                duration,
                from,
                to,
                stop_at_end,
            } => {
                let t = elapsed_fraction(start, duration);
                let vol = single_fade_volume(from, to, t);
                self.set_slot_volume(slot, vol);
                if t >= 1.0 {
                    if stop_at_end {
                        self.stop_slot(slot);
                    }
                    self.fade = None;
                }
            }
        }
    }

    fn enter_big_picture(&mut self) {
        if self.ac_process_running {
            // Reprise d'état (§4) : AC tourne déjà (chargement ou course),
            // inutile de démarrer une ambiance vouée à être coupée ou à
            // couvrir le chargement.
            return;
        }
        if self.state != State::Idle || !self.config.enabled {
            return;
        }
        self.cancel_fade();
        self.ensure_playlist(Ambience::Menu);
        let Some(track) = self.playlists.get(&Ambience::Menu).unwrap().current() else {
            log::warn!("music: dossier menu vide ou introuvable, Big Picture démarre en silence");
            return;
        };
        if self.start_track_in_slot(self.active_slot, Ambience::Menu, &track, Duration::ZERO) {
            self.fade = Some(ActiveFade::SingleFade {
                slot: self.active_slot,
                start: Instant::now(),
                duration: Duration::from_millis(self.config.fade_in_ms as u64),
                from: 0.0,
                to: self.effective_volume(),
                stop_at_end: false,
            });
            self.state = State::Playing(Ambience::Menu);
        }
    }

    fn exit_big_picture(&mut self) {
        if self.state == State::Idle {
            return;
        }
        if self.state == State::Session {
            // Rien ne jouait (cf. enter_big_picture) : retour direct à Idle.
            self.state = State::Idle;
            return;
        }
        if let State::Playing(amb) = self.state {
            self.save_elapsed(amb);
        }
        let slot = self.active_slot;
        // Part du volume réellement atteint, pas de `effective_volume()` —
        // sortir de Big Picture pendant qu'un crossfade ou un fondu d'entrée
        // est encore en cours ne doit pas faire sauter le son au plein
        // volume avant de le refaire redescendre (§18, bug réel : "ça coupe,
        // ça revient, puis ça repart"). `cancel_fade` stoppe aussi l'éventuel
        // slot abandonné d'un crossfade interrompu (l'autre moitié du bug :
        // sans ça, il continuait de jouer indéfiniment, plus jamais touché).
        let from = self.current_ambient_volume(slot);
        self.cancel_fade();
        if self.slots[slot].sink.is_some() {
            self.fade = Some(ActiveFade::SingleFade {
                slot,
                start: Instant::now(),
                duration: Duration::from_millis(self.config.fade_out_ms as u64),
                from,
                to: 0.0,
                stop_at_end: true,
            });
        }
        self.state = State::Idle;
    }

    fn enter_session(&mut self) {
        if self.state == State::Session {
            return;
        }
        if self.state == State::Idle {
            // Big Picture pas actif : rien à faire fader, juste mémoriser
            // qu'AC tourne pour qu'exit_session sache qu'il ne doit rien
            // relancer non plus.
            self.state = State::Session;
            return;
        }
        if let State::Playing(amb) = self.state {
            self.save_elapsed(amb);
            // Mémorisé pour `exit_session` : reprendre la même ambiance
            // qu'avant la session (menu ou grid selon l'écran affiché au
            // lancement, §4/§19) plutôt que de retomber systématiquement sur
            // MENU.
            self.pre_session_ambience = Some(amb);
        }
        // Musique toujours coupée pendant la session — jamais de "duck" en
        // fond (décidé avec l'utilisateur : en course comme en essais, la
        // musique de préparation n'a plus sa place une fois la voiture en
        // piste).
        let slot = self.active_slot;
        let from = self.current_ambient_volume(slot);
        self.cancel_fade();
        self.fade = Some(ActiveFade::SingleFade {
            slot,
            start: Instant::now(),
            duration: Duration::from_millis(self.config.fade_out_ms as u64),
            from,
            to: 0.0,
            stop_at_end: true,
        });
        self.state = State::Session;
    }

    fn exit_session(&mut self) {
        if self.state != State::Session {
            return;
        }
        if !self.big_picture_active {
            // Big Picture n'est pas ouvert (la session a démarré et s'est
            // terminée sans que l'utilisateur y entre) : rien à reprendre,
            // et surtout pas démarrer une ambiance dans le vide juste parce
            // qu'AC vient de rendre la main.
            self.state = State::Idle;
            return;
        }
        // Reprend l'ambiance active avant la session — menu ou grid selon
        // l'écran affiché au lancement (§4/§19) — jamais systématiquement
        // MENU. `None` seulement si la session a commencé avant que Big
        // Picture ait joué quoi que ce soit (ouvert pendant le chargement) :
        // MENU reste alors le repli le plus sensé.
        let target = self.pre_session_ambience.unwrap_or(Ambience::Menu);
        let slot = self.active_slot;
        self.cancel_fade();
        // Musique toujours coupée pendant la session (`enter_session`,
        // `stop_at_end: true`) : le slot est donc systématiquement vide ici —
        // reprend la piste mémorisée de l'ambiance cible à sa position de
        // pause, fade-in depuis le silence.
        self.ensure_playlist(target);
        let pl = self.playlists.get(&target).unwrap();
        let track = pl.current();
        let resume_at = pl.elapsed_at_pause;
        match track {
            Some(track) if self.start_track_in_slot(slot, target, &track, resume_at) => {
                self.fade = Some(ActiveFade::SingleFade {
                    slot,
                    start: Instant::now(),
                    duration: Duration::from_millis(self.config.fade_in_ms as u64),
                    from: 0.0,
                    to: self.effective_volume(),
                    stop_at_end: false,
                });
            }
            _ => {
                log::warn!(
                    "music: dossier vide ou introuvable pour l'ambiance {target:?}, reprise après session silencieuse"
                );
            }
        }
        self.state = State::Playing(target);
    }
}

// --- Écoute au clic (§6, bouton ▶) --------------------------------------
//
// Entièrement indépendante du moteur ci-dessus : une prévisualisation ouvre
// son propre `OutputStream` éphémère (plusieurs flux WASAPI en mode partagé
// coexistent sans se couper la parole) plutôt que de perturber l'état
// menu/grid en cours.

pub struct PreviewHandle(Mutex<Option<Sender<()>>>);

impl Default for PreviewHandle {
    fn default() -> Self {
        Self(Mutex::new(None))
    }
}

impl PreviewHandle {
    pub fn start(&self, track: PathBuf, volume: f32) {
        self.stop();
        let (tx, rx) = mpsc::channel::<()>();
        std::thread::spawn(move || {
            let Ok((_stream, handle)) = OutputStream::try_default() else {
                return;
            };
            let Ok(sink) = Sink::try_new(&handle) else { return };
            let Some(decoder) = open_track(&track) else { return };
            sink.set_volume(volume.clamp(0.0, 1.0));
            sink.append(decoder);
            loop {
                if rx.recv_timeout(Duration::from_millis(150)).is_ok() {
                    break; // stop() appelé
                }
                if sink.empty() {
                    break; // piste terminée
                }
            }
        });
        if let Ok(mut guard) = self.0.lock() {
            *guard = Some(tx);
        }
    }

    pub fn stop(&self) {
        if let Ok(mut guard) = self.0.lock() {
            if let Some(tx) = guard.take() {
                let _ = tx.send(());
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn crossfade_gains_preserve_constant_power() {
        for i in 0..=10 {
            let t = i as f32 / 10.0;
            let (out, inn) = crossfade_gains(t);
            let power = out * out + inn * inn;
            assert!((power - 1.0).abs() < 1e-5, "t={t}: puissance={power}, attendu 1.0");
        }
    }

    #[test]
    fn crossfade_gains_are_monotonic_bounds() {
        let (out0, in0) = crossfade_gains(0.0);
        assert!(
            (out0 - 1.0).abs() < 1e-6 && in0.abs() < 1e-6,
            "t=0 : tout sur la piste sortante"
        );
        let (out1, in1) = crossfade_gains(1.0);
        assert!(
            out1.abs() < 1e-6 && (in1 - 1.0).abs() < 1e-6,
            "t=1 : tout sur la piste entrante"
        );
    }

    #[test]
    fn fade_curves_reach_their_bounds() {
        assert_eq!(fade_in_gain(0.0), 0.0);
        assert!((fade_in_gain(1.0) - 1.0).abs() < f32::EPSILON);
        assert!((fade_out_gain(0.0) - 1.0).abs() < f32::EPSILON);
        assert_eq!(fade_out_gain(1.0), 0.0);
    }

    #[test]
    fn single_fade_volume_ramps_down_for_a_fade_out() {
        // Bug réel (§5.2) : en combinant `fade_out_gain` (déjà une courbe de
        // gain 1→0) au même lerp que `fade_in_gain` (une progression 0→1), le
        // volume d'un fondu de sortie partait de 0 pour remonter jusqu'à
        // `from` — silence au début, plein volume juste avant la coupure
        // nette de `stop_at_end` : la saccade entendue en sortie de Big
        // Picture.
        assert!(
            (single_fade_volume(0.8, 0.0, 0.0) - 0.8).abs() < 1e-6,
            "t=0 : encore au volume de départ"
        );
        assert!(single_fade_volume(0.8, 0.0, 1.0).abs() < 1e-6, "t=1 : silence atteint");
        let mid = single_fade_volume(0.8, 0.0, 0.5);
        assert!(
            mid > 0.0 && mid < 0.8,
            "à mi-chemin, ni silence ni plein volume : {mid}"
        );
    }

    #[test]
    fn single_fade_volume_ramps_up_for_a_fade_in() {
        assert!(
            single_fade_volume(0.0, 0.8, 0.0).abs() < 1e-6,
            "t=0 : encore silencieux"
        );
        assert!(
            (single_fade_volume(0.0, 0.8, 1.0) - 0.8).abs() < 1e-6,
            "t=1 : volume cible atteint"
        );
        let mid = single_fade_volume(0.0, 0.8, 0.5);
        assert!(
            mid > 0.0 && mid < 0.8,
            "à mi-chemin, ni silence ni volume cible : {mid}"
        );
    }

    fn track(name: &str) -> IndexedTrack {
        IndexedTrack {
            path: PathBuf::from(name),
            gain_db: 0.0,
            duration: None,
        }
    }

    #[test]
    fn no_repeat_constraint_swaps_first_two_when_first_equals_last_played() {
        let mut order = vec![track("a"), track("b"), track("c")];
        let last = track("a");
        apply_no_repeat_constraint(&mut order, Some(&last));
        assert_eq!(order[0], track("b"));
        assert_eq!(order[1], track("a"));
    }

    #[test]
    fn no_repeat_constraint_leaves_order_untouched_otherwise() {
        let mut order = vec![track("a"), track("b")];
        apply_no_repeat_constraint(&mut order, Some(&track("z")));
        assert_eq!(order, vec![track("a"), track("b")]);
    }

    #[test]
    fn playlist_advance_cycles_without_shuffle() {
        let tracks = vec![track("a"), track("b")];
        let mut pl = Playlist::load(tracks, false);
        assert_eq!(pl.current(), Some(track("a")));
        assert_eq!(pl.advance(false), Some(track("b")));
        assert_eq!(pl.advance(false), Some(track("a")), "liste épuisée -> repart du début");
    }

    #[test]
    fn playlist_of_empty_folder_never_advances() {
        let mut pl = Playlist::load(Vec::new(), true);
        assert_eq!(pl.current(), None);
        assert_eq!(pl.advance(true), None);
    }
}
