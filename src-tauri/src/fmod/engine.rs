//! The thread that owns the FMOD system, and the only one allowed to touch it.
//!
//! `FMOD_Studio_System_Update` has to be called regularly — mixing bookkeeping
//! and the freeing of stopped instances both happen there — so the system needs
//! a thread of its own rather than a call site. The project already has this
//! shape in `music/engine.rs`, and this follows it: one thread, one channel,
//! commands in, nothing shared out. See `docs/SPEC-engine-sound-fmod.md` §4.3.
//!
//! The FMOD types make that discipline structural rather than a convention:
//! `System` holds raw pointers, so it is not `Send`, and the compiler refuses
//! any attempt to reach it from elsewhere.

use std::path::PathBuf;
use std::sync::mpsc::{self, Receiver, Sender};
use std::sync::Mutex;
use std::time::Duration;

use super::guids::Guid;
use super::params::{self, Roles};
use super::sys::{Bank, Fmod, PlaybackState, System};

/// How often the system is pumped while idle or playing.
///
/// 20 ms is the cadence lot 0 drove a rev sweep at, and it was smooth to the
/// ear. It also bounds how long a `Stop` waits before being acted on.
const TICK: Duration = Duration::from_millis(20);

/// How long to wait for an event's samples before playing anyway.
///
/// Sample data is **not** loaded by `LoadBankFile`; without waiting, playback
/// starts on silence and fades in as FMOD streams. A large bank on a cold cache
/// is the slow case, hence seconds rather than milliseconds — but this is a
/// ceiling, not a delay: the wait ends as soon as FMOD says loaded.
const SAMPLE_WAIT: Duration = Duration::from_secs(10);

/// What the caller wants played. Everything here is resolved **before** the
/// message is sent, by pure code that needs no DLL — the thread does no lookup.
#[derive(Debug, Clone)]
pub struct PlayRequest {
    /// The Assetto Corsa install, source of the DLLs and of the master bank.
    pub ac_root: PathBuf,
    /// The `.bank` to load — in the library for a mod, in the game for stock.
    pub bank: PathBuf,
    pub guid: Guid,
    pub event_path: String,
    pub rev: f32,
    pub throttle: f32,
}

/// What actually happened, so the interface can show it and the caller can fall
/// back to the in-house decoder when it did not work.
#[derive(Debug, Clone, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PlayReport {
    pub event_path: String,
    /// Name and range of the rev parameter, when one was recognised. This is
    /// what the slider of §4.4 binds to — never a hardcoded range.
    pub rev_param: Option<String>,
    pub rev_min: Option<f32>,
    pub rev_max: Option<f32>,
    pub throttle_param: Option<String>,
}

pub enum Command {
    Play(PlayRequest, Sender<Result<PlayReport, String>>),
    SetRev(f32),
    SetThrottle(f32),
    Stop,
}

/// Managed as Tauri state. The `Mutex` guarantees `Sync` without depending on
/// which std version made `Sender` `Sync` — same reasoning as the music engine.
pub struct FmodEngineHandle(Mutex<Sender<Command>>);

impl FmodEngineHandle {
    fn send(&self, cmd: Command) {
        if let Ok(tx) = self.0.lock() {
            // The thread only exits when the channel closes, which only happens
            // as the app shuts down.
            let _ = tx.send(cmd);
        }
    }

    /// Starts playback and waits for the verdict.
    ///
    /// Blocking on purpose: the caller has to know whether the native path
    /// worked in order to fall back to the WAV decoder (§4.1), and the answer
    /// costs a bank load, not a user-visible wait.
    pub fn play(&self, request: PlayRequest) -> Result<PlayReport, String> {
        let (tx, rx) = mpsc::channel();
        self.send(Command::Play(request, tx));
        rx.recv().map_err(|_| "the FMOD thread is gone".to_string())?
    }

    pub fn set_rev(&self, value: f32) {
        self.send(Command::SetRev(value));
    }

    pub fn set_throttle(&self, value: f32) {
        self.send(Command::SetThrottle(value));
    }

    pub fn stop(&self) {
        self.send(Command::Stop);
    }
}

/// Starts the thread. Nothing is loaded yet: the DLLs are only touched on the
/// first `Play`, so an install without Assetto Corsa never pays for this.
pub fn spawn() -> FmodEngineHandle {
    let (tx, rx) = mpsc::channel();
    std::thread::spawn(move || run(rx));
    FmodEngineHandle(Mutex::new(tx))
}

/// What is loaded right now. Rebuilt when the game path changes, which in
/// practice means "never, after the first play".
struct Loaded {
    ac_root: PathBuf,
    system: System,
    /// The car bank currently loaded, if any. Exactly one at a time: two sound
    /// mods for the same car declare the **same event GUIDs**, so leaving the
    /// previous one loaded would make `GetEventByID` return the wrong bank's
    /// event — and it would look like the mod simply sounds identical.
    bank: Option<(PathBuf, Bank)>,
}

struct Playing {
    instance: super::sys::EventInstance,
    roles: Roles,
}

fn run(rx: Receiver<Command>) {
    let mut loaded: Option<Loaded> = None;
    let mut playing: Option<Playing> = None;

    loop {
        match rx.recv_timeout(TICK) {
            Ok(Command::Play(request, reply)) => {
                stop_current(&loaded, &mut playing);
                let outcome = start(&mut loaded, &mut playing, request);
                if let Err(e) = &outcome {
                    // Not an error the user sees: it is the switch to the
                    // in-house decoder (§4.2). But an install packaged as an
                    // .exe has no console, so without this line a bug report
                    // carries nothing at all.
                    log::warn!("fmod: native audition unavailable, falling back to the decoder: {e}");
                }
                let _ = reply.send(outcome);
            }
            Ok(Command::SetRev(value)) => set_role(&loaded, &playing, |roles| roles.rev.as_ref(), value),
            Ok(Command::SetThrottle(value)) => set_role(&loaded, &playing, |roles| roles.throttle.as_ref(), value),
            Ok(Command::Stop) => stop_current(&loaded, &mut playing),
            Err(mpsc::RecvTimeoutError::Timeout) => {}
            Err(mpsc::RecvTimeoutError::Disconnected) => break,
        }

        // Every tick, whether or not anything is playing: this is where FMOD
        // mixes and where a stopped instance is actually freed.
        if let Some(l) = &loaded {
            if let Err(e) = l.system.update() {
                log::warn!("fmod: update failed: {e}");
            }
        }
    }

    // Dropping the system releases banks and instances with it.
    stop_current(&loaded, &mut playing);
}

fn start(
    loaded: &mut Option<Loaded>,
    playing: &mut Option<Playing>,
    request: PlayRequest,
) -> Result<PlayReport, String> {
    // Rebuild the system if the game path changed under us (a settings edit).
    if loaded.as_ref().is_some_and(|l| l.ac_root != request.ac_root) {
        *loaded = None;
    }

    if loaded.is_none() {
        let fmod = Fmod::load(&request.ac_root).map_err(|e| e.to_string())?;
        let system = System::new(fmod).map_err(|e| e.to_string())?;
        // The master bank carries the buses every car event routes into, so it
        // goes first. Missing or refused, it is not fatal on its own — say so
        // and let the car bank decide.
        let master = request.ac_root.join("content").join("sfx").join("common.bank");
        if let Err(e) = system.load_bank(&master) {
            log::warn!("fmod: master bank {} not loaded: {e}", master.display());
        }
        *loaded = Some(Loaded {
            ac_root: request.ac_root.clone(),
            system,
            bank: None,
        });
    }

    let l = loaded.as_mut().expect("just built");

    // Swap the car bank only when it actually changes: reloading a 30 MB bank
    // to play the same mod again would be pure waste.
    let already = l.bank.as_ref().is_some_and(|(path, _)| *path == request.bank);
    if !already {
        if let Some((_, previous)) = l.bank.take() {
            if let Err(e) = l.system.unload_bank(previous) {
                log::warn!("fmod: could not unload the previous bank: {e}");
            }
        }
        let bank = l.system.load_bank(&request.bank).map_err(|e| e.to_string())?;
        l.bank = Some((request.bank.clone(), bank));
    }

    let desc = l.system.event(&request.guid).map_err(|e| e.to_string())?;
    let parameters = l.system.parameters(desc).map_err(|e| e.to_string())?;
    let roles = params::classify(&parameters);

    // Samples are not implicit in the bank load; without this the first few
    // hundred milliseconds are silent.
    l.system.load_sample_data(desc).map_err(|e| e.to_string())?;
    let deadline = std::time::Instant::now() + SAMPLE_WAIT;
    while std::time::Instant::now() < deadline {
        if l.system.samples_loaded(desc).map_err(|e| e.to_string())? {
            break;
        }
        let _ = l.system.update();
        std::thread::sleep(TICK);
    }

    let instance = l.system.create_instance(desc).map_err(|e| e.to_string())?;

    // Set both before starting, so the very first mixed block is already at the
    // requested engine speed rather than sliding up to it.
    if let Some(p) = &roles.throttle {
        let _ = l
            .system
            .set_parameter(instance, &p.name, request.throttle.clamp(p.min, p.max));
    }
    if let Some(p) = &roles.rev {
        let _ = l
            .system
            .set_parameter(instance, &p.name, request.rev.clamp(p.min, p.max));
    }

    l.system.start(instance).map_err(|e| e.to_string())?;

    // One pump, then check. "Started without error" and "actually audible" are
    // different claims: an instance can come back virtual (stolen voice) or
    // already stopped, and that is precisely the case where a user reports
    // clicking and hearing nothing. Not fatal — the event is playing as far as
    // FMOD is concerned — but it must leave a trace.
    let _ = l.system.update();
    match l.system.playback_state(instance) {
        Ok(PlaybackState::Playing | PlaybackState::Starting) => {}
        Ok(other) => log::warn!("fmod: {} started but is {other:?}", request.event_path),
        Err(e) => log::warn!("fmod: could not read the playback state: {e}"),
    }

    let report = PlayReport {
        event_path: request.event_path,
        rev_param: roles.rev.as_ref().map(|p| p.name.clone()),
        rev_min: roles.rev.as_ref().map(|p| p.min),
        rev_max: roles.rev.as_ref().map(|p| p.max),
        throttle_param: roles.throttle.as_ref().map(|p| p.name.clone()),
    };
    *playing = Some(Playing { instance, roles });
    Ok(report)
}

/// Applies a value to whichever role the selector picks, clamped to the range
/// the event declared. Silently does nothing when nothing is playing, or when
/// this event has no such parameter — an event with no rev parameter still
/// plays, it just cannot be revved (§2.4).
fn set_role(
    loaded: &Option<Loaded>,
    playing: &Option<Playing>,
    select: impl Fn(&Roles) -> Option<&params::ParamInfo>,
    value: f32,
) {
    let (Some(l), Some(p)) = (loaded, playing) else { return };
    let Some(param) = select(&p.roles) else { return };
    if let Err(e) = l
        .system
        .set_parameter(p.instance, &param.name, value.clamp(param.min, param.max))
    {
        log::warn!("fmod: could not set {}: {e}", param.name);
    }
}

fn stop_current(loaded: &Option<Loaded>, playing: &mut Option<Playing>) {
    let (Some(l), Some(p)) = (loaded, playing.take()) else {
        *playing = None;
        return;
    };
    // Immediate rather than fading out: this is an audition the user just
    // dismissed, and a tail would play over whatever they clicked next.
    if let Err(e) = l.system.stop(p.instance) {
        log::warn!("fmod: stop failed: {e}");
    }
    let _ = l.system.update();
    if let Err(e) = l.system.release_instance(p.instance) {
        log::warn!("fmod: releasing the instance failed: {e}");
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// End-to-end through the real thread: loads the game's DLLs, plays an
    /// engine event, moves the rev parameter while it plays, stops.
    ///
    /// **Ignored by default, and never run in CI.** It needs an actual Assetto
    /// Corsa install and an audio device, neither of which a build agent has —
    /// this is the one part of the FMOD path that no amount of pure testing can
    /// cover, so it exists to be run by hand rather than not to exist:
    ///
    /// ```text
    /// PITBOX_AC_ROOT="D:\SteamLibrary\steamapps\common\assettocorsa" \
    ///   cargo test --lib fmod::engine -- --ignored --nocapture
    /// ```
    ///
    /// Without the variable it reports and returns, so a stray `--ignored` run
    /// on a machine with no game is a pass rather than a spurious failure.
    #[test]
    #[ignore = "needs a real Assetto Corsa install and an audio device"]
    fn plays_a_real_engine_event_end_to_end() {
        let Ok(ac_root) = std::env::var("PITBOX_AC_ROOT") else {
            eprintln!("PITBOX_AC_ROOT unset — nothing to play against, skipping");
            return;
        };
        let ac_root = PathBuf::from(ac_root);
        let car = std::env::var("PITBOX_AC_CAR").unwrap_or_else(|_| "ks_ford_gt40".to_string());
        let bank_dir = ac_root.join("content").join("cars").join(&car).join("sfx");

        let (event_path, guid) =
            super::super::guids::resolve_engine_event(&bank_dir, Some(&ac_root), &car, Default::default())
                .unwrap_or_else(|| panic!("no engine event for {car}"));
        let bank = std::fs::read_dir(&bank_dir)
            .expect("read the car's sfx folder")
            .flatten()
            .map(|e| e.path())
            .find(|p| p.extension().is_some_and(|e| e.eq_ignore_ascii_case("bank")))
            .unwrap_or_else(|| panic!("no .bank in {}", bank_dir.display()));

        let handle = spawn();
        let report = handle
            .play(PlayRequest {
                ac_root,
                bank,
                guid,
                event_path: event_path.clone(),
                rev: 900.0,
                throttle: 0.0,
            })
            .expect("the native path must work against a real install");

        eprintln!("playing {event_path}");
        eprintln!(
            "  rev      {:?} {:?}..{:?}",
            report.rev_param, report.rev_min, report.rev_max
        );
        eprintln!("  throttle {:?}", report.throttle_param);
        assert!(
            report.rev_param.is_some(),
            "a stock car must expose a recognisable rev parameter"
        );

        // Move it while it plays — the mechanic the rev slider depends on, and
        // a different thing from setting it before `start`.
        for rev in [900.0, 2000.0, 4000.0, 6000.0, 3000.0, 900.0] {
            handle.set_rev(rev);
            handle.set_throttle(if rev > 2500.0 { 1.0 } else { 0.0 });
            std::thread::sleep(Duration::from_millis(400));
        }

        handle.stop();
        std::thread::sleep(Duration::from_millis(200));
    }
}
