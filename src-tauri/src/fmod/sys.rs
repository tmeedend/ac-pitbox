//! Hand-written FFI onto the FMOD Studio 1.08 DLLs that ship with the game.
//!
//! No crate does this for us: `libfmod` and `fmod-sys` target FMOD 2.x, whose
//! parameter API is a different shape entirely. Writing the dozen entry points
//! by hand costs less than fighting that, and adds **no dependency**.
//!
//! Nothing here is loaded from anywhere but the user's own Assetto Corsa
//! install (`docs/SPEC-engine-sound-fmod.md` §3): no FMOD binary is
//! redistributed, copied, or looked for on the wider system.
//!
//! Everything this module knows about the ABI that is *not* in FMOD's public
//! headers was measured at lot 0 and written up in §2bis of that spec.

use std::ffi::{CStr, CString};
use std::os::raw::{c_char, c_int, c_uint, c_void};
use std::os::windows::ffi::OsStrExt;
use std::path::Path;

use windows::core::{PCSTR, PCWSTR};
use windows::Win32::Foundation::{FreeLibrary, HMODULE};
use windows::Win32::System::LibraryLoader::{GetProcAddress, LoadLibraryW};

use super::guids::Guid;
use super::params::ParamInfo;

/// The version the DLL is compiled against, checked by `System_Create`.
///
/// 1.08.12, read from the version resource of both DLLs shipped with the game
/// ("1.8.12 (build 80229)"). A wrong value here comes back as
/// `FMOD_ERR_HEADER_MISMATCH` rather than misbehaving, which is why it is safe
/// to pin rather than negotiate.
const FMOD_VERSION: c_uint = 0x0001_0812;

/// `FMOD_STUDIO_INIT_ALLOW_MISSING_PLUGINS`.
///
/// **Mandatory, not a convenience.** Every car bank references the "FMOD
/// Distance Filter" effect, and that plugin exists nowhere in an Assetto Corsa
/// install — not as a DLL, not as a symbol inside either FMOD binary. Without
/// this flag `LoadBankFile` refuses every car bank outright with
/// `FMOD_ERR_PLUGIN_MISSING`. The game therefore runs with it too, so nothing
/// is being degraded here that the game itself preserves.
const INIT_ALLOW_MISSING_PLUGINS: c_uint = 0x0000_0002;

const INIT_NORMAL: c_uint = 0;
const LOAD_BANK_NORMAL: c_uint = 0;

/// `FMOD_STUDIO_LOADING_STATE::LOADED`.
const LOADING_STATE_LOADED: c_int = 2;

/// `FMOD_STUDIO_STOP_MODE`.
#[derive(Clone, Copy, Debug)]
pub enum StopMode {
    /// Let the event's release envelope run.
    AllowFadeout = 0,
    /// Cut now.
    Immediate = 1,
}

/// `FMOD_STUDIO_PLAYBACK_STATE`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PlaybackState {
    Playing,
    Sustaining,
    Stopped,
    Starting,
    Stopping,
    Unknown(c_int),
}

impl From<c_int> for PlaybackState {
    fn from(raw: c_int) -> Self {
        match raw {
            0 => PlaybackState::Playing,
            1 => PlaybackState::Sustaining,
            2 => PlaybackState::Stopped,
            3 => PlaybackState::Starting,
            4 => PlaybackState::Stopping,
            other => PlaybackState::Unknown(other),
        }
    }
}

/// `FMOD_STUDIO_PARAMETER_DESCRIPTION`, **as measured, not as documented**.
///
/// The obvious 1.x layout ends at `maximum` and puts the type enum at offset
/// 20. That is wrong: offset 20 is a further `float` — 0.0 on every parameter
/// seen so far, so presumably `defaultvalue` — and the type sits at **offset
/// 24**. The mistake is invisible if you only look: reading at 20 returns 0 for
/// everything, which decodes as "all parameters are GAME_CONTROLLED" and raises
/// no suspicion at all. It was caught by reading at 24 and finding `Distance` =
/// 1 and `Event Cone Angle` = 2, which are exactly their constants.
#[repr(C)]
#[derive(Clone, Copy)]
struct ParameterDescription {
    name: *const c_char,
    index: c_int,
    minimum: f32,
    maximum: f32,
    default_value: f32,
    kind: c_int,
}

/// Bytes handed to `GetParameterByIndex`, deliberately larger than the struct.
///
/// The struct is 32 bytes padded. Reading into a bigger zeroed buffer costs
/// nothing and means that a DLL whose layout differs from the measured one
/// writes into slack instead of past the end of ours.
const PARAM_DESC_BUFFER: usize = 64;

type FmodResult = c_int;

/// What went wrong, in engineering terms.
///
/// These are diagnostics, not advice, so they stay raw rather than becoming
/// i18n keys — same treatment as the I/O, SQLite and 7-Zip errors elsewhere in
/// the backend. Nothing here reaches the user directly: a failure to reach FMOD
/// is a silent fall back to the in-house decoder (§4.2).
#[derive(Debug, Clone)]
pub enum FmodError {
    /// A DLL could not be loaded — usually because the configured Assetto
    /// Corsa path is wrong or the install is incomplete.
    Library { dll: String, detail: String },
    /// The DLL loaded but does not export something we need.
    Symbol { name: &'static str },
    /// FMOD itself refused.
    Call { call: &'static str, code: FmodResult },
}

impl std::fmt::Display for FmodError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            FmodError::Library { dll, detail } => write!(f, "cannot load {dll}: {detail}"),
            FmodError::Symbol { name } => write!(f, "{name} is not exported by fmodstudio64.dll"),
            FmodError::Call { call, code } => write!(f, "{call} failed: {} ({code})", error_name(*code)),
        }
    }
}

impl std::error::Error for FmodError {}

/// `FMOD_RESULT` names, for diagnostics. Only the ones we can plausibly hit are
/// spelled out; anything else prints its number, which is enough to look up.
fn error_name(code: FmodResult) -> &'static str {
    match code {
        0 => "FMOD_OK",
        13 => "FMOD_ERR_FILE_BAD",
        18 => "FMOD_ERR_FILE_NOTFOUND",
        19 => "FMOD_ERR_FORMAT",
        20 => "FMOD_ERR_HEADER_MISMATCH",
        26 => "FMOD_ERR_INITIALIZATION",
        30 => "FMOD_ERR_INVALID_HANDLE",
        31 => "FMOD_ERR_INVALID_PARAM",
        38 => "FMOD_ERR_MEMORY",
        51 => "FMOD_ERR_OUTPUT_INIT",
        52 => "FMOD_ERR_OUTPUT_NODRIVERS",
        54 => "FMOD_ERR_PLUGIN_MISSING",
        70 => "FMOD_ERR_EVENT_ALREADY_LOADED",
        74 => "FMOD_ERR_EVENT_NOTFOUND",
        _ => "FMOD_ERR",
    }
}

fn check(call: &'static str, code: FmodResult) -> Result<(), FmodError> {
    if code == 0 {
        Ok(())
    } else {
        Err(FmodError::Call { call, code })
    }
}

// The twelve entry points of §2.2, plus the three that lot 0 proved necessary:
// sample loading is not implicit, and playback state is how we know an instance
// is alive.
type FnSystemCreate = unsafe extern "C" fn(*mut *mut c_void, c_uint) -> FmodResult;
type FnSystemInitialize = unsafe extern "C" fn(*mut c_void, c_int, c_uint, c_uint, *mut c_void) -> FmodResult;
type FnSystemRelease = unsafe extern "C" fn(*mut c_void) -> FmodResult;
type FnSystemUpdate = unsafe extern "C" fn(*mut c_void) -> FmodResult;
type FnSystemLoadBankFile = unsafe extern "C" fn(*mut c_void, *const c_char, c_uint, *mut *mut c_void) -> FmodResult;
type FnSystemGetEventByID = unsafe extern "C" fn(*mut c_void, *const Guid, *mut *mut c_void) -> FmodResult;
type FnDescGetParameterCount = unsafe extern "C" fn(*mut c_void, *mut c_int) -> FmodResult;
type FnDescGetParameterByIndex = unsafe extern "C" fn(*mut c_void, c_int, *mut u8) -> FmodResult;
type FnDescCreateInstance = unsafe extern "C" fn(*mut c_void, *mut *mut c_void) -> FmodResult;
type FnDescLoadSampleData = unsafe extern "C" fn(*mut c_void) -> FmodResult;
type FnDescGetSampleLoadingState = unsafe extern "C" fn(*mut c_void, *mut c_int) -> FmodResult;
type FnInstanceSetParameterValue = unsafe extern "C" fn(*mut c_void, *const c_char, f32) -> FmodResult;
type FnInstanceStart = unsafe extern "C" fn(*mut c_void) -> FmodResult;
type FnInstanceStop = unsafe extern "C" fn(*mut c_void, c_int) -> FmodResult;
type FnInstanceRelease = unsafe extern "C" fn(*mut c_void) -> FmodResult;
type FnInstanceGetPlaybackState = unsafe extern "C" fn(*mut c_void, *mut c_int) -> FmodResult;

struct Api {
    system_create: FnSystemCreate,
    system_initialize: FnSystemInitialize,
    system_release: FnSystemRelease,
    system_update: FnSystemUpdate,
    system_load_bank_file: FnSystemLoadBankFile,
    system_get_event_by_id: FnSystemGetEventByID,
    desc_get_parameter_count: FnDescGetParameterCount,
    desc_get_parameter_by_index: FnDescGetParameterByIndex,
    desc_create_instance: FnDescCreateInstance,
    desc_load_sample_data: FnDescLoadSampleData,
    desc_get_sample_loading_state: FnDescGetSampleLoadingState,
    instance_set_parameter_value: FnInstanceSetParameterValue,
    instance_start: FnInstanceStart,
    instance_stop: FnInstanceStop,
    instance_release: FnInstanceRelease,
    instance_get_playback_state: FnInstanceGetPlaybackState,
}

/// Resolves one export and reinterprets it as a function pointer.
///
/// # Safety
/// `F` must be the exact `extern "C"` signature the DLL exports under `name`.
/// Those signatures are FMOD 1.08's, and the ones that could not be taken on
/// faith were verified at lot 0.
unsafe fn symbol<F: Copy>(module: HMODULE, name: &'static str) -> Result<F, FmodError> {
    let c_name = CString::new(name).map_err(|_| FmodError::Symbol { name })?;
    // `GetProcAddress` is one of the few Win32 entry points that is ANSI-only:
    // exported symbol names are bytes, never UTF-16.
    match GetProcAddress(module, PCSTR(c_name.as_ptr() as *const u8)) {
        Some(ptr) => Ok(std::mem::transmute_copy(&ptr)),
        None => Err(FmodError::Symbol { name }),
    }
}

/// The two loaded DLLs and every entry point resolved out of them.
///
/// Held for the life of the process on purpose: `FreeLibrary` is never called
/// while anything FMOD owns might still be alive, and unloading an audio engine
/// mid-flight buys nothing.
pub struct Fmod {
    studio: HMODULE,
    low: HMODULE,
    api: Api,
}

impl Fmod {
    /// Loads `fmod64.dll` then `fmodstudio64.dll` from the game folder.
    ///
    /// Order matters and so does the full path: the Studio DLL imports the low
    /// level one, and resolving it ourselves first keeps the system search path
    /// out of the picture entirely (§4.2). Nothing is added to `PATH`.
    pub fn load(ac_root: &Path) -> Result<Self, FmodError> {
        // SAFETY: both paths point inside the configured game install, and the
        // signatures below are FMOD 1.08's.
        unsafe {
            let low = load_dll(ac_root, "fmod64.dll")?;
            let studio = load_dll(ac_root, "fmodstudio64.dll")?;

            let api = Api {
                system_create: symbol(studio, "FMOD_Studio_System_Create")?,
                system_initialize: symbol(studio, "FMOD_Studio_System_Initialize")?,
                system_release: symbol(studio, "FMOD_Studio_System_Release")?,
                system_update: symbol(studio, "FMOD_Studio_System_Update")?,
                system_load_bank_file: symbol(studio, "FMOD_Studio_System_LoadBankFile")?,
                system_get_event_by_id: symbol(studio, "FMOD_Studio_System_GetEventByID")?,
                desc_get_parameter_count: symbol(studio, "FMOD_Studio_EventDescription_GetParameterCount")?,
                desc_get_parameter_by_index: symbol(studio, "FMOD_Studio_EventDescription_GetParameterByIndex")?,
                desc_create_instance: symbol(studio, "FMOD_Studio_EventDescription_CreateInstance")?,
                desc_load_sample_data: symbol(studio, "FMOD_Studio_EventDescription_LoadSampleData")?,
                desc_get_sample_loading_state: symbol(studio, "FMOD_Studio_EventDescription_GetSampleLoadingState")?,
                instance_set_parameter_value: symbol(studio, "FMOD_Studio_EventInstance_SetParameterValue")?,
                instance_start: symbol(studio, "FMOD_Studio_EventInstance_Start")?,
                instance_stop: symbol(studio, "FMOD_Studio_EventInstance_Stop")?,
                instance_release: symbol(studio, "FMOD_Studio_EventInstance_Release")?,
                instance_get_playback_state: symbol(studio, "FMOD_Studio_EventInstance_GetPlaybackState")?,
            };

            Ok(Fmod { studio, low, api })
        }
    }
}

impl Drop for Fmod {
    fn drop(&mut self) {
        // Best effort, and last: a system built on these must already be gone.
        unsafe {
            let _ = FreeLibrary(self.studio);
            let _ = FreeLibrary(self.low);
        }
    }
}

unsafe fn load_dll(ac_root: &Path, name: &str) -> Result<HMODULE, FmodError> {
    let path = ac_root.join(name);
    let wide: Vec<u16> = path.as_os_str().encode_wide().chain(std::iter::once(0)).collect();
    LoadLibraryW(PCWSTR(wide.as_ptr())).map_err(|e| FmodError::Library {
        dll: path.display().to_string(),
        detail: e.message(),
    })
}

/// An event description. Owned by the bank, so there is nothing to release.
#[derive(Clone, Copy)]
pub struct EventDesc(*mut c_void);

/// A playing (or about to play) instance of an event.
#[derive(Clone, Copy)]
pub struct EventInstance(*mut c_void);

/// A live FMOD Studio system.
///
/// Deliberately **not** `Send`: it holds raw pointers, and §4.3 requires that
/// one thread own the system and be the only one to touch it. The type system
/// enforcing that is a feature, not an obstacle to work around.
pub struct System<'a> {
    fmod: &'a Fmod,
    raw: *mut c_void,
}

impl<'a> System<'a> {
    /// Creates and initialises the system.
    pub fn new(fmod: &'a Fmod) -> Result<Self, FmodError> {
        let mut raw: *mut c_void = std::ptr::null_mut();
        unsafe {
            check(
                "FMOD_Studio_System_Create",
                (fmod.api.system_create)(&mut raw, FMOD_VERSION),
            )?;
            let init =
                (fmod.api.system_initialize)(raw, 256, INIT_ALLOW_MISSING_PLUGINS, INIT_NORMAL, std::ptr::null_mut());
            if let Err(e) = check("FMOD_Studio_System_Initialize", init) {
                let _ = (fmod.api.system_release)(raw);
                return Err(e);
            }
        }
        Ok(System { fmod, raw })
    }

    /// Loads a bank file. The path travels as UTF-8, which lot 0 confirmed is
    /// what 1.08 expects — a bank under a path with accents and spaces opens
    /// without ceremony.
    pub fn load_bank(&self, path: &Path) -> Result<(), FmodError> {
        let c_path = CString::new(path.to_string_lossy().as_bytes()).map_err(|_| FmodError::Call {
            call: "FMOD_Studio_System_LoadBankFile",
            code: 31, // FMOD_ERR_INVALID_PARAM: an interior NUL never reaches FMOD
        })?;
        let mut bank: *mut c_void = std::ptr::null_mut();
        unsafe {
            check(
                "FMOD_Studio_System_LoadBankFile",
                (self.fmod.api.system_load_bank_file)(self.raw, c_path.as_ptr(), LOAD_BANK_NORMAL, &mut bank),
            )
        }
    }

    /// Must be called regularly: FMOD does its mixing bookkeeping and frees
    /// stopped instances here (§4.3).
    pub fn update(&self) -> Result<(), FmodError> {
        unsafe { check("FMOD_Studio_System_Update", (self.fmod.api.system_update)(self.raw)) }
    }

    pub fn event(&self, guid: &Guid) -> Result<EventDesc, FmodError> {
        let mut desc: *mut c_void = std::ptr::null_mut();
        unsafe {
            check(
                "FMOD_Studio_System_GetEventByID",
                (self.fmod.api.system_get_event_by_id)(self.raw, guid, &mut desc),
            )?;
        }
        Ok(EventDesc(desc))
    }

    /// Every parameter the event exposes, in declaration order.
    ///
    /// Sorting them into roles is `params::classify`'s job, not this one's.
    pub fn parameters(&self, desc: EventDesc) -> Result<Vec<ParamInfo>, FmodError> {
        let mut count: c_int = 0;
        unsafe {
            check(
                "FMOD_Studio_EventDescription_GetParameterCount",
                (self.fmod.api.desc_get_parameter_count)(desc.0, &mut count),
            )?;
        }

        let mut out = Vec::with_capacity(count.max(0) as usize);
        for index in 0..count {
            let mut buffer = [0u8; PARAM_DESC_BUFFER];
            unsafe {
                check(
                    "FMOD_Studio_EventDescription_GetParameterByIndex",
                    (self.fmod.api.desc_get_parameter_by_index)(desc.0, index, buffer.as_mut_ptr()),
                )?;
                // `read_unaligned` rather than a cast-and-deref: the buffer is a
                // byte array, and nothing guarantees it is aligned for a struct
                // holding a pointer.
                let raw = std::ptr::read_unaligned(buffer.as_ptr() as *const ParameterDescription);
                let name = if raw.name.is_null() {
                    String::new()
                } else {
                    CStr::from_ptr(raw.name).to_string_lossy().into_owned()
                };
                let _ = raw.default_value; // read for layout's sake; always 0.0 so far
                out.push(ParamInfo {
                    name,
                    index: raw.index,
                    min: raw.minimum,
                    max: raw.maximum,
                    kind: raw.kind,
                });
            }
        }
        Ok(out)
    }

    /// Asks for the event's samples and reports whether they are ready.
    ///
    /// Not implicit in `LoadBankFile`: without this, the first few hundred
    /// milliseconds of playback are silent while FMOD streams them in.
    pub fn load_sample_data(&self, desc: EventDesc) -> Result<(), FmodError> {
        unsafe {
            check(
                "FMOD_Studio_EventDescription_LoadSampleData",
                (self.fmod.api.desc_load_sample_data)(desc.0),
            )
        }
    }

    pub fn samples_loaded(&self, desc: EventDesc) -> Result<bool, FmodError> {
        let mut state: c_int = 0;
        unsafe {
            check(
                "FMOD_Studio_EventDescription_GetSampleLoadingState",
                (self.fmod.api.desc_get_sample_loading_state)(desc.0, &mut state),
            )?;
        }
        Ok(state == LOADING_STATE_LOADED)
    }

    pub fn create_instance(&self, desc: EventDesc) -> Result<EventInstance, FmodError> {
        let mut inst: *mut c_void = std::ptr::null_mut();
        unsafe {
            check(
                "FMOD_Studio_EventDescription_CreateInstance",
                (self.fmod.api.desc_create_instance)(desc.0, &mut inst),
            )?;
        }
        Ok(EventInstance(inst))
    }

    /// Sets a parameter by name.
    ///
    /// Valid on an instance that is **already playing**, which is what the rev
    /// slider of §4.4 relies on — and a different thing from setting it before
    /// `start`, which is all a fixed-value run proves.
    pub fn set_parameter(&self, inst: EventInstance, name: &str, value: f32) -> Result<(), FmodError> {
        let c_name = CString::new(name).map_err(|_| FmodError::Call {
            call: "FMOD_Studio_EventInstance_SetParameterValue",
            code: 31,
        })?;
        unsafe {
            check(
                "FMOD_Studio_EventInstance_SetParameterValue",
                (self.fmod.api.instance_set_parameter_value)(inst.0, c_name.as_ptr(), value),
            )
        }
    }

    pub fn start(&self, inst: EventInstance) -> Result<(), FmodError> {
        unsafe {
            check(
                "FMOD_Studio_EventInstance_Start",
                (self.fmod.api.instance_start)(inst.0),
            )
        }
    }

    pub fn stop(&self, inst: EventInstance, mode: StopMode) -> Result<(), FmodError> {
        unsafe {
            check(
                "FMOD_Studio_EventInstance_Stop",
                (self.fmod.api.instance_stop)(inst.0, mode as c_int),
            )
        }
    }

    pub fn release_instance(&self, inst: EventInstance) -> Result<(), FmodError> {
        unsafe {
            check(
                "FMOD_Studio_EventInstance_Release",
                (self.fmod.api.instance_release)(inst.0),
            )
        }
    }

    pub fn playback_state(&self, inst: EventInstance) -> Result<PlaybackState, FmodError> {
        let mut state: c_int = 0;
        unsafe {
            check(
                "FMOD_Studio_EventInstance_GetPlaybackState",
                (self.fmod.api.instance_get_playback_state)(inst.0, &mut state),
            )?;
        }
        Ok(PlaybackState::from(state))
    }
}

impl Drop for System<'_> {
    fn drop(&mut self) {
        // Releasing the system takes every bank and instance with it, so
        // nothing else needs unwinding here.
        unsafe {
            let _ = (self.fmod.api.system_release)(self.raw);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The layout correction of §2bis, frozen. If this ever fails, the struct
    /// has been "tidied" back to the plausible-but-wrong 24-byte version, and
    /// every parameter would silently read as GAME_CONTROLLED again.
    #[test]
    fn parameter_description_keeps_its_measured_layout() {
        assert_eq!(
            std::mem::size_of::<ParameterDescription>(),
            32,
            "28 bytes of fields, padded to 32"
        );

        let base = std::ptr::null::<ParameterDescription>();
        // SAFETY: offset arithmetic on a null base, never dereferenced.
        let offset = |field: *const u8| unsafe { field.offset_from(base as *const u8) };
        unsafe {
            assert_eq!(offset(std::ptr::addr_of!((*base).index) as *const u8), 8, "index at 8");
            assert_eq!(
                offset(std::ptr::addr_of!((*base).minimum) as *const u8),
                12,
                "minimum at 12"
            );
            assert_eq!(
                offset(std::ptr::addr_of!((*base).maximum) as *const u8),
                16,
                "maximum at 16"
            );
            assert_eq!(
                offset(std::ptr::addr_of!((*base).default_value) as *const u8),
                20,
                "the extra float at 20"
            );
            assert_eq!(
                offset(std::ptr::addr_of!((*base).kind) as *const u8),
                24,
                "type at 24, not 20"
            );
        }
    }

    /// A `Guid` is passed straight to `GetEventByID`, so its layout is FMOD's,
    /// not ours to rearrange.
    #[test]
    fn guid_is_sixteen_bytes() {
        assert_eq!(
            std::mem::size_of::<Guid>(),
            16,
            "FMOD_GUID is four fields totalling 16 bytes"
        );
    }

    #[test]
    fn playback_state_decodes_the_documented_values() {
        assert_eq!(PlaybackState::from(0), PlaybackState::Playing);
        assert_eq!(PlaybackState::from(2), PlaybackState::Stopped);
        assert_eq!(PlaybackState::from(3), PlaybackState::Starting);
        assert_eq!(
            PlaybackState::from(99),
            PlaybackState::Unknown(99),
            "an unknown state is kept, not guessed"
        );
    }

    /// Errors are diagnostics: they must name the call and the code, because
    /// that pair is all a bug report will carry.
    #[test]
    fn errors_name_the_failing_call() {
        let e = FmodError::Call {
            call: "FMOD_Studio_System_LoadBankFile",
            code: 54,
        };
        let text = e.to_string();
        assert!(
            text.contains("FMOD_Studio_System_LoadBankFile"),
            "names the call: {text}"
        );
        assert!(text.contains("FMOD_ERR_PLUGIN_MISSING"), "names the error: {text}");
        assert!(text.contains("54"), "keeps the raw code: {text}");
    }
}
