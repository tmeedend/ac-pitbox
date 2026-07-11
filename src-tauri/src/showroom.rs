//! Aperçu 3D natif via `acShowroom.exe`, à la racine de l'install AC —
//! distinct de Content Manager, pas de dépendance à CM (voir recherche dans
//! `docs/showroom-3d-preview-research.md`, piste 3).
//!
//! Configuration par fichier INI (`showroom_start.ini`), même principe que
//! `race.ini` : on écrit, on lance, AC lit. Le mode fenêtré (nécessaire pour
//! un futur embed dans la page) est piloté par `video.ini` — **le même
//! fichier que le vrai jeu**. Règle d'or : jamais laissé altéré durablement.
//! `video.ini` est sauvegardé avant modification et restauré dès la
//! fermeture du showroom (process tué ou fenêtré fermée par l'utilisateur),
//! avec un filet de sécurité au démarrage de l'app si une sauvegarde traîne
//! suite à un crash.

use std::fs;
use std::path::{Path, PathBuf};
use std::sync::Once;
use std::time::{Duration, Instant};

use tauri::{AppHandle, Manager};
use windows::core::{BOOL, PCWSTR};
use windows::Win32::Foundation::{CloseHandle, HANDLE, HINSTANCE, HWND, LPARAM, LRESULT, POINT, WPARAM};
use windows::Win32::Graphics::Gdi::ClientToScreen;
use windows::Win32::System::LibraryLoader::GetModuleHandleW;
use windows::Win32::System::Threading::{
    CreateProcessW, WaitForSingleObject, PROCESS_CREATION_FLAGS, PROCESS_INFORMATION, STARTF_USESHOWWINDOW,
    STARTUPINFOW,
};
use windows::Win32::UI::WindowsAndMessaging::{
    CreateWindowExW, DefWindowProcW, DestroyWindow, EnumChildWindows, EnumWindows, GetClassNameW, GetWindowLongPtrW,
    GetWindowThreadProcessId, IsWindow, PostMessageW, RegisterClassW, SetParent, SetWindowLongPtrW, SetWindowPos,
    ShowWindow, CS_HREDRAW, CS_VREDRAW, GWL_STYLE, SWP_FRAMECHANGED, SWP_NOZORDER, SW_HIDE, SW_SHOW, WM_CLOSE,
    WNDCLASSW, WS_CAPTION, WS_CHILD, WS_EX_TOOLWINDOW, WS_MAXIMIZEBOX, WS_MINIMIZEBOX, WS_POPUP, WS_SYSMENU,
    WS_THICKFRAME, WS_VISIBLE,
};

use crate::config::AppConfig;

const BACKUP_NAME: &str = "video.ini.pitbox-backup";
/// Filtre post-traitement imposé pendant l'aperçu (`[POST_PROCESS] FILTER` de
/// `video.ini`). Le filtre de l'utilisateur (souvent `pure`) assombrit
/// fortement le showroom ; `photographic` (filtre stock AC) donne un très bon
/// rendu. Appliqué et restauré avec le reste de `video.ini`.
const SHOWROOM_PP_FILTER: &str = "photographic";
/// Classe de fenêtre native d'`acShowroom.exe`, identifiée en inspectant le
/// process réel (`EnumWindows` + `GetClassName`, voir recherche §piste 3).
const SHOWROOM_WINDOW_CLASS: &str = "acShowroomW";

fn resolve_ac_cfg_dir() -> Option<PathBuf> {
    Some(dirs::document_dir()?.join("Assetto Corsa").join("cfg"))
}

/// Reproduit fidèlement le fichier généré par AC lui-même (capturé pendant
/// une session native réelle) — seuls `CAR`/`SKIN` changent. Une version
/// tronquée (sans `NEAR_PLANE`/`FAR_PLANE`/les splits d'ombre) a provoqué un
/// écran noir en test : mieux vaut garder l'intégralité des clés connues que
/// deviner lesquelles sont réellement optionnelles.
fn write_showroom_ini_at(cfg_dir: &Path, car_id: &str, skin_id: Option<&str>) -> Result<(), String> {
    fs::create_dir_all(cfg_dir).map_err(|e| format!("dossier de config AC : {e}"))?;
    let skin = skin_id.unwrap_or("");
    // Scène `studio_white` : la plus légère de loin (17 Ko contre 100–300 Mo
    // pour showroom/Hangar/industrial/beach) et **sans piste audio** (pas de
    // .bank/.wav) → chargement quasi instantané et aucune musique à couper.
    // Contrepartie : fond blanc (pas foncé). Les autres clés (caméra custom,
    // clipping, ombres) sont indépendantes de la scène.
    let content = format!(
        "[SHOWROOM]\r\n\
         CAR={car_id}\r\n\
         SKIN={skin}\r\n\
         ALLOW_SELECT_SKIN=1\r\n\
         TRACK=studio_white\r\n\
         SELECTED_SKIN=1\r\n\
         CAR_ID=0\r\n\
         \r\n\
         [FADES]\r\n\
         ENTER_EXIT_MS=0\r\n\
         \r\n\
         [PREVIEW_MODE]\r\n\
         LOOK_AT=0,0.6,0\r\n\
         CUSTOM_CAMERA_POSITION=-0.366574,0.775145,-6.12493\r\n\
         USE_CUSTOM_CAMERA=1\r\n\
         CUSTOM_CAMERA_ROLL=0\r\n\
         CUSTOM_CAMERA_EXPOSURE=94.5\r\n\
         \r\n\
         [ANIMATION]\r\n\
         MUL=0.15\r\n\
         \r\n\
         [SETTINGS]\r\n\
         ROTATION_SPEED=1.0\r\n\
         CAMERA_DISTANCE=6\r\n\
         CAMERA_HEIGHT=1.5\r\n\
         CAMERA_FOV=30\r\n\
         CAMERA_EXPOSURE=30\r\n\
         SUN_ANGLE=-50\r\n\
         SHADOW_SPLIT0=2\r\n\
         SHADOW_SPLIT1=12\r\n\
         SHADOW_SPLIT2=50\r\n\
         NEAR_PLANE=0.01\r\n\
         FAR_PLANE=200\r\n\
         MIN_EXPOSURE=0.2\r\n\
         MAX_EXPOSURE=10000\r\n"
    );
    fs::write(cfg_dir.join("showroom_start.ini"), content).map_err(|e| format!("écriture showroom_start.ini : {e}"))
}

/// Prépare `video.ini` pour la session showroom : force le mode fenêtré + une
/// taille modeste (`FULLSCREEN`/`WIDTH`/`HEIGHT` — sans réduire la taille, une
/// fenêtre « sans bordure » à la résolution du bureau reste indiscernable du
/// plein écran) et impose le filtre post-traitement `photographic`
/// (`[POST_PROCESS] FILTER`). Toutes les autres clés (refresh, anti-aliasing…)
/// restent intactes, et l'original est restauré à la fermeture (voir
/// `restore_video_ini_at`).
fn patch_video_for_showroom(original: &str) -> String {
    original
        .lines()
        .map(|l| {
            let key = l.trim_start().split('=').next().unwrap_or("");
            match key {
                "FULLSCREEN" => "FULLSCREEN=0".to_string(),
                "WIDTH" => "WIDTH=1280".to_string(),
                "HEIGHT" => "HEIGHT=720".to_string(),
                "FILTER" => format!("FILTER={SHOWROOM_PP_FILTER}"),
                _ => l.to_string(),
            }
        })
        .collect::<Vec<_>>()
        .join("\r\n")
}

/// Sauvegarde `video.ini` puis le bascule en fenêtré. No-op si une
/// sauvegarde existe déjà (session précédente pas fermée proprement : on ne
/// veut surtout pas écraser l'original par une copie déjà modifiée).
fn backup_and_force_windowed_at(cfg_dir: &Path) -> Result<(), String> {
    let path = cfg_dir.join("video.ini");
    let backup = cfg_dir.join(BACKUP_NAME);
    if backup.exists() {
        return Ok(());
    }
    let original = fs::read_to_string(&path).map_err(|e| format!("lecture video.ini : {e}"))?;
    fs::write(&backup, &original).map_err(|e| format!("sauvegarde video.ini : {e}"))?;
    fs::write(&path, patch_video_for_showroom(&original)).map_err(|e| format!("écriture video.ini : {e}"))?;
    Ok(())
}

/// Restaure `video.ini` depuis la sauvegarde si elle existe. No-op sinon —
/// appelable sans risque à la fermeture du showroom et au démarrage de l'app.
fn restore_video_ini_at(cfg_dir: &Path) -> Result<(), String> {
    let path = cfg_dir.join("video.ini");
    let backup = cfg_dir.join(BACKUP_NAME);
    if !backup.exists() {
        return Ok(());
    }
    let original = fs::read_to_string(&backup).map_err(|e| format!("lecture sauvegarde video.ini : {e}"))?;
    fs::write(&path, original).map_err(|e| format!("restauration video.ini : {e}"))?;
    fs::remove_file(&backup).map_err(|e| format!("suppression sauvegarde video.ini : {e}"))?;
    Ok(())
}

/// Filet de sécurité à appeler une fois au démarrage de l'app : restaure
/// `video.ini` si une sauvegarde traîne (Pit Box ou le showroom ont été tués
/// avant la restauration normale).
pub fn restore_orphaned_video_ini() {
    if let Some(dir) = resolve_ac_cfg_dir() {
        let _ = restore_video_ini_at(&dir);
    }
}

/// Lance `acShowroom.exe`, **fenêtre initiale masquée** (`STARTUPINFO` +
/// `SW_HIDE`), en s'appuyant sur le répertoire courant `ac`. Renvoie
/// `(pid, handle process brut)`. Masquer à la naissance est la seule façon
/// fiable d'éviter le flash noir top-level : tenter de masquer/réduire la
/// fenêtre APRÈS coup échoue car acShowroom réasserte sa géométrie/visibilité
/// pendant l'init DirectX (cf. docs/showroom-3d-preview-research.md). Le handle
/// process (au lieu du `Child` de `std`) sert à attendre la fin du process
/// pour restaurer `video.ini`.
fn spawn_hidden(exe: &Path, ac: &Path) -> Result<(u32, isize), String> {
    let app = wstr(&exe.to_string_lossy());
    let cwd = wstr(&ac.to_string_lossy());
    let si = STARTUPINFOW {
        cb: std::mem::size_of::<STARTUPINFOW>() as u32,
        dwFlags: STARTF_USESHOWWINDOW,
        wShowWindow: SW_HIDE.0 as u16,
        ..Default::default()
    };
    let mut pi = PROCESS_INFORMATION::default();
    unsafe {
        CreateProcessW(
            PCWSTR(app.as_ptr()),
            None,
            None,
            None,
            false,
            PROCESS_CREATION_FLAGS(0),
            None,
            PCWSTR(cwd.as_ptr()),
            &si,
            &mut pi,
        )
        .map_err(|e| format!("lancement d'acShowroom.exe : {e}"))?;
        // Le handle du thread principal ne nous sert pas ; on ne garde que
        // celui du process (pour l'attente de fin).
        let _ = CloseHandle(pi.hThread);
    }
    Ok((pi.dwProcessId, pi.hProcess.0 as isize))
}

/// Lance `acShowroom.exe` ciblé sur `car_id` (+ skin optionnel). Bascule
/// `video.ini` en fenêtré le temps de la session, restauré automatiquement
/// à la fermeture (fenêtre fermée par l'utilisateur ou process tué). Renvoie
/// le PID du process lancé, nécessaire pour le retrouver et l'intégrer dans
/// la page (§ Phase B). La fenêtre naît masquée (voir `spawn_hidden`) et est
/// ré-affichée par `attach()` une fois reparentée.
pub fn open_native_showroom(cfg: &AppConfig, car_id: &str, skin_id: Option<&str>) -> Result<u32, String> {
    let ac = cfg.ac_install_path.as_ref().ok_or("dossier AC non configuré")?.clone();
    // Garde-fou : `acShowroom.exe` cherche `data/lods.ini` sous `content/cars/<id>`
    // et plante (fenêtre d'erreur native, hors de notre contrôle) si `car_id`
    // n'est pas une vraie voiture — ex. l'id d'un circuit passé par erreur
    // depuis le front. Mieux vaut une erreur propre ici qu'un crash natif.
    if !crate::modscan::is_car(&ac.join("content").join("cars").join(car_id)) {
        return Err(format!("« {car_id} » n'est pas une voiture valide — aperçu 3D indisponible"));
    }
    let exe = ac.join("acShowroom.exe");
    if !exe.is_file() {
        return Err("acShowroom.exe introuvable dans le dossier d'installation AC".into());
    }
    let cfg_dir = resolve_ac_cfg_dir().ok_or("dossier Documents introuvable")?;

    write_showroom_ini_at(&cfg_dir, car_id, skin_id)?;
    backup_and_force_windowed_at(&cfg_dir)?;

    let (pid, handle_raw) = spawn_hidden(&exe, &ac)?;

    // Restauration dès la fermeture du showroom, quelle que soit la cause
    // (fenêtre fermée par l'utilisateur, ou process tué). Un filet de sécurité
    // au démarrage de l'app (restore_orphaned_video_ini) couvre le cas où ce
    // thread n'aboutirait pas (crash). HANDLE n'étant pas Send, on traverse la
    // frontière de thread via l'entier brut, reconstruit de l'autre côté.
    std::thread::spawn(move || {
        let handle = HANDLE(handle_raw as *mut _);
        unsafe {
            let _ = WaitForSingleObject(handle, u32::MAX);
            let _ = CloseHandle(handle);
        }
        eprintln!("[showroom] process {pid} terminé");
        if let Some(dir) = resolve_ac_cfg_dir() {
            let _ = restore_video_ini_at(&dir);
        }
    });

    Ok(pid)
}

struct FindCtx {
    /// `None` = ne filtre pas par PID (utilisé pour chercher parmi les enfants
    /// d'une fenêtre overlay, où le PID est déjà implicitement garanti par la
    /// portée de l'énumération).
    pid: Option<u32>,
    /// HWND stocké en pointeur brut (isize) : `HWND` n'est pas `Send`, on ne
    /// le reconstruit qu'après avoir traversé la frontière de thread.
    found: Option<isize>,
}

unsafe extern "system" fn enum_proc(hwnd: HWND, lparam: LPARAM) -> BOOL {
    let ctx = &mut *(lparam.0 as *mut FindCtx);
    let pid_ok = match ctx.pid {
        None => true,
        Some(want) => {
            let mut pid = 0u32;
            let _ = GetWindowThreadProcessId(hwnd, Some(&mut pid));
            pid == want
        }
    };
    if pid_ok {
        let mut buf = [0u16; 64];
        let len = GetClassNameW(hwnd, &mut buf);
        if len > 0 && String::from_utf16_lossy(&buf[..len as usize]) == SHOWROOM_WINDOW_CLASS {
            ctx.found = Some(hwnd.0 as isize);
            return BOOL(0); // arrête l'énumération : trouvé.
        }
    }
    BOOL(1) // continue.
}

/// Retrouve la fenêtre **top-level** native du showroom pour un PID donné
/// (classe `acShowroomW`). `None` si le process n'a pas encore créé sa
/// fenêtre (démarrage/initialisation DirectX en cours), n'existe plus, ou a
/// déjà été intégrée (elle n'est alors plus top-level, voir
/// `find_showroom_child`).
fn find_showroom_window(pid: u32) -> Option<HWND> {
    let mut ctx = FindCtx { pid: Some(pid), found: None };
    unsafe {
        let _ = EnumWindows(Some(enum_proc), LPARAM(&mut ctx as *mut FindCtx as isize));
    }
    ctx.found.map(|raw| HWND(raw as *mut _))
}

/// Retrouve la fenêtre du showroom parmi les enfants de l'overlay qui
/// l'héberge (après intégration, elle n'est plus une fenêtre top-level).
fn find_showroom_child(overlay: HWND) -> Option<HWND> {
    let mut ctx = FindCtx { pid: None, found: None };
    unsafe {
        let _ = EnumChildWindows(Some(overlay), Some(enum_proc), LPARAM(&mut ctx as *mut FindCtx as isize));
    }
    ctx.found.map(|raw| HWND(raw as *mut _))
}

/// Attend une fenêtre showroom **valide** pour `pid` (classe `acShowroomW`),
/// avec un budget de temps donné. `acShowroom.exe` peut détruire et recréer sa
/// fenêtre pendant l'initialisation DirectX (splash puis fenêtre définitive) —
/// `find_showroom_window` seul peut donc renvoyer un HWND déjà obsolète au
/// moment où il est utilisé un peu plus tard (cause de l'erreur intermittente
/// « handle de fenêtre non valide », 0x80070578/ERROR_INVALID_WINDOW_HANDLE).
/// `IsWindow` revérifie juste avant de rendre la main ; si la fenêtre trouvée
/// s'avère déjà morte, on continue de sonder au lieu de la renvoyer telle quelle.
fn wait_for_valid_window(pid: u32, deadline: Instant, poll: Duration) -> Option<HWND> {
    loop {
        if let Some(h) = find_showroom_window(pid) {
            if unsafe { IsWindow(Some(h)) }.as_bool() {
                return Some(h);
            }
        }
        if Instant::now() >= deadline {
            return None;
        }
        std::thread::sleep(poll);
    }
}

const OVERLAY_CLASS_NAME: &str = "PitboxShowroomOverlay";

fn wstr(s: &str) -> Vec<u16> {
    s.encode_utf16().chain(std::iter::once(0)).collect()
}

unsafe extern "system" fn overlay_wndproc(hwnd: HWND, msg: u32, wparam: WPARAM, lparam: LPARAM) -> LRESULT {
    DefWindowProcW(hwnd, msg, wparam, lparam)
}

/// Enregistre la classe de la fenêtre overlay (une fois par process).
fn ensure_overlay_class_registered() {
    static ONCE: Once = Once::new();
    ONCE.call_once(|| unsafe {
        let class_name = wstr(OVERLAY_CLASS_NAME);
        let hinstance: HINSTANCE = GetModuleHandleW(None).unwrap_or_default().into();
        let wc = WNDCLASSW {
            style: CS_HREDRAW | CS_VREDRAW,
            lpfnWndProc: Some(overlay_wndproc),
            hInstance: hinstance,
            lpszClassName: PCWSTR(class_name.as_ptr()),
            ..Default::default()
        };
        RegisterClassW(&wc);
    });
}

fn client_to_screen_origin(hwnd: HWND) -> (i32, i32) {
    let mut pt = POINT::default();
    unsafe {
        let _ = ClientToScreen(hwnd, &mut pt);
    }
    (pt.x, pt.y)
}

/// Poignée du showroom intégré : PID du process + fenêtre overlay créée par
/// Pit Box (`None` tant que l'intégration n'a pas encore eu lieu).
pub struct ShowroomHandle {
    pub pid: u32,
    pub overlay: Option<isize>,
}

pub struct ShowroomState(pub std::sync::Mutex<Option<ShowroomHandle>>);

/// Intègre la fenêtre du showroom (PID `pid`) dans la page — **pas** en enfant
/// direct de la fenêtre principale : WebView2 compose son rendu accéléré
/// au-dessus de toute fenêtre native sœur, peu importe l'ordre Z classique
/// (problème dit "d'espace aérien", confirmé en direct : la fenêtre native
/// restait invisible bien que correctement reparentée et visible au sens
/// Win32). La contourner nécessite une **fenêtre overlay séparée**, top-level
/// mais possédée par la fenêtre principale (jamais vue par l'utilisateur en
/// tant que telle), dans laquelle le showroom est ensuite intégré.
///
/// Créée sur le **thread principal** de Tauri (`run_on_main_thread`), pas sur
/// le thread (jetable, recyclé par le pool) qui exécute la commande : Windows
/// détruit automatiquement les fenêtres d'un thread qui se termine, ce qui
/// entraînait la disparition de l'overlay — et avec elle, la destruction du
/// showroom reparenté dedans, provoquant son crash pur et simple (constaté :
/// fenêtre noire puis disparition totale du process).
///
/// `(x, y, width, height)` sont des pixels physiques relatifs à la zone
/// cliente de la fenêtre principale. Attend l'apparition de la fenêtre du
/// showroom jusqu'à 10s (le temps que le process démarre et initialise
/// DirectX) avant de basculer sur le thread principal.
pub fn attach(app: &AppHandle, pid: u32, x: i32, y: i32, width: i32, height: i32) -> Result<isize, String> {
    let deadline = Instant::now() + Duration::from_secs(10);
    let target_raw = match wait_for_valid_window(pid, deadline, Duration::from_millis(250)) {
        Some(h) => h.0 as isize,
        None => return Err("fenêtre du showroom introuvable (délai dépassé)".into()),
    };

    eprintln!("[showroom] attach: fenêtre trouvée hwnd={target_raw:#x}, dispatch vers le thread principal");
    let (tx, rx) = std::sync::mpsc::channel::<Result<isize, String>>();
    let app_for_closure = app.clone();
    app.run_on_main_thread(move || {
        eprintln!("[showroom] attach: closure exécutée sur le thread principal");
        let app = app_for_closure;
        let result = (|| -> Result<isize, String> {
            let win = app.get_webview_window("main").ok_or("fenêtre principale introuvable")?;
            let host = win.hwnd().map_err(|e| e.to_string())?;

            // Re-vérifie juste avant usage : le HWND trouvé avant le dispatch
            // vers ce thread peut avoir été détruit/recréé entre-temps pendant
            // l'init DirectX d'acShowroom (fenêtre de démarrage remplacée par
            // la définitive) — cause de l'erreur intermittente « handle de
            // fenêtre non valide » (0x80070578/ERROR_INVALID_WINDOW_HANDLE).
            // Re-sonde brièvement si besoin (thread principal : budget court
            // pour ne pas geler l'UI, la recréation est typiquement immédiate).
            let mut target = HWND(target_raw as *mut _);
            if !unsafe { IsWindow(Some(target)) }.as_bool() {
                eprintln!("[showroom] attach: HWND initial devenu invalide, nouvelle recherche...");
                target = wait_for_valid_window(pid, Instant::now() + Duration::from_secs(2), Duration::from_millis(50))
                    .ok_or("la fenêtre du showroom a disparu avant l'intégration")?;
                eprintln!("[showroom] attach: nouvelle fenêtre trouvée hwnd={:#x}", target.0 as isize);
            }
            eprintln!("[showroom] attach: host={:#x}", host.0 as isize);

            ensure_overlay_class_registered();
            let (ox, oy) = client_to_screen_origin(host);
            eprintln!("[showroom] attach: origine écran host=({ox},{oy}), rect demandé=({x},{y},{width},{height})");
            let class_name = wstr(OVERLAY_CLASS_NAME);
            let title = wstr("Pit Box — Aperçu 3D");
            let overlay = unsafe {
                CreateWindowExW(
                    WS_EX_TOOLWINDOW,
                    PCWSTR(class_name.as_ptr()),
                    PCWSTR(title.as_ptr()),
                    WS_POPUP | WS_VISIBLE,
                    ox + x,
                    oy + y,
                    width,
                    height,
                    Some(host),
                    None,
                    None,
                    None,
                )
            }
            .map_err(|e| format!("création de la fenêtre overlay : {e}"))?;
            eprintln!("[showroom] attach: overlay créé hwnd={:#x}", overlay.0 as isize);

            unsafe {
                let style = GetWindowLongPtrW(target, GWL_STYLE);
                eprintln!("[showroom] attach: style avant = {style:#x}");
                // WS_POPUP seul ne suffit pas : la fenêtre garde sa propre
                // barre de titre/bordure/boutons une fois enfant (confirmé
                // avec le prototype de test hors-app) tant qu'on ne retire
                // pas aussi CAPTION/SYSMENU/THICKFRAME/MIN·MAXIMIZEBOX.
                let remove = WS_POPUP.0 | WS_CAPTION.0 | WS_THICKFRAME.0 | WS_SYSMENU.0 | WS_MINIMIZEBOX.0 | WS_MAXIMIZEBOX.0;
                let new_style = (style & !(remove as isize)) | (WS_CHILD.0 as isize);
                SetWindowLongPtrW(target, GWL_STYLE, new_style);
                eprintln!("[showroom] attach: style après = {new_style:#x}, SetParent...");
                SetParent(target, Some(overlay)).map_err(|e| e.to_string())?;
                eprintln!("[showroom] attach: SetParent OK, SetWindowPos...");
                SetWindowPos(target, None, 0, 0, width, height, SWP_NOZORDER | SWP_FRAMECHANGED)
                    .map_err(|e| e.to_string())?;
                eprintln!("[showroom] attach: SetWindowPos OK, ShowWindow enfant + overlay...");
                // La fenêtre est née masquée (SW_HIDE au spawn, cf.
                // open_native_showroom) : on la ré-affiche maintenant qu'elle
                // est reparentée et positionnée dans l'overlay.
                let _ = ShowWindow(target, SW_SHOW);
                let _ = ShowWindow(overlay, SW_SHOW);
            }
            eprintln!("[showroom] attach: terminé avec succès");

            Ok(overlay.0 as isize)
        })();
        if let Err(e) = &result {
            eprintln!("[showroom] attach: ÉCHEC dans la closure : {e}");
        }
        let _ = tx.send(result);
    })
    .map_err(|e| e.to_string())?;

    rx.recv().map_err(|e| e.to_string())?
}

/// Repositionne/redimensionne l'overlay (donc le showroom qu'il héberge) —
/// suivi du scroll/resize de la page côté front, puisque la fenêtre native
/// ne fait pas partie du rendu web et ne défile pas avec la page.
pub fn reposition(host: HWND, overlay_raw: isize, x: i32, y: i32, width: i32, height: i32) -> Result<(), String> {
    eprintln!("[showroom] reposition: overlay={overlay_raw:#x} rect=({x},{y},{width},{height})");
    let overlay = HWND(overlay_raw as *mut _);
    let (ox, oy) = client_to_screen_origin(host);
    unsafe {
        SetWindowPos(overlay, None, ox + x, oy + y, width, height, SWP_NOZORDER).map_err(|e| e.to_string())?;
    }
    if let Some(target) = find_showroom_child(overlay) {
        unsafe {
            SetWindowPos(target, None, 0, 0, width, height, SWP_NOZORDER).map_err(|e| e.to_string())?;
        }
    }
    eprintln!("[showroom] reposition: OK");
    Ok(())
}

/// Demande la fermeture propre du showroom (`WM_CLOSE`) puis détruit
/// l'overlay — le thread lancé par `open_native_showroom` restaure
/// `video.ini` dès que le process se termine, intégré ou non.
pub fn close(pid: u32, overlay_raw: Option<isize>) -> Result<(), String> {
    let target = match overlay_raw {
        Some(raw) => find_showroom_child(HWND(raw as *mut _)),
        None => find_showroom_window(pid),
    };
    if let Some(target) = target {
        unsafe {
            let _ = PostMessageW(Some(target), WM_CLOSE, WPARAM(0), LPARAM(0));
        }
    }
    if let Some(raw) = overlay_raw {
        unsafe {
            let _ = DestroyWindow(HWND(raw as *mut _));
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn writes_showroom_ini_with_car_and_skin() {
        let base = std::env::temp_dir().join(format!("pitbox-showroom-{}", uuid::Uuid::new_v4()));
        write_showroom_ini_at(&base, "ks_toyota_celica_st185", Some("00_racing_3")).unwrap();
        let content = fs::read_to_string(base.join("showroom_start.ini")).unwrap();
        assert!(content.contains("CAR=ks_toyota_celica_st185"));
        assert!(content.contains("SKIN=00_racing_3"));
        assert!(content.contains("[SHOWROOM]"));
        // Scène légère et silencieuse (17 Ko, sans audio), cf. write_showroom_ini_at.
        assert!(content.contains("TRACK=studio_white"));
        // Clés de clipping/ombres : leur absence a provoqué un écran noir en test réel.
        assert!(content.contains("NEAR_PLANE=0.01"));
        assert!(content.contains("FAR_PLANE=200"));
        assert!(content.contains("[FADES]"));
        assert!(content.contains("[ANIMATION]"));
        let _ = fs::remove_dir_all(&base);
    }

    #[test]
    fn patch_video_shrinks_size_sets_filter_and_leaves_other_keys() {
        let original = "[POST_PROCESS]\r\nENABLED=1\r\nFILTER=pure\r\nGLARE=5\r\n[VIDEO]\r\nFULLSCREEN=1\r\nWIDTH=3840\r\nHEIGHT=2160\r\nREFRESH=144\r\n";
        let patched = patch_video_for_showroom(original);
        assert!(patched.contains("FULLSCREEN=0"));
        assert!(patched.contains("WIDTH=1280"));
        assert!(patched.contains("HEIGHT=720"));
        // Filtre PP forcé sur photographic (l'original 'pure' assombrissait).
        assert!(patched.contains("FILTER=photographic"));
        assert!(!patched.contains("FILTER=pure"));
        // Clés voisines intactes (même section et autres).
        assert!(patched.contains("GLARE=5"));
        assert!(patched.contains("ENABLED=1"));
        assert!(patched.contains("REFRESH=144"));
        assert!(!patched.contains("FULLSCREEN=1"));
        assert!(!patched.contains("WIDTH=3840"));
        assert!(!patched.contains("HEIGHT=2160"));
    }

    #[test]
    fn backup_and_restore_video_ini_roundtrip() {
        let base = std::env::temp_dir().join(format!("pitbox-video-{}", uuid::Uuid::new_v4()));
        fs::create_dir_all(&base).unwrap();
        let original = "[VIDEO]\r\nFULLSCREEN=1\r\nWIDTH=3840\r\nHEIGHT=2160\r\n";
        fs::write(base.join("video.ini"), original).unwrap();

        backup_and_force_windowed_at(&base).unwrap();
        let modified = fs::read_to_string(base.join("video.ini")).unwrap();
        assert!(modified.contains("FULLSCREEN=0"));
        assert!(base.join(BACKUP_NAME).is_file());

        // Un second appel (session déjà en cours) ne doit pas écraser la sauvegarde.
        fs::write(base.join("video.ini"), "FULLSCREEN=0\r\nCORRUPTED=1\r\n").unwrap();
        backup_and_force_windowed_at(&base).unwrap();
        let backup_content = fs::read_to_string(base.join(BACKUP_NAME)).unwrap();
        assert_eq!(backup_content, original);

        restore_video_ini_at(&base).unwrap();
        let restored = fs::read_to_string(base.join("video.ini")).unwrap();
        assert_eq!(restored, original);
        assert!(!base.join(BACKUP_NAME).exists());

        // Restaurer sans sauvegarde présente est un no-op sûr.
        restore_video_ini_at(&base).unwrap();

        let _ = fs::remove_dir_all(&base);
    }
}
