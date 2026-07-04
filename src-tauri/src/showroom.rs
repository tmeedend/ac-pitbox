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
use std::process::Command;
use std::sync::Once;
use std::time::{Duration, Instant};

use tauri::{AppHandle, Manager};
use windows::core::{BOOL, PCWSTR};
use windows::Win32::Foundation::{HINSTANCE, HWND, LPARAM, LRESULT, POINT, WPARAM};
use windows::Win32::Graphics::Gdi::ClientToScreen;
use windows::Win32::System::LibraryLoader::GetModuleHandleW;
use windows::Win32::UI::WindowsAndMessaging::{
    CreateWindowExW, DefWindowProcW, DestroyWindow, EnumChildWindows, EnumWindows, GetClassNameW, GetWindowLongPtrW,
    GetWindowThreadProcessId, PostMessageW, RegisterClassW, SetParent, SetWindowLongPtrW, SetWindowPos, ShowWindow,
    CS_HREDRAW, CS_VREDRAW, GWL_STYLE, SWP_FRAMECHANGED, SWP_NOZORDER, SW_SHOW, WM_CLOSE, WNDCLASSW, WS_CAPTION,
    WS_CHILD, WS_EX_TOOLWINDOW, WS_MAXIMIZEBOX, WS_MINIMIZEBOX, WS_POPUP, WS_SYSMENU, WS_THICKFRAME, WS_VISIBLE,
};

use crate::config::AppConfig;

const BACKUP_NAME: &str = "video.ini.pitbox-backup";
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
    let content = format!(
        "[SHOWROOM]\r\n\
         CAR={car_id}\r\n\
         SKIN={skin}\r\n\
         ALLOW_SELECT_SKIN=1\r\n\
         TRACK=showroom\r\n\
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

/// Force le mode fenêtré et une taille modeste dans `video.ini` : `FULLSCREEN`,
/// `WIDTH`, `HEIGHT`. Sans réduire aussi la taille, une fenêtre "sans bordure"
/// à la résolution du bureau (ex. 3840×2160) reste visuellement indiscernable
/// du plein écran. Toutes les autres clés (refresh, anti-aliasing…) restent
/// intactes.
fn force_windowed(original: &str) -> String {
    original
        .lines()
        .map(|l| {
            let key = l.trim_start().split('=').next().unwrap_or("");
            match key {
                "FULLSCREEN" => "FULLSCREEN=0",
                "WIDTH" => "WIDTH=1280",
                "HEIGHT" => "HEIGHT=720",
                _ => l,
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
    fs::write(&path, force_windowed(&original)).map_err(|e| format!("écriture video.ini : {e}"))?;
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

/// Lance `acShowroom.exe` ciblé sur `car_id` (+ skin optionnel). Bascule
/// `video.ini` en fenêtré le temps de la session, restauré automatiquement
/// à la fermeture (fenêtre fermée par l'utilisateur ou process tué). Renvoie
/// le PID du process lancé, nécessaire pour le retrouver et l'intégrer dans
/// la page (§ Phase B).
pub fn open_native_showroom(cfg: &AppConfig, car_id: &str, skin_id: Option<&str>) -> Result<u32, String> {
    let ac = cfg.ac_install_path.as_ref().ok_or("dossier AC non configuré")?.clone();
    let exe = ac.join("acShowroom.exe");
    if !exe.is_file() {
        return Err("acShowroom.exe introuvable dans le dossier d'installation AC".into());
    }
    let cfg_dir = resolve_ac_cfg_dir().ok_or("dossier Documents introuvable")?;

    write_showroom_ini_at(&cfg_dir, car_id, skin_id)?;
    backup_and_force_windowed_at(&cfg_dir)?;

    let mut child = Command::new(&exe)
        .current_dir(&ac)
        .spawn()
        .map_err(|e| format!("lancement d'acShowroom.exe : {e}"))?;
    let pid = child.id();

    // Restauration dès la fermeture du showroom, quelle que soit la cause
    // (fenêtre fermée par l'utilisateur, ou process tué).
    std::thread::spawn(move || {
        let _ = child.wait();
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
    let target_raw = loop {
        if let Some(h) = find_showroom_window(pid) {
            break h.0 as isize;
        }
        if Instant::now() >= deadline {
            return Err("fenêtre du showroom introuvable (délai dépassé)".into());
        }
        std::thread::sleep(Duration::from_millis(250));
    };

    let (tx, rx) = std::sync::mpsc::channel::<Result<isize, String>>();
    let app_for_closure = app.clone();
    app.run_on_main_thread(move || {
        let app = app_for_closure;
        let result = (|| -> Result<isize, String> {
            let win = app.get_webview_window("main").ok_or("fenêtre principale introuvable")?;
            let host = win.hwnd().map_err(|e| e.to_string())?;
            let target = HWND(target_raw as *mut _);

            ensure_overlay_class_registered();
            let (ox, oy) = client_to_screen_origin(host);
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

            unsafe {
                let style = GetWindowLongPtrW(target, GWL_STYLE);
                // WS_POPUP seul ne suffit pas : la fenêtre garde sa propre
                // barre de titre/bordure/boutons une fois enfant (confirmé
                // avec le prototype de test hors-app) tant qu'on ne retire
                // pas aussi CAPTION/SYSMENU/THICKFRAME/MIN·MAXIMIZEBOX.
                let remove = WS_POPUP.0 | WS_CAPTION.0 | WS_THICKFRAME.0 | WS_SYSMENU.0 | WS_MINIMIZEBOX.0 | WS_MAXIMIZEBOX.0;
                let new_style = (style & !(remove as isize)) | (WS_CHILD.0 as isize);
                SetWindowLongPtrW(target, GWL_STYLE, new_style);
                SetParent(target, Some(overlay)).map_err(|e| e.to_string())?;
                SetWindowPos(target, None, 0, 0, width, height, SWP_NOZORDER | SWP_FRAMECHANGED)
                    .map_err(|e| e.to_string())?;
                let _ = ShowWindow(overlay, SW_SHOW);
            }

            Ok(overlay.0 as isize)
        })();
        let _ = tx.send(result);
    })
    .map_err(|e| e.to_string())?;

    rx.recv().map_err(|e| e.to_string())?
}

/// Repositionne/redimensionne l'overlay (donc le showroom qu'il héberge) —
/// suivi du scroll/resize de la page côté front, puisque la fenêtre native
/// ne fait pas partie du rendu web et ne défile pas avec la page.
pub fn reposition(host: HWND, overlay_raw: isize, x: i32, y: i32, width: i32, height: i32) -> Result<(), String> {
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
        // Clés de clipping/ombres : leur absence a provoqué un écran noir en test réel.
        assert!(content.contains("NEAR_PLANE=0.01"));
        assert!(content.contains("FAR_PLANE=200"));
        assert!(content.contains("[FADES]"));
        assert!(content.contains("[ANIMATION]"));
        let _ = fs::remove_dir_all(&base);
    }

    #[test]
    fn force_windowed_shrinks_size_and_leaves_other_keys() {
        let original = "[VIDEO]\r\nFULLSCREEN=1\r\nWIDTH=3840\r\nHEIGHT=2160\r\nREFRESH=144\r\n";
        let windowed = force_windowed(original);
        assert!(windowed.contains("FULLSCREEN=0"));
        assert!(windowed.contains("WIDTH=1280"));
        assert!(windowed.contains("HEIGHT=720"));
        assert!(windowed.contains("REFRESH=144"));
        assert!(!windowed.contains("FULLSCREEN=1"));
        assert!(!windowed.contains("WIDTH=3840"));
        assert!(!windowed.contains("HEIGHT=2160"));
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
