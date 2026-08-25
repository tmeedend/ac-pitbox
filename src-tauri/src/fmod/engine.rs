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
use super::sys::{Attributes3d, Bank, Fmod, PlaybackState, Reverb, System};

/// How often the system is pumped while idle or playing.
///
/// 20 ms is the cadence lot 0 drove a rev sweep at, and it was smooth to the
/// ear. It also bounds how long a `Stop` waits before being acted on.
const TICK: Duration = Duration::from_millis(20);

/// A play taking longer than this is worth a line in the log.
///
/// The ignition key spins while it waits, so nothing looks broken — which is
/// exactly why a slow one goes unnoticed until someone complains about it.
const SLOW_PLAY: Duration = Duration::from_millis(600);

/// How long to wait for an event's samples before playing anyway.
///
/// Sample data is **not** loaded by `LoadBankFile`; without waiting, playback
/// starts on silence and fades in as FMOD streams. A large bank on a cold cache
/// is the slow case, hence seconds rather than milliseconds — but this is a
/// ceiling, not a delay: the wait ends as soon as FMOD says loaded.
const SAMPLE_WAIT: Duration = Duration::from_secs(10);

/// How much of the room to mix in, in decibels.
///
/// The one number here that is a matter of taste rather than of physics, and it
/// wants to stay timid: the engine samples already carry the acoustics of
/// wherever they were recorded, so a room laid generously on top of that gives
/// a bathroom rather than a showroom.
pub const DEFAULT_REVERB_WET_DB: f32 = -14.0;

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
    /// Top of this car's rev range. Not decoration: the showcase routine needs
    /// it to know what "a good blip" means on *this* engine — the same 5000 rpm
    /// is the redline of a diesel van and half throttle on a Ferrari.
    pub rev_ceiling: f32,
    /// How much room to mix in. Carried per play rather than fixed at build
    /// time so that a dosage can be compared by ear without a rebuild — and so
    /// that it can become a setting later without moving anything.
    pub reverb_wet_db: f32,
    /// `event:/cars/<id>/limiter`, when the car has one. AC keeps the limiter
    /// out of the engine event entirely — it is its own sound, and the reason a
    /// rev-out is instantly recognisable.
    pub limiter_guid: Option<Guid>,
    /// Where that sound belongs, from the car's own physics. `None` means the
    /// limiter stays silent rather than being guessed at.
    pub limiter_rev: Option<f32>,
}

/// Where the ear sits relative to the car, in the terms the interface has:
/// an orbit angle, a height angle and a distance. Turning that into vectors is
/// this module's job, not the caller's.
#[derive(Debug, Clone, Copy)]
pub struct Listener {
    /// Degrees around the car. 0 faces the nose, 180 the tail.
    pub azimuth: f32,
    /// Degrees above the horizon, clamped: at the poles "up" stops being
    /// definable and the orientation degenerates.
    pub elevation: f32,
    pub distance: f32,
}

impl Default for Listener {
    fn default() -> Self {
        // Three-quarter front, at the distance someone would stand to listen —
        // the same idea as the 3D preview's default framing.
        Listener {
            azimuth: 35.0,
            elevation: 8.0,
            distance: 4.0,
        }
    }
}

impl Listener {
    /// The listener always **faces the car**, which is what a camera orbiting a
    /// model does. That has a consequence worth stating: the source stays dead
    /// ahead, so stereo panning barely moves. What changes is the *timbre*,
    /// through `Event Cone Angle` — measured, and exactly the difference
    /// between standing at the bonnet and standing at the exhaust.
    fn attributes(&self) -> Attributes3d {
        let az = self.azimuth.to_radians();
        let el = self.elevation.clamp(-80.0, 80.0).to_radians();
        let d = self.distance.max(0.5);
        let position = [az.sin() * el.cos() * d, el.sin() * d, az.cos() * el.cos() * d];
        let forward = normalize([-position[0], -position[1], -position[2]]);
        let world_up = [0.0, 1.0, 0.0];
        let right = normalize(cross(world_up, forward));
        let up = normalize(cross(forward, right));
        Attributes3d {
            position,
            velocity: [0.0; 3],
            forward,
            up,
        }
    }
}

fn cross(a: [f32; 3], b: [f32; 3]) -> [f32; 3] {
    [
        a[1] * b[2] - a[2] * b[1],
        a[2] * b[0] - a[0] * b[2],
        a[0] * b[1] - a[1] * b[0],
    ]
}

fn normalize(v: [f32; 3]) -> [f32; 3] {
    let len = (v[0] * v[0] + v[1] * v[1] + v[2] * v[2]).sqrt();
    if len <= f32::EPSILON {
        [0.0, 0.0, 1.0]
    } else {
        [v[0] / len, v[1] / len, v[2] / len]
    }
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
    SetListener(Listener),
    /// Start or stop the "showing off the engine" routine (§6bis).
    Showcase(bool),
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

    /// Moves the ear. Sent without waiting: this follows a camera being
    /// dragged, so it must never make the drag stutter.
    pub fn set_listener(&self, listener: Listener) {
        self.send(Command::SetListener(listener));
    }

    pub fn set_showcase(&self, on: bool) {
        self.send(Command::Showcase(on));
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
    /// `None` when the room could not be installed — the engine then plays dry,
    /// which is worse but perfectly usable.
    reverb: Option<Reverb>,
}

struct Playing {
    instance: super::sys::EventInstance,
    roles: Roles,
    /// The limiter, ready to be struck. Its description is kept rather than an
    /// instance: it only exists while the engine is actually against the stop.
    limiter: Option<Limiter>,
    rev_ceiling: f32,
    /// The engine speed the manual slider last asked for. The showcase borrows
    /// the engine while it runs and hands it back to this on the way out.
    manual_rev: f32,
    /// The throttle the slider's direction implies, while the showcase is not
    /// the one driving.
    throttle: Throttle,
    showcase: Option<Showcase>,
}

/// The limiter event, and the speed at which it belongs.
struct Limiter {
    description: super::sys::EventDesc,
    at_rev: f32,
    /// Playing right now. Started when the engine reaches the stop, stopped
    /// when it leaves — the same thing the game does.
    instance: Option<super::sys::EventInstance>,
    roles: Roles,
}

/// How far below the limit the sound still belongs.
///
/// Not zero: an engine held against its limiter oscillates around it rather
/// than sitting exactly on it, and a threshold with no width would make the
/// sound stutter on and off across that oscillation.
const LIMITER_MARGIN: f32 = 60.0;

/// Idle for a few seconds, then a handful of short throttle blips — someone
/// letting a bystander hear what the car has. §6bis.
///
/// It lives on the audio thread rather than in the interface for one reason:
/// a blip is 200 ms of rising engine speed, and driving that from the webview
/// would mean an IPC round trip per step. The 20 ms tick is already here.
struct Showcase {
    idle_rev: f32,
    ceiling: f32,
    /// Where the limiter actually is, when the car states one. Kept apart from
    /// `ceiling` because the two only coincide by accident — see
    /// [`REDLINE_FRACTION`].
    redline: f32,
    phase: Phase,
    /// Time inside the current phase. Fed by the caller rather than read from a
    /// clock, so the routine can be exercised in a test without sleeping
    /// through a minute of engine noise.
    elapsed: Duration,
    span: Duration,
    /// Whether the blip under way is one of the ones that reaches the limiter —
    /// the only ones allowed to hang there.
    at_limiter: bool,
    /// Blips left in the current burst.
    left: u32,
    /// Engine speed this blip is reaching for.
    peak: f32,
}

#[derive(Clone, Copy, PartialEq)]
enum Phase {
    /// Ticking over, waiting. This is most of the time.
    Idle,
    /// Foot going down.
    Attack,
    /// Held at the top, briefly.
    Hold,
    /// Foot off, revs falling.
    Release,
    /// Between two blips of the same burst.
    Gap,
}

/// How long the engine idles between two bursts. The user asked for 3 to 7
/// seconds, and the randomness is the point: a fixed interval reads as a
/// machine, an irregular one as somebody being pleased with their car.
const IDLE_SPAN: (f32, f32) = (3.0, 7.0);
const BLIPS_PER_BURST: (u32, u32) = (2, 5);

/// Milliseconds a blip takes to rise, before the part that depends on how far
/// it is reaching.
///
/// The first version drew this from a flat 150–260 ms and it was audibly wrong:
/// a ratio of 1.7 between the fastest and the slowest is not enough to hear, so
/// every prod of the throttle had the same urgency and the routine sounded
/// mechanical. Two things fix it — the reach-proportional part below, and the
/// occasional lazy one.
const ATTACK_BASE_MS: (u32, u32) = (110, 230);
/// …plus this much, in proportion to how far up the rev range the blip goes.
/// A real engine has inertia: a flick to 3000 rpm is over before a pull to the
/// limiter has got going. This is what makes a big blip *sound* big rather than
/// just end higher.
const ATTACK_REACH_MS: (u32, u32) = (150, 380);
/// And now and then, someone leaning on it rather than flicking it.
const LAZY_ODDS: f32 = 0.30;
const LAZY_FACTOR: (f32, f32) = (1.45, 2.10);

const HOLD_MS: (u32, u32) = (60, 150);
/// Held **against the limiter**, when a blip went all the way up: the engine
/// sits there bouncing instead of being let go straight away.
///
/// Capped at a second and a half on purpose. It is a demonstration of what the
/// car can do, not a demonstration of abusing it, and a limiter held longer
/// than that stops being impressive and becomes uncomfortable.
const LIMITER_HOLD_MS: (u32, u32) = (700, 1500);
/// Odds of hanging on the limiter, **among the blips that reach it** — so
/// roughly one blip in seven overall.
const LIMITER_HOLD_ODDS: f32 = 0.5;

/// Falling back to idle, before the reach-proportional part: coming down from
/// the limiter takes longer than coming down from 3000 rpm.
const RELEASE_BASE_MS: (u32, u32) = (280, 420);
const RELEASE_REACH_MS: (u32, u32) = (220, 420);
const GAP_MS: (u32, u32) = (120, 360);
/// Fraction of the car's own ceiling a normal blip reaches.
const PEAK_FRACTION: (f32, f32) = (0.50, 0.75);
/// …and now and then, right onto the limiter. Slightly past it on purpose: an
/// engine held against its stop bounces around it rather than resting below,
/// and the limiter event only sounds while the needle is actually there.
///
/// **A fraction of the rev ceiling is not the same thing as the limiter**, and
/// conflating the two silenced the limiter entirely for a while. When the
/// ceiling came from the power curve the two were far apart — 8000 against a
/// real 6500 on the GT40 — so 88–97 % of it landed *above* the stop. Once the
/// ceiling became the stop itself, the same 88–97 % landed 130 rpm *below* the
/// trigger, and the sound could never fire.
const REDLINE_FRACTION: (f32, f32) = (1.0, 1.02);
const REDLINE_ODDS: f32 = 0.28;

fn span_ms(range: (u32, u32)) -> Duration {
    Duration::from_millis(fastrand::u32(range.0..=range.1) as u64)
}

fn between(range: (f32, f32)) -> f32 {
    range.0 + fastrand::f32() * (range.1 - range.0)
}

impl Showcase {
    fn new(idle_rev: f32, ceiling: f32, redline: Option<f32>) -> Self {
        Showcase {
            idle_rev,
            ceiling,
            // Without a stated limiter there is no limiter sound either, so the
            // top of the range is as good a place as any to reach for.
            redline: redline.unwrap_or(ceiling * 0.95),
            phase: Phase::Idle,
            elapsed: Duration::ZERO,
            // Start on a short idle rather than blipping the instant it is
            // switched on: the first thing heard should be the engine ticking
            // over.
            span: Duration::from_secs_f32(between((1.5, 3.0))),
            at_limiter: false,
            left: 0,
            peak: idle_rev,
        }
    }

    /// Chooses the top of the next blip, and remembers whether it is one of the
    /// ones that reaches the limiter.
    fn pick_peak(&mut self) {
        self.at_limiter = fastrand::f32() < REDLINE_ODDS;
        self.peak = if self.at_limiter {
            // Onto the stop itself, wherever the car says it is.
            self.redline * between(REDLINE_FRACTION)
        } else {
            self.ceiling * between(PEAK_FRACTION)
        }
        .max(self.idle_rev + 500.0);
    }

    /// How far up its own range this blip is reaching, 0 to 1. Both the rise
    /// and the fall are stretched by it.
    fn reach_fraction(&self) -> f32 {
        ((self.peak - self.idle_rev) / (self.ceiling - self.idle_rev).max(1.0)).clamp(0.0, 1.0)
    }

    fn attack_span(&self) -> Duration {
        let base = fastrand::u32(ATTACK_BASE_MS.0..=ATTACK_BASE_MS.1) as f32;
        let reach = fastrand::u32(ATTACK_REACH_MS.0..=ATTACK_REACH_MS.1) as f32 * self.reach_fraction();
        let lazy = if fastrand::f32() < LAZY_ODDS {
            between(LAZY_FACTOR)
        } else {
            1.0
        };
        Duration::from_millis(((base + reach) * lazy) as u64)
    }

    fn release_span(&self) -> Duration {
        let base = fastrand::u32(RELEASE_BASE_MS.0..=RELEASE_BASE_MS.1) as f32;
        let reach = fastrand::u32(RELEASE_REACH_MS.0..=RELEASE_REACH_MS.1) as f32 * self.reach_fraction();
        Duration::from_millis((base + reach) as u64)
    }

    /// Advances the routine by `dt` and returns the engine speed and throttle
    /// to apply.
    fn tick(&mut self, dt: Duration) -> (f32, f32) {
        self.elapsed += dt;
        // A `while`, not an `if`: a long stall — a debugger, a machine coming
        // back from sleep — must not leave the routine several phases behind.
        while self.elapsed >= self.span {
            self.elapsed -= self.span;
            self.advance();
        }
        let t = (self.elapsed.as_secs_f32() / self.span.as_secs_f32()).clamp(0.0, 1.0);
        let reach = self.peak - self.idle_rev;
        match self.phase {
            Phase::Idle | Phase::Gap => (self.idle_rev, 0.0),
            // Rises quickly then eases into the top, the way an engine under
            // full throttle actually behaves.
            Phase::Attack => (self.idle_rev + reach * t.powf(0.7), 1.0),
            Phase::Hold => (self.peak, 1.0),
            // Falls away fast at first, then hangs near idle.
            Phase::Release => (self.idle_rev + reach * (1.0 - t).powf(1.6), 0.0),
        }
    }

    fn advance(&mut self) {
        match self.phase {
            Phase::Idle => {
                self.left = fastrand::u32(BLIPS_PER_BURST.0..=BLIPS_PER_BURST.1);
                self.pick_peak();
                self.phase = Phase::Attack;
                self.span = self.attack_span();
            }
            Phase::Attack => {
                self.phase = Phase::Hold;
                // Only a blip that actually got to the limiter may sit on it.
                self.span = if self.at_limiter && fastrand::f32() < LIMITER_HOLD_ODDS {
                    span_ms(LIMITER_HOLD_MS)
                } else {
                    span_ms(HOLD_MS)
                };
            }
            Phase::Hold => {
                self.phase = Phase::Release;
                self.span = self.release_span();
            }
            Phase::Release => {
                self.left = self.left.saturating_sub(1);
                if self.left == 0 {
                    self.phase = Phase::Idle;
                    self.span = Duration::from_secs_f32(between(IDLE_SPAN));
                } else {
                    self.phase = Phase::Gap;
                    self.span = span_ms(GAP_MS);
                }
            }
            Phase::Gap => {
                self.pick_peak();
                self.phase = Phase::Attack;
                self.span = self.attack_span();
            }
        }
        // A zero span would spin `tick`'s loop forever.
        self.span = self.span.max(Duration::from_millis(1));
    }
}

/// Throttle held while the slider is not moving.
///
/// Neither 0 nor 1: holding an engine at a speed takes a partly open throttle.
/// At 0 the ear would get engine braking while the reading does not move — the
/// two contradict each other, and the ear wins that argument.
pub const HOLD_THROTTLE: f32 = 0.3;

/// How long without slider movement counts as "put down".
///
/// A mouse drag arrives in bursts rather than continuously: too short a delay
/// and every gap between two pixels would lift off the throttle.
const SETTLE_AFTER: Duration = Duration::from_millis(180);

/// Time constant of the throttle plate, in seconds.
///
/// A real one opens in a few tens of milliseconds; what matters here is mostly
/// not switching abruptly between two layers of the bank, which is heard as a
/// click rather than as a pedal.
const THROTTLE_TAU: f32 = 0.07;

/// Below this change, the value is not sent to FMOD: it has converged, and
/// repeating it fifty times a second changes nothing.
const THROTTLE_EPSILON: f32 = 0.002;

/// The throttle, deduced from **which way the rev slider is moving**.
///
/// The slider states an engine speed, but the gesture says something its
/// position cannot: one climbs to 5000 rpm *on the throttle* and comes back
/// down *off* it. Without this a mod was only ever heard under load — half of
/// what a bank holds, the off-throttle layers of §2.4, stayed inaudible.
///
/// Deliberately clockless: time comes in through `tick`, which makes the rule
/// testable without FMOD running.
#[derive(Debug)]
struct Throttle {
    /// Smoothed value, the one the event is given.
    current: f32,
    /// What it moves towards: 1 while rising, 0 while falling, hold at rest.
    target: f32,
    /// How long the slider has been still.
    still: Duration,
    /// Last value actually handed to FMOD. `None` until something has been,
    /// and after a take-over, which has to force a send.
    sent: Option<f32>,
}

impl Throttle {
    fn new() -> Self {
        // The slider has not moved yet, so the starting state is the resting
        // one — which is also what the play command set before `Start`.
        Throttle {
            current: HOLD_THROTTLE,
            target: HOLD_THROTTLE,
            still: SETTLE_AFTER,
            sent: Some(HOLD_THROTTLE),
        }
    }

    /// The slider just went from `from` to `to`.
    fn slider_moved(&mut self, from: f32, to: f32) {
        self.still = Duration::ZERO;
        // The same value sent again — which happens, the slider emits on every
        // pixel it passes over — is not a movement and says nothing about a
        // direction.
        if to > from {
            self.target = 1.0;
        } else if to < from {
            self.target = 0.0;
        }
    }

    /// Takes back control from the showcase routine, which drove the throttle
    /// itself: whatever the last blip left behind, this is the resting state.
    fn take_over(&mut self) {
        self.current = HOLD_THROTTLE;
        self.target = HOLD_THROTTLE;
        self.still = SETTLE_AFTER;
        // `sent` is forgotten on purpose: the value has to reach FMOD again on
        // the next tick, or the event would stay wherever the last blip left it.
        self.sent = None;
    }

    /// Advances one tick and returns the value to hand over, or `None` when it
    /// has not moved enough to be worth the trip.
    fn tick(&mut self, dt: Duration) -> Option<f32> {
        self.still += dt;
        if self.still >= SETTLE_AFTER {
            self.target = HOLD_THROTTLE;
        }
        // Exponential approach: the step follows `dt`, so a tick stretched by a
        // command arriving in between does not make the value jump.
        let k = 1.0 - (-dt.as_secs_f32() / THROTTLE_TAU).exp();
        self.current += (self.target - self.current) * k;
        self.current = self.current.clamp(0.0, 1.0);
        if self
            .sent
            .is_some_and(|sent| (self.current - sent).abs() <= THROTTLE_EPSILON)
        {
            return None;
        }
        self.sent = Some(self.current);
        Some(self.current)
    }
}

fn run(rx: Receiver<Command>) {
    let mut loaded: Option<Loaded> = None;
    let mut playing: Option<Playing> = None;
    // Kept across plays: moving the camera, switching sound mod and listening
    // again should not silently put the ear back in front of the car.
    let mut listener = Listener::default();

    loop {
        match rx.recv_timeout(TICK) {
            Ok(Command::Play(request, reply)) => {
                stop_current(&loaded, &mut playing);
                let outcome = start(&mut loaded, &mut playing, request, listener);
                if let Err(e) = &outcome {
                    // Not an error the user sees: it is the switch to the
                    // in-house decoder (§4.2). But an install packaged as an
                    // .exe has no console, so without this line a bug report
                    // carries nothing at all.
                    log::warn!("fmod: native audition unavailable, falling back to the decoder: {e}");
                }
                let _ = reply.send(outcome);
            }
            Ok(Command::SetRev(value)) => {
                if let Some(p) = &mut playing {
                    // The direction of the move is the throttle: read it before
                    // `manual_rev` is overwritten, or there is nothing to
                    // compare against (§6quater).
                    p.throttle.slider_moved(p.manual_rev, value);
                    p.manual_rev = value;
                    // Dragging the slider is an explicit takeover: the routine
                    // stops rather than fighting the hand that moved it.
                    if p.showcase.take().is_some() {
                        p.throttle.take_over();
                    }
                }
                set_role(&loaded, &playing, |roles| roles.rev.as_ref(), value);
            }
            Ok(Command::SetListener(next)) => {
                listener = next;
                if let Some(l) = &loaded {
                    if let Err(e) = l.system.set_listener(&listener.attributes()) {
                        log::warn!("fmod: could not move the listener: {e}");
                    }
                }
            }
            Ok(Command::Showcase(on)) => {
                if let Some(p) = &mut playing {
                    p.showcase =
                        on.then(|| Showcase::new(p.manual_rev, p.rev_ceiling, p.limiter.as_ref().map(|l| l.at_rev)));
                    if !on {
                        // Hand the engine back to the slider where it left it,
                        // rather than wherever the last blip happened to stop.
                        // The throttle goes back to its resting value the same
                        // way — the next tick is what actually sends it.
                        p.throttle.take_over();
                        let manual = p.manual_rev;
                        set_role(&loaded, &playing, |r| r.rev.as_ref(), manual);
                    }
                }
            }
            Ok(Command::Stop) => stop_current(&loaded, &mut playing),
            Err(mpsc::RecvTimeoutError::Timeout) => {}
            Err(mpsc::RecvTimeoutError::Disconnected) => break,
        }

        // The showcase advances on the tick, which is why it lives here and not
        // in the interface: a blip is a couple of hundred milliseconds of
        // rising engine speed, sampled every 20 ms.
        if let Some(p) = &mut playing {
            if let Some(showcase) = &mut p.showcase {
                let (rev, throttle) = showcase.tick(TICK);
                if let Some(l) = &loaded {
                    drive_limiter(l, &mut p.limiter, rev);
                }
                if let (Some(l), Some(param)) = (&loaded, p.roles.rev.as_ref()) {
                    let _ = l
                        .system
                        .set_parameter(p.instance, &param.name, rev.clamp(param.min, param.max));
                }
                if let (Some(l), Some(param)) = (&loaded, p.roles.throttle.as_ref()) {
                    let _ = l
                        .system
                        .set_parameter(p.instance, &param.name, throttle.clamp(param.min, param.max));
                }
            // Nobody is showing off: the throttle follows the slider's own
            // direction, which is the only thing that tells accelerating from
            // lifting off at a given engine speed (§6quater).
            } else if let Some(throttle) = p.throttle.tick(TICK) {
                if let (Some(l), Some(param)) = (&loaded, p.roles.throttle.as_ref()) {
                    let _ = l
                        .system
                        .set_parameter(p.instance, &param.name, throttle.clamp(param.min, param.max));
                }
            }
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
    listener: Listener,
) -> Result<PlayReport, String> {
    let began = std::time::Instant::now();

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
        // A room, once, for the life of the system. Best effort on purpose: a
        // dry engine is worth playing, a silent one is not.
        let reverb = match super::guids::master_bus(&request.ac_root) {
            Some(bus) => match system.install_room_reverb(&bus, request.reverb_wet_db) {
                Ok(handle) => Some(handle),
                Err(e) => {
                    log::warn!("fmod: no room reverb, playing dry: {e}");
                    None
                }
            },
            None => {
                log::warn!("fmod: master bus not found in GUIDs.txt, playing dry");
                None
            }
        };
        *loaded = Some(Loaded {
            ac_root: request.ac_root.clone(),
            system,
            bank: None,
            reverb,
        });
    }

    let l = loaded.as_mut().expect("just built");

    // A system built earlier keeps the room it was given; honour the dosage the
    // caller is asking for now.
    if let Some(reverb) = l.reverb {
        if let Err(e) = l.system.set_reverb_wet(reverb, request.reverb_wet_db) {
            log::warn!("fmod: could not set the room level: {e}");
        }
    }

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
    let bank_ready = began.elapsed();

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

    let samples_ready = began.elapsed();

    let instance = l.system.create_instance(desc).map_err(|e| e.to_string())?;

    // The limiter, prepared but silent. Best effort throughout: a car without
    // one, or whose limiter event will not load, simply revs out quietly.
    let limiter = match (request.limiter_guid, request.limiter_rev) {
        (Some(guid), Some(at_rev)) => match l.system.event(&guid) {
            Ok(description) => {
                let _ = l.system.load_sample_data(description);
                let roles = l
                    .system
                    .parameters(description)
                    .map(|found| params::classify(&found))
                    .unwrap_or_default();
                Some(Limiter {
                    description,
                    at_rev,
                    instance: None,
                    roles,
                })
            }
            Err(e) => {
                log::warn!("fmod: no limiter event, revving out silently: {e}");
                None
            }
        },
        _ => None,
    };

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

    // The car sits at the origin; the ear goes where the caller last put it.
    // Both are needed before `start`, or the first mixed block is computed with
    // the event at the default position.
    //
    // **The event faces -Z, the car's tail, and that is deliberate.** The model
    // itself faces +Z — measured, not assumed: in `ford_gt40.kn5` the
    // `SUSP_FRONT_*` nodes sit at z = +1.08 to +1.36 and `SUSP_REAR_*` at
    // z = -1.31, and the converter applies no change of frame at all (see the
    // header of `kn5-gltf/src/geometry.rs`). Pointing the event at +Z along
    // with the geometry therefore *looks* right and sounds backwards: standing
    // at the bonnet gives you the exhaust.
    //
    // So `Event Cone Angle` = 0 is the **tail** in AC's banks, not the nose.
    // Which end of that is responsible — the game passing the car's backward
    // vector, or the sound designers authoring the cone from the exhaust — is
    // not something this side can tell, and it does not change what to do.
    // Only the ear could catch it: the cone readings are symmetric about the
    // axis, so 0-at-the-nose and 0-at-the-tail measure identically.
    let car = Attributes3d {
        position: [0.0; 3],
        velocity: [0.0; 3],
        forward: [0.0, 0.0, -1.0],
        up: [0.0, 1.0, 0.0],
    };
    if let Err(e) = l.system.set_instance_3d(instance, &car) {
        log::warn!("fmod: could not place the event in space: {e}");
    }
    if let Err(e) = l.system.set_listener(&listener.attributes()) {
        log::warn!("fmod: could not place the listener: {e}");
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

    // **A slow play is felt as a frozen key, so it has to leave a trace.**
    // The interesting case is the *second* audition of the same mod, where
    // nothing should need loading at all — and where the breakdown says which
    // of the three phases is actually paying. Warn rather than info: the file
    // log is at Warn, so an info line would be written nowhere on a packaged
    // install, which is precisely where the report comes from.
    let total = began.elapsed();
    if total >= SLOW_PLAY {
        log::warn!(
            "fmod: {} took {:?} to play (bank {:?}, samples {:?}, start {:?})",
            request.event_path,
            total,
            bank_ready,
            samples_ready - bank_ready,
            total - samples_ready,
        );
    }

    let report = PlayReport {
        event_path: request.event_path,
        rev_param: roles.rev.as_ref().map(|p| p.name.clone()),
        rev_min: roles.rev.as_ref().map(|p| p.min),
        rev_max: roles.rev.as_ref().map(|p| p.max),
        throttle_param: roles.throttle.as_ref().map(|p| p.name.clone()),
    };
    *playing = Some(Playing {
        instance,
        roles,
        limiter,
        rev_ceiling: request.rev_ceiling,
        manual_rev: request.rev,
        throttle: Throttle::new(),
        showcase: None,
    });
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

/// Strikes the limiter when the engine reaches its stop, and lets it go when
/// it leaves.
///
/// Started and released rather than left running paused: a limiter is a short
/// looping event, and one lingering across a whole idle would be worse than
/// none at all.
fn drive_limiter(loaded: &Loaded, limiter: &mut Option<Limiter>, rev: f32) {
    let Some(limiter) = limiter else { return };
    let against_the_stop = rev >= limiter.at_rev - LIMITER_MARGIN;

    match (against_the_stop, limiter.instance) {
        (true, None) => match loaded.system.create_instance(limiter.description) {
            Ok(instance) => {
                // Some limiter events are themselves driven by engine speed.
                if let Some(p) = &limiter.roles.rev {
                    let _ = loaded.system.set_parameter(instance, &p.name, rev.clamp(p.min, p.max));
                }
                if let Some(p) = &limiter.roles.throttle {
                    let _ = loaded.system.set_parameter(instance, &p.name, p.max);
                }
                if let Err(e) = loaded.system.start(instance) {
                    log::warn!("fmod: limiter would not start: {e}");
                    let _ = loaded.system.release_instance(instance);
                } else {
                    limiter.instance = Some(instance);
                }
            }
            Err(e) => log::warn!("fmod: could not instance the limiter: {e}"),
        },
        (true, Some(instance)) => {
            if let Some(p) = &limiter.roles.rev {
                let _ = loaded.system.set_parameter(instance, &p.name, rev.clamp(p.min, p.max));
            }
        }
        (false, Some(instance)) => {
            let _ = loaded.system.stop(instance);
            let _ = loaded.system.release_instance(instance);
            limiter.instance = None;
        }
        (false, None) => {}
    }
}

fn stop_current(loaded: &Option<Loaded>, playing: &mut Option<Playing>) {
    let (Some(l), Some(p)) = (loaded, playing.take()) else {
        *playing = None;
        return;
    };
    // Immediate rather than fading out: this is an audition the user just
    // dismissed, and a tail would play over whatever they clicked next.
    if let Some(limiter) = &p.limiter {
        if let Some(instance) = limiter.instance {
            let _ = l.system.stop(instance);
            let _ = l.system.release_instance(instance);
        }
    }
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

    /// Runs the model for `ms` with nobody touching the slider, and returns
    /// where the throttle ended up.
    fn coast(throttle: &mut Throttle, ms: u64) -> f32 {
        let mut last = throttle.current;
        for _ in 0..(ms / 20) {
            if let Some(value) = throttle.tick(TICK) {
                last = value;
            }
        }
        last
    }

    /// The rule of §6quater: pushing the slider up is accelerating.
    #[test]
    fn pushing_the_slider_up_opens_the_throttle() {
        let mut throttle = Throttle::new();
        throttle.slider_moved(1000.0, 1400.0);
        let after = coast(&mut throttle, 100);
        assert!(after > 0.8, "climbing means on the throttle, got {after} after 100 ms");
    }

    /// And the half that was missing before: pulling it back down is lifting
    /// off, which is what makes the off-throttle layers of the bank audible.
    #[test]
    fn pulling_the_slider_down_closes_the_throttle() {
        let mut throttle = Throttle::new();
        throttle.slider_moved(5000.0, 4600.0);
        let after = coast(&mut throttle, 100);
        assert!(after < 0.1, "coming down means off the throttle, got {after}");
    }

    /// A slider that is being dragged arrives in bursts, and the gaps between
    /// two pixels must not be read as letting go.
    #[test]
    fn a_gap_inside_a_drag_does_not_lift_off() {
        let mut throttle = Throttle::new();
        throttle.slider_moved(1000.0, 1200.0);
        coast(&mut throttle, 100);
        throttle.slider_moved(1200.0, 1400.0);
        let during = coast(&mut throttle, 100);
        assert!(during > 0.8, "still accelerating across the gap, got {during}");
    }

    /// Slider put down: the engine holds its speed, so the throttle holds too —
    /// neither wide open nor shut, which would be engine braking at a steady
    /// reading.
    #[test]
    fn a_still_slider_settles_on_the_holding_throttle() {
        let mut throttle = Throttle::new();
        throttle.slider_moved(1000.0, 4000.0);
        coast(&mut throttle, 100);
        let settled = coast(&mut throttle, 1000);
        assert!(
            (settled - HOLD_THROTTLE).abs() < 0.02,
            "settles on the holding value, got {settled}"
        );
    }

    /// The same value sent twice — the slider emits on every pixel it crosses —
    /// says nothing about a direction and must not change the throttle.
    #[test]
    fn a_repeated_value_is_not_a_direction() {
        let mut throttle = Throttle::new();
        throttle.slider_moved(3000.0, 2000.0);
        coast(&mut throttle, 100);
        throttle.slider_moved(2000.0, 2000.0);
        let after = coast(&mut throttle, 60);
        assert!(after < 0.1, "still off the throttle, got {after}");
    }

    /// Leaving the showcase must re-send the throttle even when the model's own
    /// value has not changed: the routine drove the parameter directly, so FMOD
    /// is wherever the last blip left it.
    #[test]
    fn taking_over_from_the_showcase_forces_a_send() {
        let mut throttle = Throttle::new();
        assert_eq!(throttle.tick(TICK), None, "nothing to send while at rest");
        throttle.take_over();
        assert!(
            throttle.tick(TICK).is_some(),
            "the holding value has to reach FMOD again"
        );
    }

    /// Whatever the sequence, the value handed to an event stays a ratio.
    #[test]
    fn the_throttle_stays_between_zero_and_one() {
        let mut throttle = Throttle::new();
        for step in 0..200 {
            let from = 1000.0 + (step % 7) as f32 * 500.0;
            throttle.slider_moved(from, from + if step % 3 == 0 { -800.0 } else { 900.0 });
            let value = coast(&mut throttle, 40);
            assert!((0.0..=1.0).contains(&value), "sane throttle at step {step}: {value}");
        }
    }

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
    /// The same numbers the app derives, so a listening session hears what a
    /// user would rather than a hardcoded 900 rpm on an 8000 rpm ceiling — which
    /// is three times too slow for a Formula 1 and made the demo lie.
    fn car_speeds(ac_root: &std::path::Path, car: &str, bank: &std::path::Path) -> (f32, f32) {
        let car_dir = ac_root.join("content").join("cars").join(car);
        // Same order the app uses: the car's own physics first, the estimates
        // only if they are unreadable. Getting this wrong makes the demo lie
        // about what a user would hear, which it has done twice already.
        let physics = crate::acd::read_engine_data(&car_dir).unwrap_or_default();
        let ceiling = physics
            .limiter_rev
            .unwrap_or_else(|| crate::enginesound::rev_ceiling(&car_dir));
        let idle = physics.idle_rev.unwrap_or_else(|| {
            let parsed = std::fs::read(bank).ok().and_then(|b| crate::fsb5::parse(&b).ok());
            crate::enginesound::idle_rev(parsed.as_ref(), car, ceiling)
        });
        eprintln!(
            "  limiter {ceiling:.0} rpm, idle {idle:.0} rpm  ({})",
            if physics.limiter_rev.is_some() {
                "from data.acd"
            } else {
                "estimated"
            }
        );
        (ceiling, idle)
    }

    /// Lets a listening session compare dosages of room without a rebuild:
    /// `PITBOX_REVERB_WET=-8`. Defaults to what the app ships.
    fn reverb_wet() -> f32 {
        std::env::var("PITBOX_REVERB_WET")
            .ok()
            .and_then(|v| v.parse().ok())
            .unwrap_or(DEFAULT_REVERB_WET_DB)
    }

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
        let bank =
            crate::enginesound::find_bank(&bank_dir).unwrap_or_else(|| panic!("no .bank in {}", bank_dir.display()));
        let (ceiling, idle) = car_speeds(&ac_root, &car, &bank);
        let limiter = super::super::guids::resolve_event(&bank_dir, Some(&ac_root), &car, "limiter");

        let handle = spawn();
        let report = handle
            .play(PlayRequest {
                ac_root,
                bank,
                guid,
                event_path: event_path.clone(),
                rev: idle,
                throttle: HOLD_THROTTLE,
                rev_ceiling: ceiling,
                reverb_wet_db: reverb_wet(),
                limiter_guid: limiter,
                limiter_rev: Some(ceiling),
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
        //
        // The throttle is **not** set here any more: it now follows the
        // direction of these very moves (§6quater). What to listen for is that
        // 6000 → 3000 does not sound like 4000 → 6000 played backwards — the
        // way down should be off-throttle, and audibly quieter.
        for rev in [900.0, 2000.0, 4000.0, 6000.0, 3000.0, 900.0] {
            handle.set_rev(rev);
            std::thread::sleep(Duration::from_millis(400));
        }

        // Orbit the ear around the car. What should be heard is the *timbre*
        // changing between nose and tail, not the sound moving left and right:
        // the listener faces the car throughout, so the source stays centred.
        eprintln!("orbiting the listener");
        for azimuth in (0..360).step_by(15) {
            handle.set_listener(Listener {
                azimuth: azimuth as f32,
                elevation: 8.0,
                distance: 4.0,
            });
            std::thread::sleep(Duration::from_millis(120));
        }

        handle.stop();
        std::thread::sleep(Duration::from_millis(200));
    }

    /// The blip routine, played for real. Ignored for the same reason as the
    /// rest of this module's tests: it needs the game's DLLs and a sound card.
    ///
    /// ```text
    /// PITBOX_AC_ROOT="D:\...ssettocorsa"     ///   cargo test --lib fmod::engine::tests::showcase -- --ignored --nocapture
    /// ```
    #[test]
    #[ignore = "needs a real Assetto Corsa install and an audio device"]
    fn showcase_blips_the_throttle() {
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
        let bank = crate::enginesound::find_bank(&bank_dir).expect("a bank next to the events");
        let (ceiling, idle) = car_speeds(&ac_root, &car, &bank);
        let limiter = super::super::guids::resolve_event(&bank_dir, Some(&ac_root), &car, "limiter");

        let handle = spawn();
        handle
            .play(PlayRequest {
                ac_root,
                bank,
                guid,
                event_path,
                rev: idle,
                throttle: 0.0,
                rev_ceiling: ceiling,
                reverb_wet_db: reverb_wet(),
                limiter_guid: limiter,
                limiter_rev: Some(ceiling),
            })
            .expect("the native path must work against a real install");

        let seconds: u64 = std::env::var("PITBOX_SHOWCASE_SECONDS")
            .ok()
            .and_then(|v| v.parse().ok())
            .unwrap_or(25);
        eprintln!("showcase running for {seconds} s — idle, then bursts of blips");
        handle.set_showcase(true);
        std::thread::sleep(Duration::from_secs(seconds));

        handle.stop();
        std::thread::sleep(Duration::from_millis(200));
    }

    /// The geometry, without any DLL: the ear must end up where the angles say,
    /// and its basis must be orthonormal or FMOD is entitled to refuse it.
    #[test]
    fn listener_geometry_is_orthonormal_and_faces_the_car() {
        for azimuth in [0.0, 35.0, 90.0, 180.0, 275.0] {
            for elevation in [-45.0, 0.0, 8.0, 60.0] {
                let listener = Listener {
                    azimuth,
                    elevation,
                    distance: 4.0,
                };
                let a = listener.attributes();

                let len = |v: [f32; 3]| (v[0] * v[0] + v[1] * v[1] + v[2] * v[2]).sqrt();
                let dot = |u: [f32; 3], v: [f32; 3]| u[0] * v[0] + u[1] * v[1] + u[2] * v[2];

                assert!(
                    (len(a.position) - 4.0).abs() < 1e-3,
                    "the ear sits at the requested distance, got {}",
                    len(a.position)
                );
                assert!((len(a.forward) - 1.0).abs() < 1e-4, "forward is normalised");
                assert!((len(a.up) - 1.0).abs() < 1e-4, "up is normalised");
                assert!(
                    dot(a.forward, a.up).abs() < 1e-4,
                    "forward and up must be perpendicular, dot = {}",
                    dot(a.forward, a.up)
                );
                // Facing the car means forward points from the ear back to the
                // origin — the whole reason the source stays centred.
                let to_car = normalize([-a.position[0], -a.position[1], -a.position[2]]);
                assert!(
                    dot(a.forward, to_car) > 0.999,
                    "the listener must look at the car, dot = {}",
                    dot(a.forward, to_car)
                );
            }
        }
    }

    /// Runs the routine for a simulated stretch and reports, per blip, how long
    /// each phase lasted and where it peaked. No sleeping: the clock is an
    /// argument, which is the whole reason `tick` takes one.
    fn simulate(seconds: u32) -> (Vec<Duration>, Vec<Duration>, f32, f32, bool) {
        let mut showcase = Showcase::new(900.0, 8000.0, Some(6500.0));
        let (mut attacks, mut holds) = (Vec::new(), Vec::new());
        let (mut peak, mut lowest, mut throttle_open) = (0.0_f32, f32::MAX, false);
        let (mut phase, mut phase_for) = (showcase.phase, Duration::ZERO);

        for _ in 0..(seconds as u64 * 1000 / TICK.as_millis() as u64) {
            let (rev, throttle) = showcase.tick(TICK);
            peak = peak.max(rev);
            lowest = lowest.min(rev);
            throttle_open |= throttle > 0.5;

            if showcase.phase == phase {
                phase_for += TICK;
            } else {
                match phase {
                    Phase::Attack => attacks.push(phase_for),
                    Phase::Hold => holds.push(phase_for),
                    _ => {}
                }
                phase = showcase.phase;
                phase_for = Duration::ZERO;
            }
        }
        (attacks, holds, peak, lowest, throttle_open)
    }

    /// A blip has to actually reach for the top of *this* engine, and come back
    /// to idle. Pure logic, no audio: the routine is a state machine.
    #[test]
    fn the_showcase_idles_then_reaches_for_the_top() {
        let (attacks, _, peak, lowest, throttle_open) = simulate(180);
        assert!(throttle_open, "a blip opens the throttle");
        assert!(!attacks.is_empty(), "three minutes must contain several blips");
        assert!(peak > 900.0 * 2.0, "a blip must be audible as one, peaked at {peak}");
        assert!(
            peak <= 8000.0,
            "and must never exceed the car's own ceiling, peaked at {peak}"
        );
        assert!(
            (lowest - 900.0).abs() < 1.0,
            "and it must come back to idle, lowest was {lowest}"
        );
    }

    /// The correction the user asked for: every blip used to be prodded at the
    /// same speed, because the rise was drawn from a flat 150–260 ms. A rise
    /// that also depends on how far it reaches, plus the occasional lazy one,
    /// has to produce a spread wide enough to *hear*.
    #[test]
    fn blips_do_not_all_rise_at_the_same_speed() {
        let (attacks, ..) = simulate(600);
        let shortest = attacks.iter().min().copied().expect("blips happened");
        let longest = attacks.iter().max().copied().expect("blips happened");
        assert!(
            longest.as_secs_f32() / shortest.as_secs_f32() >= 2.0,
            "the slowest rise must be at least twice the quickest, got {shortest:?} to {longest:?}"
        );
    }

    /// The regression this file most needed. A blip aiming at "88 % of the
    /// ceiling" sounds fine and looks fine, and yet the limiter event never
    /// fires — because the trigger sits at the stop itself, and 88 % of it is
    /// below. Reaching *high* is not the same as reaching *the limiter*, and
    /// nothing but this assertion tells the two apart.
    #[test]
    fn a_redline_blip_actually_crosses_the_limiter_threshold() {
        const LIMITER: f32 = 6500.0;
        let mut showcase = Showcase::new(900.0, LIMITER, Some(LIMITER));
        let mut highest = 0.0_f32;
        for _ in 0..(300 * 1000 / TICK.as_millis() as u64) {
            let (rev, _) = showcase.tick(TICK);
            highest = highest.max(rev);
        }
        assert!(
            highest >= LIMITER - LIMITER_MARGIN,
            "the limiter sound is gated on {} rpm and the routine only reached {highest:.0}",
            LIMITER - LIMITER_MARGIN
        );
    }

    /// Without a stated limiter there is no limiter sound, and nothing should
    /// pretend there is one — but the routine must still rev out sensibly.
    #[test]
    fn a_car_with_no_stated_limiter_still_revs_out() {
        let mut showcase = Showcase::new(900.0, 8000.0, None);
        let mut highest = 0.0_f32;
        for _ in 0..(300 * 1000 / TICK.as_millis() as u64) {
            let (rev, _) = showcase.tick(TICK);
            highest = highest.max(rev);
        }
        assert!(highest > 900.0 * 4.0, "still reaches for the top, got {highest:.0}");
        assert!(
            highest <= 8000.0,
            "without ever inventing revs it has not got, got {highest:.0}"
        );
    }

    /// And the other correction: sometimes it reaches the limiter and *stays*
    /// there — but never for long. The upper bound matters as much as the lower
    /// one: a limiter held too long stops being a demonstration.
    #[test]
    fn some_blips_hang_on_the_limiter_but_never_for_long() {
        let (_, holds, ..) = simulate(600);
        let longest = holds.iter().max().copied().expect("blips happened");
        assert!(
            longest >= Duration::from_millis(LIMITER_HOLD_MS.0 as u64),
            "ten minutes must contain at least one blip held on the limiter, longest was {longest:?}"
        );
        // One tick of slack: the phase is sampled every TICK, so the measured
        // length can overshoot the span by that much.
        assert!(
            longest <= Duration::from_millis(LIMITER_HOLD_MS.1 as u64) + TICK,
            "never held longer than the cap, got {longest:?}"
        );
        assert!(
            holds
                .iter()
                .any(|h| *h <= Duration::from_millis(HOLD_MS.1 as u64) + TICK),
            "and most blips are still short flicks, not sustained pulls"
        );
    }

    /// A stalled thread — a debugger, a machine waking from sleep — must not
    /// leave the routine several phases behind, replaying them one tick at a
    /// time long after the fact.
    #[test]
    fn a_long_stall_does_not_leave_the_routine_behind() {
        let mut showcase = Showcase::new(900.0, 8000.0, Some(6500.0));
        let (rev, throttle) = showcase.tick(Duration::from_secs(30));
        assert!(
            rev.is_finite() && (900.0..=8000.0).contains(&rev),
            "still a sane engine speed: {rev}"
        );
        assert!((0.0..=1.0).contains(&throttle), "still a sane throttle: {throttle}");
    }
}
