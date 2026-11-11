//! Grid thumbnails of protected cars, rendered by the game itself (GRILLE§7.1).
//!
//! A protected KN5 only decodes inside Assetto Corsa once CSP is loaded: the
//! game is the only renderer that can photograph those cars. A session here is
//! one car: the bundled CSP Lua app (`resources/csp/pitbox_shot`) is linked
//! into `apps/lua` by a junction, a job file tells it which images to render,
//! a practice session starts through Content Manager, the app renders off
//! screen, writes its PNGs and closes the game, and the images move into the
//! grid thumbnail store under the names the Pit Box renderer would have used.
//! What was learnt building it is in `docs/csp-shot-research.md`.
//!
//! Three properties hold this module together:
//!
//! **Nothing of the game is written.** The app is a junction in `apps/lua`,
//! removed after the session and by a sweep at start-up (`sweep`) if Pit Box
//! was closed in between; a real folder of that name is never touched. The job
//! and the images live in `Documents\Assetto Corsa\pitbox_shot`, the folder a
//! Lua app may write to.
//!
//! **The session is driven like any other**: Content Manager owns `race.ini`
//! (golden rule 4), the skin goes through the same patch as a normal launch.
//!
//! **The end is read from the game, not guessed**: the app writes `done.json`
//! as it goes and quits the game; the wait ends when the game is gone, or on a
//! deadline when it never starts or never ends.

use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

use serde::{Deserialize, Serialize};

/// Folder name of the app in `apps/lua`, and of its working folder in
/// `Documents\Assetto Corsa`. The Lua script reads its job from the latter.
pub const APP_ID: &str = "pitbox_shot";

/// The folder of Pit Box's data the app's files are written to, as
/// `<data>/gameshot/pitbox_shot`: what tells our junction from another.
pub const SOURCE_PARENT: &str = "gameshot";

const MANIFEST: &str = include_str!("../resources/csp/pitbox_shot/manifest.ini");
const SCRIPT: &str = include_str!("../resources/csp/pitbox_shot/pitbox_shot.lua");

/// The track every shot is taken on: base content, so present in every
/// install. The shot draws the car alone, so the track never shows.
pub const STUDIO_TRACK: &str = "magione";

/// Thumbnail size and supersampling: those of the Pit Box renderer
/// (`gridThumbs.svelte.ts`, `WIDTH`/`HEIGHT`/`SUPERSAMPLE`).
const WIDTH: u32 = 1024;
const HEIGHT: u32 = 576;
const SUPERSAMPLE: u32 = 2;
/// Seconds the app waits after the session has loaded, for textures and
/// LOD A. Eight was enough on every car tried.
const WARMUP: u32 = 8;

/// How long the game may take to appear after Content Manager was asked.
/// CM's own preparation (downloads, checks) happens before it.
const START_DEADLINE: Duration = Duration::from_secs(120);
/// How long a whole session may last. A shot takes about ten seconds once the
/// session has loaded; past this, the game is stuck and the shot failed.
const SESSION_DEADLINE: Duration = Duration::from_secs(300);

/// One car to photograph, and the thumbnails wanted of it — one per grid
/// density whose preset the game can render (GRILLE§7.1).
#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ShotRequest {
    pub car_id: String,
    /// The skin the grid shows; `None` = the first one, as the Pit Box
    /// renderer picks it (`kn5_gltf::resolve_skin`).
    pub skin: Option<String>,
    pub outputs: Vec<ShotOutput>,
}

/// One thumbnail: its entry name in the store, and the template it renders,
/// passed to the Lua app untouched — its field names are the frontend's.
#[derive(Debug, Clone, Deserialize)]
pub struct ShotOutput {
    pub stem: String,
    pub template: serde_json::Value,
}

/// What became of one requested thumbnail.
#[derive(Debug, Clone, Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct ShotResult {
    pub stem: String,
    /// The stored PNG, or `None` when the game did not produce it.
    pub path: Option<String>,
}

/// Where a session is, for the task panel.
#[derive(Debug, Clone, Copy, Serialize, PartialEq)]
#[serde(rename_all = "lowercase")]
pub enum Phase {
    /// Content Manager asked, the game not there yet.
    Launching,
    /// The game runs: loading, then the shot.
    InGame,
}

/// The folders a session works with.
pub struct Paths {
    /// The game install.
    pub ac: PathBuf,
    /// `Documents\Assetto Corsa`.
    pub ac_documents: PathBuf,
    /// Where the app's files are written before being linked, under Pit Box's
    /// own data folder.
    pub app_source: PathBuf,
    /// The grid thumbnail store.
    pub thumbs: PathBuf,
}

impl Paths {
    fn link(&self) -> PathBuf {
        app_link(&self.ac)
    }
    fn work(&self) -> PathBuf {
        self.ac_documents.join(APP_ID)
    }
    fn out(&self) -> PathBuf {
        self.work().join("out")
    }
}

fn app_link(ac: &Path) -> PathBuf {
    ac.join("apps").join("lua").join(APP_ID)
}

/// `Documents\Assetto Corsa`, where the game keeps its user files.
pub fn ac_documents() -> Option<PathBuf> {
    Some(dirs::document_dir()?.join("Assetto Corsa"))
}

/// Whether `link` is a junction to the app as Pit Box writes it — a folder
/// `gameshot/pitbox_shot`, wherever Pit Box's data lives. A junction elsewhere
/// is a user app that happens to share the id, deployed by the Apps screen.
fn is_ours(link: &Path) -> bool {
    crate::activation::is_junction(link)
        && std::fs::read_link(link).is_ok_and(|target| target.ends_with(Path::new(SOURCE_PARENT).join(APP_ID)))
}

/// Writes the app's files and links them into `apps/lua`.
///
/// The files are rewritten every time, so the game always runs the app of
/// this version of Pit Box. A junction of ours already there, from an
/// interrupted session, is replaced; anything else of that name — a real
/// folder, or a user app of the same id linked by the Apps screen — is
/// refused, never touched.
pub fn install_app(paths: &Paths) -> Result<(), String> {
    std::fs::create_dir_all(&paths.app_source).map_err(|e| format!("{} : {e}", paths.app_source.display()))?;
    let script = format!("{APP_ID}.lua");
    for (name, text) in [("manifest.ini", MANIFEST), (script.as_str(), SCRIPT)] {
        let file = paths.app_source.join(name);
        std::fs::write(&file, text).map_err(|e| format!("{} : {e}", file.display()))?;
    }
    let link = paths.link();
    if is_ours(&link) {
        crate::activation::remove_junction(&link)?;
    } else if link.exists() || crate::activation::is_junction(&link) {
        return Err(crate::errors::GAMESHOT_APP_FOLDER_TAKEN.into());
    }
    if let Some(parent) = link.parent() {
        std::fs::create_dir_all(parent).map_err(|e| format!("{} : {e}", parent.display()))?;
    }
    crate::activation::create_junction(&link, &paths.app_source)
}

/// Removes the app's junction and the job, so the app stays idle if the game
/// is started by hand afterwards. Best-effort and logged: it runs on every
/// exit path of a session, and again at start-up.
pub fn uninstall_app(ac: &Path, ac_documents: &Path) {
    let link = app_link(ac);
    if is_ours(&link) {
        if let Err(e) = crate::activation::remove_junction(&link) {
            log::warn!("gameshot: junction {} left in place ({e})", link.display());
        }
    }
    let job = ac_documents.join(APP_ID).join("job.json");
    if let Err(e) = std::fs::remove_file(&job) {
        if e.kind() != std::io::ErrorKind::NotFound {
            log::warn!("gameshot: job {} left in place ({e})", job.display());
        }
    }
}

/// Start-up safety net: a session cut short by Pit Box closing leaves the
/// app linked. Removed here, so the next game started by hand does not carry
/// it. Only our junction goes — see `is_ours`.
pub fn sweep(ac: &Path) {
    if let Some(docs) = ac_documents() {
        uninstall_app(ac, &docs);
    }
}

/// Writes the job the Lua app reads, after clearing what a previous session
/// left in the working folder.
pub fn write_job(paths: &Paths, request: &ShotRequest) -> Result<(), String> {
    for output in &request.outputs {
        if !crate::gridthumbs::is_entry_stem(&output.stem) {
            return Err(format!("nom de vignette refusé : {}", output.stem));
        }
    }
    let work = paths.work();
    let out = paths.out();
    if out.exists() {
        std::fs::remove_dir_all(&out).map_err(|e| format!("{} : {e}", out.display()))?;
    }
    std::fs::create_dir_all(&out).map_err(|e| format!("{} : {e}", out.display()))?;
    if let Err(e) = std::fs::remove_file(work.join("done.json")) {
        if e.kind() != std::io::ErrorKind::NotFound {
            log::warn!("gameshot: previous done.json left in place ({e})");
        }
    }

    let outputs: Vec<serde_json::Value> = request
        .outputs
        .iter()
        .map(|o| {
            serde_json::json!({
                "file": out.join(format!("{}.png", o.stem)).to_string_lossy(),
                "template": o.template,
            })
        })
        .collect();
    let job = serde_json::json!({
        "cars": [{ "id": request.car_id, "outputs": outputs }],
        "width": WIDTH,
        "height": HEIGHT,
        "supersample": SUPERSAMPLE,
        "warmup": WARMUP,
        "quit": true,
    });
    let file = work.join("job.json");
    std::fs::write(&file, job.to_string()).map_err(|e| format!("{} : {e}", file.display()))
}

/// How a session ended.
#[derive(Debug, PartialEq)]
pub enum Ending {
    /// The game came and went.
    Closed,
    /// The game never appeared.
    NeverStarted,
    /// The game was still running at the deadline.
    TimedOut,
}

/// Waits for the session to end. `game_running` is asked twice a second.
pub fn wait(
    game_running: &dyn Fn() -> bool,
    progress: &dyn Fn(Phase),
    start_deadline: Duration,
    session_deadline: Duration,
    poll: Duration,
) -> Ending {
    let begin = Instant::now();
    let mut started = false;
    loop {
        let running = game_running();
        if running && !started {
            started = true;
            progress(Phase::InGame);
        }
        if started && !running {
            return Ending::Closed;
        }
        if !started && begin.elapsed() > start_deadline {
            return Ending::NeverStarted;
        }
        if begin.elapsed() > session_deadline {
            return Ending::TimedOut;
        }
        std::thread::sleep(poll);
    }
}

/// Moves the images the game produced into the store. A thumbnail the game
/// did not write comes back without a path: the card keeps its photo.
pub fn collect(paths: &Paths, request: &ShotRequest) -> Vec<ShotResult> {
    request
        .outputs
        .iter()
        .map(|o| {
            let file = paths.out().join(format!("{}.png", o.stem));
            let path = match std::fs::read(&file) {
                Ok(png) => match crate::gridthumbs::write_in(&paths.thumbs, &o.stem, &png) {
                    Ok(stored) => Some(stored.to_string_lossy().into_owned()),
                    Err(e) => {
                        log::warn!("gameshot: {} not stored ({e})", o.stem);
                        None
                    }
                },
                Err(_) => None,
            };
            ShotResult {
                stem: o.stem.clone(),
                path,
            }
        })
        .collect()
}

/// One whole session: link the app, write the job, start the game, wait,
/// store the images, unlink — the unlinking whatever happened.
///
/// `launch` starts the session (Content Manager, in the app); `game_running`
/// says whether the game runs. Both are passed in so tests can stand in for
/// the game.
pub fn run(
    paths: &Paths,
    request: &ShotRequest,
    launch: &dyn Fn() -> Result<(), String>,
    game_running: &dyn Fn() -> bool,
    progress: &dyn Fn(Phase),
) -> Result<Vec<ShotResult>, String> {
    run_with(
        paths,
        request,
        launch,
        game_running,
        progress,
        (START_DEADLINE, SESSION_DEADLINE, Duration::from_millis(500)),
    )
}

fn run_with(
    paths: &Paths,
    request: &ShotRequest,
    launch: &dyn Fn() -> Result<(), String>,
    game_running: &dyn Fn() -> bool,
    progress: &dyn Fn(Phase),
    (start, session, poll): (Duration, Duration, Duration),
) -> Result<Vec<ShotResult>, String> {
    if game_running() {
        return Err(crate::errors::GAME_RUNNING.into());
    }
    let outcome = (|| -> Result<Ending, String> {
        install_app(paths)?;
        write_job(paths, request)?;
        progress(Phase::Launching);
        launch()?;
        Ok(wait(game_running, progress, start, session, poll))
    })();
    uninstall_app(&paths.ac, &paths.ac_documents);
    match outcome? {
        Ending::Closed => {
            let results = collect(paths, request);
            if results.iter().all(|r| r.path.is_none()) {
                // What the app wrote says why, for a bug report: a step error,
                // a car missing from the session, or no file at all when the
                // app never ran.
                let done = std::fs::read_to_string(paths.work().join("done.json"));
                log::warn!("gameshot: no image for {} — done.json: {done:?}", request.car_id);
                return Err(crate::errors::GAMESHOT_NO_IMAGE.into());
            }
            Ok(results)
        }
        Ending::NeverStarted => Err(crate::errors::GAMESHOT_NEVER_STARTED.into()),
        Ending::TimedOut => {
            log::warn!("gameshot: {} still running after the deadline", request.car_id);
            Err(crate::errors::GAMESHOT_TIMED_OUT.into())
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::cell::Cell;

    const STEM: &str = "v49-0123456789abcdef0123456789abcdef-t0a1b2c3d";

    fn paths(base: &Path) -> Paths {
        let p = Paths {
            ac: base.join("ac"),
            ac_documents: base.join("docs"),
            app_source: base.join("appdata").join(SOURCE_PARENT).join(APP_ID),
            thumbs: base.join("thumbs"),
        };
        std::fs::create_dir_all(p.ac.join("apps").join("lua")).unwrap();
        std::fs::create_dir_all(&p.ac_documents).unwrap();
        std::fs::create_dir_all(&p.thumbs).unwrap();
        p
    }

    fn request() -> ShotRequest {
        ShotRequest {
            car_id: "some_car".into(),
            skin: None,
            outputs: vec![ShotOutput {
                stem: STEM.into(),
                template: serde_json::json!({ "azimuth": 320 }),
            }],
        }
    }

    // Rule (GRILLE§7.1): a session leaves nothing in the game — the app is
    // linked for the session and unlinked after it, its files never copied in.
    #[test]
    fn the_app_is_a_junction_for_the_session_and_gone_after() {
        let base = crate::testutil::temp_dir("gameshot-link");
        let p = paths(&base);
        let linked = Cell::new(false);
        let running = Cell::new(0u32);
        let launch = || {
            linked.set(crate::activation::is_junction(&p.link()));
            assert!(
                p.link().join(format!("{APP_ID}.lua")).is_file(),
                "the script is reachable through the link"
            );
            Ok(())
        };
        // Not running, then running twice, then gone.
        let game = || {
            let n = running.get();
            running.set(n + 1);
            (1..=3).contains(&n)
        };
        // No image written here: the session ends in `GAMESHOT_NO_IMAGE`,
        // which is beside the point of this test.
        let _ = run_with(
            &p,
            &request(),
            &launch,
            &game,
            &|_| {},
            (Duration::from_secs(5), Duration::from_secs(5), Duration::from_millis(1)),
        );
        assert!(linked.get(), "the app was linked when the game started");
        assert!(!p.link().exists(), "the link is gone after the session");
        assert!(!p.work().join("job.json").exists(), "the job is gone too");
    }

    // Rule: a real folder named like the app belongs to someone else — refused
    // and left exactly as it was (golden rule 2, same guard as activation).
    #[test]
    fn a_real_folder_in_the_app_place_is_refused_and_untouched() {
        let base = crate::testutil::temp_dir("gameshot-real");
        let p = paths(&base);
        std::fs::create_dir_all(p.link()).unwrap();
        std::fs::write(p.link().join("mine.lua"), "x").unwrap();
        let err = install_app(&p).unwrap_err();
        assert_eq!(err, crate::errors::GAMESHOT_APP_FOLDER_TAKEN, "refused with its key");
        uninstall_app(&p.ac, &p.ac_documents);
        assert!(p.link().join("mine.lua").is_file(), "the real folder is untouched");
    }

    // Rule: the start-up sweep and a new session both replace a junction of
    // ours left by an interrupted session — even one to another data folder.
    #[test]
    fn a_leftover_junction_of_ours_is_replaced() {
        let base = crate::testutil::temp_dir("gameshot-leftover");
        let p = paths(&base);
        let older = base.join("older").join(SOURCE_PARENT).join(APP_ID);
        std::fs::create_dir_all(&older).unwrap();
        crate::activation::create_junction(&p.link(), &older).unwrap();
        install_app(&p).unwrap();
        assert!(
            p.link().join(format!("{APP_ID}.lua")).is_file(),
            "the link now points at this version of the app"
        );
        uninstall_app(&p.ac, &p.ac_documents);
        assert!(!p.link().exists(), "and is removed after");
        assert!(older.is_dir(), "the old target itself is untouched");
    }

    // Rule: a user app of the same id, linked by the Apps screen, is not ours:
    // refused, and left in place by the session and by the sweep.
    #[test]
    fn a_user_app_of_the_same_id_is_left_alone() {
        let base = crate::testutil::temp_dir("gameshot-userapp");
        let p = paths(&base);
        let library_app = base.join("library").join(APP_ID);
        std::fs::create_dir_all(&library_app).unwrap();
        crate::activation::create_junction(&p.link(), &library_app).unwrap();
        assert_eq!(
            install_app(&p).unwrap_err(),
            crate::errors::GAMESHOT_APP_FOLDER_TAKEN,
            "refused"
        );
        uninstall_app(&p.ac, &p.ac_documents);
        assert!(
            crate::activation::is_junction(&p.link()),
            "the user's link is still there"
        );
    }

    // Rule: the image the game wrote lands in the store under the entry name
    // the Pit Box renderer would have used, and replaces the failure marker.
    #[test]
    fn an_image_written_by_the_game_is_stored_under_its_entry_name() {
        let base = crate::testutil::temp_dir("gameshot-collect");
        let p = paths(&base);
        std::fs::write(p.thumbs.join(format!("{STEM}.fail")), "errors.previewProtected").unwrap();
        let game_ran = Cell::new(0u32);
        let launch = || {
            // Stands in for the Lua app: reads the job, writes the image.
            let job: serde_json::Value =
                serde_json::from_str(&std::fs::read_to_string(p.work().join("job.json")).unwrap()).unwrap();
            let file = job["cars"][0]["outputs"][0]["file"].as_str().unwrap().to_string();
            assert_eq!(job["cars"][0]["id"], "some_car", "the car is in the job");
            assert_eq!(
                job["cars"][0]["outputs"][0]["template"]["azimuth"], 320,
                "the template goes through untouched"
            );
            std::fs::write(file, b"png").unwrap();
            Ok(())
        };
        let game = || {
            let n = game_ran.get();
            game_ran.set(n + 1);
            n == 1
        };
        let results = run_with(
            &p,
            &request(),
            &launch,
            &game,
            &|_| {},
            (Duration::from_secs(5), Duration::from_secs(5), Duration::from_millis(1)),
        )
        .unwrap();
        let stored = p.thumbs.join(format!("{STEM}.png"));
        assert_eq!(
            results[0].path.as_deref(),
            Some(stored.to_string_lossy().as_ref()),
            "stored path returned"
        );
        assert_eq!(
            std::fs::read(&stored).unwrap(),
            b"png",
            "the game's image is the stored one"
        );
        assert!(
            !p.thumbs.join(format!("{STEM}.fail")).exists(),
            "the failure marker is gone"
        );
    }

    // Rule: a game that never starts is a failure of the session, said with
    // its own key — and the app is unlinked all the same.
    #[test]
    fn a_game_that_never_starts_fails_the_session_and_unlinks() {
        let base = crate::testutil::temp_dir("gameshot-never");
        let p = paths(&base);
        let err = run_with(
            &p,
            &request(),
            &|| Ok(()),
            &|| false,
            &|_| {},
            (
                Duration::from_millis(5),
                Duration::from_secs(5),
                Duration::from_millis(1),
            ),
        )
        .unwrap_err();
        assert_eq!(err, crate::errors::GAMESHOT_NEVER_STARTED, "said with its key");
        assert!(!p.link().exists(), "unlinked after a failed session");
    }

    // Rule: a session is refused while the game runs (SESSION§2.4), before
    // anything is linked or written.
    #[test]
    fn nothing_is_written_while_the_game_runs() {
        let base = crate::testutil::temp_dir("gameshot-busy");
        let p = paths(&base);
        let err = run(&p, &request(), &|| Ok(()), &|| true, &|_| {}).unwrap_err();
        assert_eq!(err, crate::errors::GAME_RUNNING, "refused with the usual key");
        assert!(!p.link().exists(), "nothing linked");
        assert!(!p.work().exists(), "nothing written");
    }

    // Rule: a game that closed without writing any image is a failed session,
    // said with its own key (likely a CSP that cannot), never an empty success.
    #[test]
    fn a_game_that_wrote_nothing_fails_the_session() {
        let base = crate::testutil::temp_dir("gameshot-nothing");
        let p = paths(&base);
        let ran = Cell::new(0u32);
        let game = || {
            let n = ran.get();
            ran.set(n + 1);
            n == 1
        };
        let err = run_with(
            &p,
            &request(),
            &|| Ok(()),
            &game,
            &|_| {},
            (Duration::from_secs(5), Duration::from_secs(5), Duration::from_millis(1)),
        )
        .unwrap_err();
        assert_eq!(err, crate::errors::GAMESHOT_NO_IMAGE, "said with its key");
        assert!(!p.thumbs.join(format!("{STEM}.png")).exists(), "nothing stored");
    }
}
