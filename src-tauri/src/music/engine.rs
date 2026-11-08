//! Moteur audio (MUSIQUE§4, MUSIQUE§5, §16.6) : deux lecteurs qui se relaient
//! pour le crossfade, et autant d'ambiances que de **sources** — un dossier ou
//! un fichier, identifiés par leur chemin. MENU et GRID sont deux sources
//! parmi d'autres ; une marque, une famille, un écran peuvent en désigner une
//! troisième (§16.4). Chacune garde sa position de lecture (MUSIQUE§5.4,
//! généralisé) : revenir sur une ambiance la reprend où on l'avait laissée.
//!
//! **Ce qui joue est décidé par le front, pas ici** : l'écran affiché envoie un
//! `Context` (la source attribuée qu'il a résolue, et l'ambiance de base qui
//! la remplace), et le moteur se contente de l'appliquer — après
//! `RESOLVE_DELAY`, la dernière demande seule comptant, et sans aucune
//! transition quand la source est déjà celle qui joue.
//!
//! Écart assumé vs la spec : celle-ci décrit une chaîne NAudio explicite
//! (`MixingSampleProvider`, `WdlResamplingSampleProvider`…) parce que c'est ce
//! que NAudio expose. `rodio` mixe et rééchantillonne déjà en interne au sein
//! d'un même `OutputStream` : chaque `Sink` est juste une piste de plus dans
//! son mixeur. Le crossfade lui-même (MUSIQUE§5.2, gains à puissance constante)
//! reste identique — c'est la seule partie du pipeline qui ne se déduit pas de
//! la bibliothèque, donc la seule qui mérite d'être écrite à la main ici.
//!
//! Écart MUSIQUE§5.3 (préchargement) : `rodio` ne connaît la durée totale d'une
//! piste à l'avance que pour certains formats. `index.rs` comble l'essentiel de
//! cet écart avec la durée exacte calculée au premier scan ; le repli sur
//! `Sink::empty()` ne reste utile que pour une piste dont l'indexation a échoué.

use std::fs::File;
use std::io::BufReader;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::mpsc::{self, Receiver, Sender};
use std::sync::Mutex;
use std::time::{Duration, Instant};

use rodio::{Decoder, OutputStream, OutputStreamHandle, Sink, Source};
use serde::Deserialize;
use tauri::AppHandle;

use super::config::{Base, MusicConfig, PlayMode};
use super::index::{self, IndexedTrack};

/// How long a new resolution waits before it is applied (§16.6). Scrolling
/// through car sheets must not fade at every step: only the context still
/// asked for after this delay plays.
pub const RESOLVE_DELAY: Duration = Duration::from_secs(2);

/// How many sources keep their playlist and position in memory. Beyond, the
/// least recently heard one is forgotten (and starts over next time). A
/// playlist holds no file open — only the two playing `Sink`s do — so this
/// bounds memory, not file locks.
const PLAYLISTS_KEPT: usize = 8;

/// What the screen on display asks to hear (§16.5), as the front resolved it.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
pub struct Context {
    /// The source assigned to the first level of the cascade that has one.
    pub assigned: Option<PathBuf>,
    /// What plays when nothing is assigned, or the user's own music is off.
    pub base: Base,
}

impl Default for Context {
    fn default() -> Self {
        Self {
            assigned: None,
            base: Base::Menu,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
enum State {
    /// Nothing plays: music off, Big Picture closed in that mode, window
    /// minimised, a preview playing, or a source with nothing in it.
    Idle,
    Playing(PathBuf),
    /// La voiture est en piste : musique coupée.
    Session,
}

pub enum EngineCommand {
    UpdateConfig(MusicConfig),
    EnterBigPicture,
    ExitBigPicture,
    /// The window was minimised (`true`) or restored: music fades out, then
    /// back in. A mere loss of focus is not sent — music goes on.
    SetMinimized(bool),
    SetContext(Context),
    /// A preview (▶ of a picker) started or ended, with its generation: the
    /// ambience steps aside while it plays, then resumes (`PreviewHandle`).
    PreviewStarted(u64),
    PreviewEnded(u64),
    /// La voiture est réellement pilotable sur la piste (mémoire partagée
    /// AC, `AC_LIVE` — voir `ac_status.rs`) : c'est ce signal, pas le
    /// lancement du process, qui coupe la musique. Le chargement de session
    /// continue de jouer l'ambiance normalement.
    EnterSession,
    /// Fin de la conduite (retour aux stands/résultats, `AC_LIVE` quitté) ou
    /// filet de sécurité sur fermeture du process (voir `AcProcessStopped`).
    ExitSession,
    /// `acs.exe`/`AssettoCorsa.exe` vient de démarrer — aucun effet sur la
    /// lecture, juste un repère d'état (MUSIQUE§4, "reprise d'état").
    AcProcessStarted,
    /// Le process s'est fermé — filet de sécurité qui force la sortie de
    /// session même si la mémoire partagée n'a pas signalé la transition.
    AcProcessStopped,
}

/// Poignée exposée comme état Tauri managé. Le `Mutex` garantit `Sync` sans
/// dépendre de la version de std.
pub struct MusicEngineHandle(Mutex<Sender<EngineCommand>>);

impl MusicEngineHandle {
    pub fn send(&self, cmd: EngineCommand) {
        if let Ok(tx) = self.0.lock() {
            // Le thread moteur ne se termine que si le canal est fermé, ce
            // qui n'arrive qu'à la fermeture de l'app.
            let _ = tx.send(cmd);
        }
    }

    /// Poignée déportée dans les threads qui n'ont besoin que d'émettre
    /// (surveillance AC `watch.rs`, écoute `PreviewHandle`).
    pub(crate) fn clone_sender(&self) -> Sender<EngineCommand> {
        self.0.lock().expect("music engine sender mutex poisoned").clone()
    }
}

/// Démarre le thread moteur (propriétaire de l'`OutputStream` WASAPI, jamais
/// en mode exclusif — le comportement par défaut de `rodio`/`cpal`, MUSIQUE§5.1)
/// et renvoie la poignée à manager côté Tauri.
///
/// `on_track` is called with the path of every track that starts playing, for
/// the now-playing notification (MUSIQUE§5.5). A closure rather than an event
/// emitted from here: a business module importing `tauri::Emitter` makes the
/// lib's test binary unrunnable (see `CLAUDE.md`), so the facade emits. It runs
/// on the engine thread, whose 30 ms tick drives the fades: it must hand the
/// work off, not do it.
pub fn spawn(
    app: AppHandle,
    initial_config: MusicConfig,
    on_track: impl Fn(PathBuf) + Send + 'static,
) -> MusicEngineHandle {
    let (tx, rx) = mpsc::channel();
    let paths = Paths {
        root: super::config::music_root(&app),
        cache: super::config::index_dir(&app),
    };
    std::thread::spawn(move || run(paths, initial_config, Box::new(on_track), rx));
    MusicEngineHandle(Mutex::new(tx))
}

/// The app folders the engine reads: the music root (embedded pack, default
/// folders) and the index caches.
struct Paths {
    root: PathBuf,
    cache: PathBuf,
}

fn run(paths: Paths, initial_config: MusicConfig, on_track: Box<dyn Fn(PathBuf) + Send>, rx: Receiver<EngineCommand>) {
    let (_stream, handle) = match OutputStream::try_default() {
        Ok(v) => v,
        Err(e) => {
            log::warn!("music: aucun périphérique audio disponible, module musique inactif : {e}");
            return;
        }
    };
    let mut engine = Engine {
        paths,
        on_track,
        handle,
        config: initial_config,
        slots: [Slot::empty(), Slot::empty()],
        active_slot: 0,
        playlists: Lru::new(PLAYLISTS_KEPT),
        state: State::Idle,
        fade: None,
        context: Context::default(),
        pending: Debounced::new(RESOLVE_DELAY),
        flags: Flags::default(),
    };
    let tick = Duration::from_millis(30);
    loop {
        match rx.recv_timeout(tick) {
            Ok(cmd) => engine.handle_command(cmd),
            Err(mpsc::RecvTimeoutError::Timeout) => {}
            Err(mpsc::RecvTimeoutError::Disconnected) => break,
        }
        engine.apply_due_context();
        engine.advance_fade();
        engine.check_track_end();
    }
}

/// Everything outside the music itself that decides whether it should play.
#[derive(Debug, Default, Clone, Copy)]
struct Flags {
    big_picture: bool,
    minimized: bool,
    /// Generation of the preview playing, if any.
    preview: Option<u64>,
    /// `acs.exe` tourne, chargement compris — distinct de `State::Session`
    /// qui ne devient vrai qu'à `AC_LIVE`. Pendant le chargement la musique
    /// continue ; mais rien ne *démarre* tant que le jeu tourne (MUSIQUE§4,
    /// "reprise d'état") : une ambiance vouée à être coupée n'a pas à partir.
    ac_running: bool,
}

/// Whether music should be heard at all (§16.1), the session aside.
fn music_wanted(mode: PlayMode, flags: Flags) -> bool {
    let mode_on = match mode {
        PlayMode::Off => false,
        PlayMode::BigPicture => flags.big_picture,
        PlayMode::Always => true,
    };
    mode_on && !flags.minimized && flags.preview.is_none()
}

/// Whether music may start now. Nothing starts while the game runs (an
/// ambience bound to be cut, MUSIQUE§4) — except on the way back from the
/// track (fter_session): the game still runs in the pits, and that is
/// exactly when the music returns (§16.1).
fn may_start(wanted: bool, ac_running: bool, after_session: bool) -> bool {
    wanted && (after_session || !ac_running)
}

/// A value applied only once it has stood for `delay`, the last one pushed
/// replacing any earlier one (§16.6).
struct Debounced<T> {
    pending: Option<(T, Instant)>,
    delay: Duration,
}

impl<T> Debounced<T> {
    fn new(delay: Duration) -> Self {
        Self { pending: None, delay }
    }

    fn push(&mut self, value: T, now: Instant) {
        self.pending = Some((value, now + self.delay));
    }

    fn clear(&mut self) {
        self.pending = None;
    }

    fn take_due(&mut self, now: Instant) -> Option<T> {
        match &self.pending {
            Some((_, due)) if now >= *due => self.pending.take().map(|(v, _)| v),
            _ => None,
        }
    }
}

/// Playlists by source, most recently used first, bounded (`PLAYLISTS_KEPT`).
struct Lru {
    entries: Vec<(PathBuf, Playlist)>,
    cap: usize,
}

impl Lru {
    fn new(cap: usize) -> Self {
        Self {
            entries: Vec::new(),
            cap,
        }
    }

    /// The playlist of `source`, moved to the front.
    fn get_mut(&mut self, source: &Path) -> Option<&mut Playlist> {
        let i = self.entries.iter().position(|(k, _)| k == source)?;
        let entry = self.entries.remove(i);
        self.entries.insert(0, entry);
        self.entries.first_mut().map(|(_, p)| p)
    }

    fn peek_mut(&mut self, source: &Path) -> Option<&mut Playlist> {
        self.entries.iter_mut().find(|(k, _)| k == source).map(|(_, p)| p)
    }

    fn insert(&mut self, source: PathBuf, playlist: Playlist) {
        self.entries.retain(|(k, _)| *k != source);
        self.entries.insert(0, (source, playlist));
        self.entries.truncate(self.cap);
    }
}

struct Slot {
    sink: Option<Sink>,
    source: Option<PathBuf>,
    started_at: Option<Instant>,
    /// Where in the track playback started (a resumed position): the
    /// position is `offset + started_at.elapsed()`. Without it, a track
    /// resumed twice saved only the time since its last resume, and came
    /// back at the wrong place the third time.
    offset: Duration,
    total_duration: Option<Duration>,
    /// Correction de gain de normalisation en dB (`index.rs`), appliquée en
    /// plus du fondu (`Engine::slot_gain_linear`).
    gain_db: f32,
}

impl Slot {
    fn empty() -> Self {
        Self {
            sink: None,
            source: None,
            started_at: None,
            offset: Duration::ZERO,
            total_duration: None,
            gain_db: 0.0,
        }
    }

    fn position(&self) -> Option<Duration> {
        self.started_at.map(|s| self.offset + s.elapsed())
    }
}

/// Ordre de lecture d'une source + position mémorisée pour reprendre là où
/// elle en était au retour (MUSIQUE§5.4).
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
    /// la liste est épuisée (jamais un tirage à chaque piste, MUSIQUE§5.4). Un
    /// fichier seul est une liste d'une piste : il repart donc du début, ce
    /// qui est exactement la boucle voulue (§16.3).
    fn advance(&mut self, shuffle: bool) -> Option<IndexedTrack> {
        if self.order.is_empty() {
            return None;
        }
        let previous = self.current();
        self.pos += 1;
        self.elapsed_at_pause = Duration::ZERO;
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

/// The playlist no longer matches the source: a track added, removed or
/// renamed. Both lists come from `scan::list_tracks`, sorted, so a pairwise
/// comparison is enough.
fn playlist_is_stale(tracks: &[IndexedTrack], on_disk: &[PathBuf]) -> bool {
    tracks.len() != on_disk.len() || tracks.iter().zip(on_disk).any(|(t, p)| &t.path != p)
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

/// Permutation Fisher-Yates (MUSIQUE§5.4) + contrainte : si la première piste de la
/// nouvelle permutation est la dernière jouée, l'échanger avec la seconde.
pub fn shuffle_order(tracks: &[IndexedTrack], last_played: Option<&IndexedTrack>) -> Vec<IndexedTrack> {
    let mut order = tracks.to_vec();
    fastrand::shuffle(&mut order);
    apply_no_repeat_constraint(&mut order, last_played);
    order
}

/// Crossfade à puissance constante (MUSIQUE§5.2). `t` ∈ [0,1]. Invariant :
/// `gain_out² + gain_in² == 1`, contrairement à un fondu linéaire qui produit
/// un creux de volume audible au milieu de la transition.
pub fn crossfade_gains(t: f32) -> (f32, f32) {
    let t = t.clamp(0.0, 1.0) * std::f32::consts::FRAC_PI_2;
    (t.cos(), t.sin())
}

/// Fondu simple, courbe quadratique (MUSIQUE§5.2) — l'oreille perçoit le volume de
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
/// sens du fondu. Elle se combine donc avec `to + (from-to)*gain` à la place.
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
    paths: Paths,
    /// See `spawn`.
    on_track: Box<dyn Fn(PathBuf) + Send>,
    handle: OutputStreamHandle,
    config: MusicConfig,
    slots: [Slot; 2],
    active_slot: usize,
    playlists: Lru,
    state: State,
    fade: Option<ActiveFade>,
    /// The context applied — what plays, or will play once music is wanted.
    context: Context,
    /// A context asked for while music plays, waiting for `RESOLVE_DELAY`.
    pending: Debounced<Context>,
    flags: Flags,
}

impl Engine {
    fn handle_command(&mut self, cmd: EngineCommand) {
        match cmd {
            EngineCommand::UpdateConfig(cfg) => self.update_config(cfg),
            EngineCommand::EnterBigPicture => self.set_flags(|f| f.big_picture = true),
            EngineCommand::ExitBigPicture => self.set_flags(|f| f.big_picture = false),
            EngineCommand::SetMinimized(on) => self.set_flags(|f| f.minimized = on),
            EngineCommand::PreviewStarted(g) => self.set_flags(|f| f.preview = Some(g)),
            EngineCommand::PreviewEnded(g) => self.set_flags(|f| {
                // An older preview ending after a newer one started must not
                // bring the ambience back over it.
                if f.preview == Some(g) {
                    f.preview = None;
                }
            }),
            EngineCommand::SetContext(ctx) => self.set_context(ctx),
            EngineCommand::EnterSession => self.enter_session(),
            EngineCommand::ExitSession => self.exit_session(),
            EngineCommand::AcProcessStarted => self.flags.ac_running = true,
            EngineCommand::AcProcessStopped => {
                self.flags.ac_running = false;
                if self.state == State::Session {
                    // Filet de sécurité : la mémoire partagée n'a pas
                    // signalé la sortie de AC_LIVE (fermeture brutale) —
                    // sans ça la musique resterait coupée indéfiniment.
                    self.exit_session();
                } else {
                    // Music wanted while the game was running (always mode,
                    // or Big Picture opened during a race): it starts now.
                    self.reconcile();
                }
            }
        }
    }

    fn update_config(&mut self, cfg: MusicConfig) {
        self.config = cfg;
        // Un changement de volume dans les réglages doit s'entendre tout de
        // suite, pas seulement à la prochaine transition.
        if self.fade.is_none() {
            self.set_slot_volume(self.active_slot, self.effective_volume());
        }
        // A change of mode starts or stops the music on the spot — set from
        // inside Big Picture, "off" used to change nothing to the ear until
        // the next entry (reported bug, kept fixed).
        self.reconcile();
        // A source changed in the settings is applied at once, without the
        // delay meant for navigation: the user asked for it explicitly.
        self.apply_context();
    }

    fn set_flags(&mut self, change: impl FnOnce(&mut Flags)) {
        change(&mut self.flags);
        self.reconcile();
    }

    /// Starts or stops the music after a change of what decides it (mode,
    /// Big Picture, window, preview). The session has its own transitions.
    fn reconcile(&mut self) {
        let wanted = music_wanted(self.config.play_mode, self.flags);
        match self.state {
            State::Session => {}
            State::Idle if may_start(wanted, self.flags.ac_running, false) => self.start_playing(),
            State::Playing(_) if !wanted => self.fade_to_idle(),
            _ => {}
        }
    }

    fn set_context(&mut self, ctx: Context) {
        if matches!(self.state, State::Playing(_)) {
            self.pending.push(ctx, Instant::now());
        } else {
            // Nothing heard: no transition to smooth, the context simply
            // becomes what will play when music is wanted again.
            self.pending.clear();
            self.context = ctx;
            // The first context of a run is what starts "always" mode: nothing
            // else is sent at launch.
            self.reconcile();
        }
    }

    fn apply_due_context(&mut self) {
        if let Some(ctx) = self.pending.take_due(Instant::now()) {
            self.context = ctx;
            self.apply_context();
        }
    }

    /// Crossfades to the source of the current context — or does nothing at
    /// all when it is the one already playing (§16.6).
    fn apply_context(&mut self) {
        let State::Playing(playing) = &self.state else {
            return;
        };
        let target = self.current_source();
        if &target != playing {
            self.begin_crossfade(target, false);
        }
    }

    fn effective_volume(&self) -> f32 {
        self.config.music_volume.clamp(0.0, 1.0)
    }

    /// The source the current context plays. An assigned source that holds
    /// nothing (emptied, moved, a drive unplugged) falls back silently on the
    /// base ambience (MUSIQUE§9).
    fn current_source(&self) -> PathBuf {
        let base = self.config.source_for(&self.paths.root, None, self.context.base);
        let source = self
            .config
            .source_for(&self.paths.root, self.context.assigned.as_deref(), self.context.base);
        if source != base && super::scan::list_tracks(&source).is_empty() {
            log::warn!(
                "music: source attribuée vide ou introuvable, repli sur l'ambiance de base : {}",
                source.display()
            );
            return base;
        }
        source
    }

    /// Builds the source's playlist, or rebuilds it when its files changed on
    /// disk since. Checking only the source *path* is not enough: a folder
    /// picked while still empty, then filled with the app open, kept its empty
    /// playlist for good — real bug. Listing the folder is a `read_dir`, cheap
    /// on every transition; the costly index only runs when something changed.
    fn ensure_playlist(&mut self, source: &Path) -> &mut Playlist {
        let on_disk = super::scan::list_tracks(source);
        let stale = self
            .playlists
            .get_mut(source)
            .is_none_or(|pl| playlist_is_stale(&pl.tracks, &on_disk));
        if stale {
            let tracks = index::indexed_tracks(&self.paths.cache, source);
            self.playlists
                .insert(source.to_path_buf(), Playlist::load(tracks, self.config.shuffle));
        }
        self.playlists.get_mut(source).expect("playlist just ensured")
    }

    /// Facteur linéaire de la correction de normalisation (MUSIQUE§3.4) —
    /// toujours appliqué, sans réglage.
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
        // fichier (MUSIQUE§9 — sans ça, l'utilisateur ne peut plus renommer ni
        // supprimer ses pistes tant que l'app tourne).
        self.slots[slot] = Slot::empty();
    }

    /// Volume "ambiant" courant d'un slot (avant gain de normalisation), lu
    /// depuis le `Sink` plutôt que supposé : un fondu qui en interrompt un
    /// autre repart de là où le son en est vraiment.
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
    /// Sans ça, une transition interrompue par une autre laisse le slot
    /// abandonné jouer indéfiniment à son dernier volume — bug réel, entendu
    /// comme un décrochage suivi d'un retour de son.
    fn cancel_fade(&mut self) {
        match self.fade.take() {
            Some(ActiveFade::Crossfade { out_slot, .. }) => self.stop_slot(out_slot),
            Some(ActiveFade::SingleFade { slot, stop_at_end, .. }) if stop_at_end => self.stop_slot(slot),
            _ => {}
        }
    }

    /// Remembers where the active slot's track is, in its playlist.
    fn save_position(&mut self) {
        let slot = &self.slots[self.active_slot];
        let (Some(source), Some(position)) = (slot.source.clone(), slot.position()) else {
            return;
        };
        if let Some(pl) = self.playlists.peek_mut(&source) {
            pl.elapsed_at_pause = position;
        }
    }

    fn start_track_in_slot(&mut self, slot: usize, source: &Path, track: &IndexedTrack, resume_at: Duration) -> bool {
        let Ok(sink) = Sink::try_new(&self.handle) else {
            log::warn!("music: impossible de créer un lecteur audio");
            return false;
        };
        let Some(decoder) = open_track(&track.path) else {
            // Piste corrompue/illisible (MUSIQUE§9) : on journalise et on laisse
            // l'appelant décider de la suite.
            log::warn!("music: piste illisible, ignorée : {}", track.path.display());
            return false;
        };
        let total_duration = track.duration.or_else(|| decoder.total_duration());
        sink.set_volume(0.0);
        sink.append(decoder);
        let mut offset = resume_at;
        if resume_at > Duration::ZERO && sink.try_seek(resume_at).is_err() {
            log::warn!(
                "music: reprise de position impossible sur {}, redémarre du début",
                track.path.display()
            );
            offset = Duration::ZERO;
        }
        self.slots[slot] = Slot {
            sink: Some(sink),
            source: Some(source.to_path_buf()),
            started_at: Some(Instant::now()),
            offset,
            total_duration,
            gain_db: track.gain_db,
        };
        // Every audible change goes through here — next track, a change of
        // ambience, the first track, a resume after a session — and each one
        // is "the music changed" to whoever is listening.
        if self.config.show_now_playing {
            (self.on_track)(track.path.clone());
        }
        true
    }

    /// The track to start for `source`: the remembered one at its position
    /// (`advance = false`, a change of ambience), or the next (`advance =
    /// true`, end of track).
    fn next_track(&mut self, source: &Path, advance: bool) -> Option<(IndexedTrack, Duration)> {
        let shuffle = self.config.shuffle;
        let pl = self.ensure_playlist(source);
        if advance {
            pl.advance(shuffle).map(|t| (t, Duration::ZERO))
        } else {
            let at = pl.elapsed_at_pause;
            pl.current().map(|t| (t, at))
        }
    }

    fn begin_crossfade(&mut self, target: PathBuf, advance: bool) {
        self.save_position();
        // Avant de choisir `next_slot` : un fondu interrompu doit être résolu
        // avant qu'on décide où charger la piste suivante.
        self.cancel_fade();
        let Some((track, resume_at)) = self.next_track(&target, advance) else {
            log::warn!(
                "music: source vide ou introuvable, transition ignorée : {}",
                target.display()
            );
            return;
        };
        let next_slot = 1 - self.active_slot;
        if !self.start_track_in_slot(next_slot, &target, &track, resume_at) {
            return;
        }
        self.fade = Some(ActiveFade::Crossfade {
            out_slot: self.active_slot,
            in_slot: next_slot,
            start: Instant::now(),
            duration: Duration::from_millis(self.config.crossfade_ms as u64),
        });
        self.active_slot = next_slot;
        self.state = State::Playing(target);
    }

    fn check_track_end(&mut self) {
        if self.fade.is_some() {
            return;
        }
        let State::Playing(source) = &self.state else { return };
        let slot = &self.slots[self.active_slot];
        let Some(sink) = &slot.sink else { return };
        let should_advance = match (slot.position(), slot.total_duration) {
            (Some(position), Some(total)) => {
                let margin = Duration::from_millis(self.config.crossfade_ms as u64 + 500);
                total.checked_sub(margin).is_some_and(|threshold| position >= threshold)
            }
            _ => sink.empty(),
        };
        if should_advance {
            let source = source.clone();
            self.begin_crossfade(source, true);
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

    /// Fades the current context's source in, from its remembered position.
    fn start_playing(&mut self) {
        // A context still waiting is the latest one asked for: nothing is
        // heard yet, so no reason to make it wait any longer.
        if let Some((ctx, _)) = self.pending.pending.take() {
            self.context = ctx;
        }
        let source = self.current_source();
        self.cancel_fade();
        let Some((track, resume_at)) = self.next_track(&source, false) else {
            log::warn!("music: source vide ou introuvable, rien ne joue : {}", source.display());
            return;
        };
        let slot = self.active_slot;
        if self.start_track_in_slot(slot, &source, &track, resume_at) {
            self.fade = Some(ActiveFade::SingleFade {
                slot,
                start: Instant::now(),
                duration: Duration::from_millis(self.config.fade_in_ms as u64),
                from: 0.0,
                to: self.effective_volume(),
                stop_at_end: false,
            });
            self.state = State::Playing(source);
        }
    }

    /// Fades out and stops, keeping the position for the next start. Starts
    /// from the volume actually reached, not the full one: leaving during a
    /// fade-in must not jump up before going down (real bug).
    fn fade_out(&mut self) {
        self.save_position();
        let slot = self.active_slot;
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
    }

    fn fade_to_idle(&mut self) {
        self.fade_out();
        self.pending.clear();
        self.state = State::Idle;
    }

    fn enter_session(&mut self) {
        match self.state {
            State::Session => {}
            // Rien ne joue : juste mémoriser que la voiture est en piste.
            State::Idle => self.state = State::Session,
            // Musique toujours coupée pendant la session — jamais de "duck"
            // en fond (décidé avec l'utilisateur).
            State::Playing(_) => {
                self.fade_out();
                self.pending.clear();
                self.state = State::Session;
            }
        }
    }

    fn exit_session(&mut self) {
        if self.state != State::Session {
            return;
        }
        // Back to Idle, then whatever is wanted now plays: the context the
        // screen asked for during the session (the results screen, the
        // session settings), not the one left at launch. Directly, not
        // through `reconcile`: the game still runs when the car goes back to
        // the pits, and that guard is for a start, not for a return.
        self.state = State::Idle;
        if may_start(
            music_wanted(self.config.play_mode, self.flags),
            self.flags.ac_running,
            true,
        ) {
            self.start_playing();
        }
    }
}

// --- Écoute au clic (MUSIQUE§6, bouton ▶) --------------------------------------
//
// Une prévisualisation ouvre son propre `OutputStream` éphémère (plusieurs flux
// WASAPI en mode partagé coexistent sans se couper la parole), et l'ambiance
// s'efface le temps qu'elle dure (`PreviewStarted`/`PreviewEnded`) : on écoute
// le choix, pas les deux en même temps, et rien ne change durablement.

pub struct PreviewHandle {
    stop: Mutex<Option<Sender<()>>>,
    engine: Mutex<Sender<EngineCommand>>,
    generation: AtomicU64,
}

impl PreviewHandle {
    pub fn new(engine: Sender<EngineCommand>) -> Self {
        Self {
            stop: Mutex::new(None),
            engine: Mutex::new(engine),
            generation: AtomicU64::new(0),
        }
    }

    fn tell_engine(&self, cmd: EngineCommand) {
        if let Ok(tx) = self.engine.lock() {
            let _ = tx.send(cmd);
        }
    }

    /// `on_end` runs once the preview is over, stopped or played to the end:
    /// the facade tells the front, whose ■ would otherwise outlive the track.
    pub fn start(&self, track: PathBuf, volume: f32, on_end: impl FnOnce() + Send + 'static) {
        let generation = self.generation.fetch_add(1, Ordering::SeqCst) + 1;
        // Started before the old one is stopped: its "ended" then arrives for
        // a generation the engine no longer waits for, and the ambience does
        // not come back for a moment between two previews.
        self.tell_engine(EngineCommand::PreviewStarted(generation));
        self.stop();
        let (tx, rx) = mpsc::channel::<()>();
        let engine = self.engine.lock().map(|e| e.clone()).ok();
        std::thread::spawn(move || {
            play_preview(&track, volume, &rx);
            if let Some(engine) = engine {
                let _ = engine.send(EngineCommand::PreviewEnded(generation));
            }
            on_end();
        });
        if let Ok(mut guard) = self.stop.lock() {
            *guard = Some(tx);
        }
    }

    pub fn stop(&self) {
        if let Ok(mut guard) = self.stop.lock() {
            if let Some(tx) = guard.take() {
                let _ = tx.send(());
            }
        }
    }
}

/// Plays one track until it ends or `stop` is signalled.
fn play_preview(track: &Path, volume: f32, stop: &Receiver<()>) {
    let Ok((_stream, handle)) = OutputStream::try_default() else {
        return;
    };
    let Ok(sink) = Sink::try_new(&handle) else { return };
    let Some(decoder) = open_track(track) else { return };
    sink.set_volume(volume.clamp(0.0, 1.0));
    sink.append(decoder);
    loop {
        // `stop()` called, or the handle dropped with the app.
        if !matches!(
            stop.recv_timeout(Duration::from_millis(150)),
            Err(mpsc::RecvTimeoutError::Timeout)
        ) {
            break;
        }
        if sink.empty() {
            break; // piste terminée
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
        // Bug réel (MUSIQUE§5.2) : en combinant `fade_out_gain` (déjà une courbe de
        // gain 1→0) au même lerp que `fade_in_gain` (une progression 0→1), le
        // volume d'un fondu de sortie partait de 0 pour remonter jusqu'à
        // `from` — la saccade entendue en sortie de Big Picture.
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

    // Rule (§16.3): a file source loops — its one-track playlist starts the
    // same track again, from the start.
    #[test]
    fn a_one_track_playlist_loops_from_the_start() {
        let mut pl = Playlist::load(vec![track("loop.ogg")], true);
        pl.elapsed_at_pause = Duration::from_secs(40);
        assert_eq!(pl.advance(true), Some(track("loop.ogg")));
        assert_eq!(pl.elapsed_at_pause, Duration::ZERO, "a new pass starts at 0");
    }

    #[test]
    fn playlist_of_empty_folder_never_advances() {
        let mut pl = Playlist::load(Vec::new(), true);
        assert_eq!(pl.current(), None);
        assert_eq!(pl.advance(true), None);
    }

    // A folder picked while empty then filled with the app open must not keep
    // its empty playlist (bug: "3 tracks detected" in the settings, silence in
    // Big Picture until a restart). Any change of the file list rebuilds it.
    #[test]
    fn playlist_is_stale_whenever_the_folder_listing_changed() {
        let paths = |names: &[&str]| names.iter().map(PathBuf::from).collect::<Vec<_>>();
        let built = vec![track("a.mp3"), track("b.mp3")];

        assert!(
            playlist_is_stale(&[], &paths(&["a.mp3"])),
            "empty playlist, folder since filled"
        );
        assert!(
            playlist_is_stale(&built, &paths(&["a.mp3", "b.mp3", "c.mp3"])),
            "track added"
        );
        assert!(playlist_is_stale(&built, &paths(&["a.mp3"])), "track removed");
        assert!(playlist_is_stale(&built, &paths(&["a.mp3", "z.mp3"])), "track renamed");
        assert!(
            !playlist_is_stale(&built, &paths(&["a.mp3", "b.mp3"])),
            "same files: position and order are kept"
        );
        assert!(!playlist_is_stale(&[], &[]), "still empty: nothing to rebuild");
    }

    // Rule (§16.6): a resolution waits RESOLVE_DELAY, and only the last one
    // pushed counts — scrolling through five sheets in a second fades once.
    #[test]
    fn only_the_last_context_pushed_is_applied_after_the_delay() {
        let t0 = Instant::now();
        let mut d = Debounced::new(Duration::from_secs(2));
        d.push("brand", t0);
        d.push("family", t0 + Duration::from_millis(500));
        d.push("country", t0 + Duration::from_millis(1200));
        assert_eq!(
            d.take_due(t0 + Duration::from_millis(2100)),
            None,
            "2 s not yet elapsed since the last"
        );
        assert_eq!(
            d.take_due(t0 + Duration::from_millis(3200)),
            Some("country"),
            "the last one wins"
        );
        assert_eq!(d.take_due(t0 + Duration::from_secs(10)), None, "applied once");
    }

    // Rule (§16.6): only a handful of sources keep their position; the least
    // recently heard is forgotten first, and hearing one refreshes it.
    #[test]
    fn the_least_recently_heard_playlist_is_dropped_first() {
        let mut lru = Lru::new(2);
        lru.insert("a".into(), Playlist::load(vec![track("a1")], false));
        lru.insert("b".into(), Playlist::load(vec![track("b1")], false));
        assert!(
            lru.get_mut(Path::new("a")).is_some(),
            "a heard again: now the most recent"
        );
        lru.insert("c".into(), Playlist::load(vec![track("c1")], false));
        assert!(lru.get_mut(Path::new("b")).is_none(), "b was the least recent: dropped");
        assert!(lru.get_mut(Path::new("a")).is_some() && lru.get_mut(Path::new("c")).is_some());
    }

    // Rule (§16.1): the play mode says where music is heard; a minimised
    // window and a preview silence it in every mode.
    #[test]
    fn the_play_mode_decides_where_music_is_heard() {
        let windowed = Flags::default();
        let big_picture = Flags {
            big_picture: true,
            ..Flags::default()
        };
        assert!(!music_wanted(PlayMode::Off, big_picture), "off: never");
        assert!(
            !music_wanted(PlayMode::BigPicture, windowed),
            "Big Picture only: not windowed"
        );
        assert!(music_wanted(PlayMode::BigPicture, big_picture));
        assert!(music_wanted(PlayMode::Always, windowed), "always: windowed too");
        let minimized = Flags {
            minimized: true,
            ..Flags::default()
        };
        assert!(!music_wanted(PlayMode::Always, minimized), "minimised: faded out");
        let previewing = Flags {
            preview: Some(3),
            ..big_picture
        };
        assert!(!music_wanted(PlayMode::BigPicture, previewing), "a preview plays alone");
    }

    // Rule (§16.1): the music comes back when the car returns to the pits,
    // the game still running; it never STARTS while the game runs.
    #[test]
    fn music_returns_after_the_track_but_never_starts_during_the_game() {
        assert!(may_start(true, true, true), "back to the pits: music again");
        assert!(!may_start(true, true, false), "Big Picture opened mid-race: silent");
        assert!(may_start(true, false, false), "no game: starts");
        assert!(!may_start(false, false, true), "not wanted: never");
    }

    // The position of a track resumed once is where it was resumed PLUS the
    // time played since — not the time since the resume alone, which brought
    // a track back at the wrong place on the third visit.
    #[test]
    fn a_resumed_slot_counts_its_position_from_the_resume_point() {
        let slot = Slot {
            started_at: Some(Instant::now() - Duration::from_secs(5)),
            offset: Duration::from_secs(60),
            ..Slot::empty()
        };
        let pos = slot.position().unwrap();
        assert!(
            pos >= Duration::from_secs(65) && pos < Duration::from_secs(66),
            "{pos:?}"
        );
    }
}
