//! Outil de test jetable, hors de l'app Pit Box : vérifie la mécanique
//! Win32 pure (spawn acShowroom.exe, reparenting, positionnement) dans une
//! fenêtre native SANS WebView2, pour isoler "est-ce que le reparenting
//! marche du tout" de "est-ce que WebView2 s'en mêle". Jamais committé
//! (voir tools/ dans .gitignore / convention du projet).
//!
//! Usage : showroom-embed-test.exe [car_id] [skin_id]
//! Par défaut : ks_toyota_celica_st185 / 00_racing_3
//!
//! Mode par défaut : reparente directement dans la fenêtre hôte (pas
//! d'overlay séparé) — le test le plus simple possible. Passer `--overlay`
//! pour tester la variante avec fenêtre overlay séparée (comme dans l'app).

use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::time::{Duration, Instant};

use windows::core::{BOOL, PCWSTR};
use windows::Win32::Foundation::{HINSTANCE, HWND, LPARAM, LRESULT, POINT, WPARAM};
use windows::Win32::Graphics::Gdi::ClientToScreen;
use windows::Win32::System::LibraryLoader::GetModuleHandleW;
use windows::Win32::UI::WindowsAndMessaging::{
    CreateWindowExW, DefWindowProcW, DispatchMessageW, EnumWindows, GetClassNameW, GetMessageW, GetWindowLongPtrW,
    GetWindowThreadProcessId, PostQuitMessage, RegisterClassW, SetForegroundWindow, SetParent, SetWindowLongPtrW,
    SetWindowPos, ShowWindow, TranslateMessage, CS_HREDRAW, CS_VREDRAW, CW_USEDEFAULT, GWL_STYLE, MSG,
    SWP_FRAMECHANGED, SWP_NOZORDER, SW_SHOW, WM_DESTROY, WNDCLASSW, WS_CAPTION, WS_CHILD, WS_EX_TOOLWINDOW,
    WS_MAXIMIZEBOX, WS_MINIMIZEBOX, WS_OVERLAPPEDWINDOW, WS_POPUP, WS_SYSMENU, WS_THICKFRAME, WS_VISIBLE,
};

// --- Config machine (outil jetable, pas générique) --------------------------
const AC_INSTALL: &str = r"D:\SteamLibrary\steamapps\common\assettocorsa";
const SHOWROOM_WINDOW_CLASS: &str = "acShowroomW";
const HOST_CLASS: &str = "ShowroomEmbedTestHost";
const OVERLAY_CLASS: &str = "ShowroomEmbedTestOverlay";
const BACKUP_NAME: &str = "video.ini.pitbox-backup";

fn wstr(s: &str) -> Vec<u16> {
    s.encode_utf16().chain(std::iter::once(0)).collect()
}

fn ac_cfg_dir() -> PathBuf {
    dirs_document_dir().join("Assetto Corsa").join("cfg")
}

// Pas de dépendance à `dirs` ici (outil minimal) : on lit %USERPROFILE% et on
// ajoute Documents, suffisant pour ce test ponctuel sur cette machine.
fn dirs_document_dir() -> PathBuf {
    let profile = std::env::var("USERPROFILE").expect("USERPROFILE introuvable");
    PathBuf::from(profile).join("Documents")
}

fn write_showroom_ini(cfg_dir: &Path, car_id: &str, skin_id: &str) {
    let content = format!(
        "[SHOWROOM]\r\n\
         CAR={car_id}\r\n\
         SKIN={skin_id}\r\n\
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
    fs::write(cfg_dir.join("showroom_start.ini"), content).expect("écriture showroom_start.ini");
}

fn backup_and_force_windowed(cfg_dir: &Path) {
    let path = cfg_dir.join("video.ini");
    let backup = cfg_dir.join(BACKUP_NAME);
    if backup.exists() {
        println!("(sauvegarde video.ini déjà présente, on ne l'écrase pas)");
        return;
    }
    let original = fs::read_to_string(&path).expect("lecture video.ini");
    fs::write(&backup, &original).expect("sauvegarde video.ini");
    let windowed: String = original
        .lines()
        .map(|l| {
            let key = l.trim_start().split('=').next().unwrap_or("");
            match key {
                "FULLSCREEN" => "FULLSCREEN=0",
                "WIDTH" => "WIDTH=900",
                "HEIGHT" => "HEIGHT=600",
                _ => l,
            }
        })
        .collect::<Vec<_>>()
        .join("\r\n");
    fs::write(&path, windowed).expect("écriture video.ini");
}

fn restore_video_ini(cfg_dir: &Path) {
    let path = cfg_dir.join("video.ini");
    let backup = cfg_dir.join(BACKUP_NAME);
    if !backup.exists() {
        return;
    }
    let original = fs::read_to_string(&backup).expect("lecture sauvegarde");
    fs::write(&path, original).expect("restauration video.ini");
    fs::remove_file(&backup).expect("suppression sauvegarde");
    println!("video.ini restauré.");
}

// --- Recherche de fenêtre ----------------------------------------------------

struct FindCtx {
    pid: Option<u32>,
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
            return BOOL(0);
        }
    }
    BOOL(1)
}

fn find_showroom_top_level(pid: u32) -> Option<HWND> {
    let mut ctx = FindCtx { pid: Some(pid), found: None };
    unsafe {
        let _ = EnumWindows(Some(enum_proc), LPARAM(&mut ctx as *mut FindCtx as isize));
    }
    ctx.found.map(|raw| HWND(raw as *mut _))
}

// --- Fenêtres hôte + overlay --------------------------------------------------

unsafe extern "system" fn host_wndproc(hwnd: HWND, msg: u32, wparam: WPARAM, lparam: LPARAM) -> LRESULT {
    if msg == WM_DESTROY {
        PostQuitMessage(0);
        return LRESULT(0);
    }
    DefWindowProcW(hwnd, msg, wparam, lparam)
}

unsafe extern "system" fn plain_wndproc(hwnd: HWND, msg: u32, wparam: WPARAM, lparam: LPARAM) -> LRESULT {
    DefWindowProcW(hwnd, msg, wparam, lparam)
}

fn register_class(name: &str, wndproc: windows::Win32::UI::WindowsAndMessaging::WNDPROC) -> Vec<u16> {
    let class_name = wstr(name);
    unsafe {
        let hinstance: HINSTANCE = GetModuleHandleW(None).unwrap_or_default().into();
        let wc = WNDCLASSW {
            style: CS_HREDRAW | CS_VREDRAW,
            lpfnWndProc: wndproc,
            hInstance: hinstance,
            lpszClassName: PCWSTR(class_name.as_ptr()),
            ..Default::default()
        };
        RegisterClassW(&wc);
    }
    class_name
}

fn client_to_screen_origin(hwnd: HWND) -> (i32, i32) {
    let mut pt = POINT::default();
    unsafe {
        let _ = ClientToScreen(hwnd, &mut pt);
    }
    (pt.x, pt.y)
}

fn main() {
    let args: Vec<String> = std::env::args().collect();
    let use_overlay = args.iter().any(|a| a == "--overlay");
    let positional: Vec<&String> = args.iter().skip(1).filter(|a| !a.starts_with("--")).collect();
    let car_id = positional.first().map(|s| s.to_string()).unwrap_or_else(|| "ks_toyota_celica_st185".to_string());
    let skin_id = positional.get(1).map(|s| s.to_string()).unwrap_or_else(|| "00_racing_3".to_string());

    println!("Mode : {}", if use_overlay { "overlay séparé" } else { "reparent direct dans l'hôte" });
    println!("Voiture : {car_id} / skin {skin_id}");

    let cfg_dir = ac_cfg_dir();
    write_showroom_ini(&cfg_dir, &car_id, &skin_id);
    backup_and_force_windowed(&cfg_dir);

    // Fenêtre hôte : un vrai WS_OVERLAPPEDWINDOW normal, message-pompé par CE
    // thread (le même qui va créer l'overlay et faire le reparenting) — donc
    // jamais orpheline, contrairement au bug rencontré dans l'app réelle.
    let host_class = register_class(HOST_CLASS, Some(host_wndproc));
    let title = wstr("Showroom Embed Test — ferme cette fenêtre pour quitter");
    let host = unsafe {
        CreateWindowExW(
            Default::default(),
            PCWSTR(host_class.as_ptr()),
            PCWSTR(title.as_ptr()),
            WS_OVERLAPPEDWINDOW | WS_VISIBLE,
            CW_USEDEFAULT,
            CW_USEDEFAULT,
            900,
            600,
            None,
            None,
            None,
            None,
        )
    }
    .expect("création fenêtre hôte");

    // Lance acShowroom.exe.
    let exe = Path::new(AC_INSTALL).join("acShowroom.exe");
    let mut child = Command::new(&exe).current_dir(AC_INSTALL).spawn().expect("lancement acShowroom.exe");
    let pid = child.id();
    println!("acShowroom.exe lancé, PID {pid}. Attente de sa fenêtre...");

    let deadline = Instant::now() + Duration::from_secs(15);
    let target = loop {
        if let Some(h) = find_showroom_top_level(pid) {
            break h;
        }
        if Instant::now() >= deadline {
            eprintln!("Fenêtre du showroom introuvable après 15s, abandon.");
            restore_video_ini(&cfg_dir);
            return;
        }
        std::thread::sleep(Duration::from_millis(200));
    };
    println!("Fenêtre trouvée : {:?}", target.0);

    let parent = if use_overlay {
        let overlay_class = register_class(OVERLAY_CLASS, Some(plain_wndproc));
        let (ox, oy) = client_to_screen_origin(host);
        let overlay_title = wstr("overlay");
        let overlay = unsafe {
            CreateWindowExW(
                WS_EX_TOOLWINDOW,
                PCWSTR(overlay_class.as_ptr()),
                PCWSTR(overlay_title.as_ptr()),
                WS_POPUP | WS_VISIBLE,
                ox + 20,
                oy + 20,
                860,
                540,
                Some(host),
                None,
                None,
                None,
            )
        }
        .expect("création overlay");
        unsafe {
            let _ = ShowWindow(overlay, SW_SHOW);
        }
        println!("Overlay créé : {:?}", overlay.0);
        overlay
    } else {
        host
    };

    unsafe {
        let style = GetWindowLongPtrW(target, GWL_STYLE);
        // WS_CAPTION à lui seul retire la barre de titre ; SYSMENU/THICKFRAME/
        // MINIMIZEBOX/MAXIMIZEBOX retirent respectivement le menu système, la
        // bordure redimensionnable et les boutons — sans ça la fenêtre garde
        // sa propre chrome Windows complète une fois enfant (constaté à l'écran).
        let remove = WS_POPUP.0 | WS_CAPTION.0 | WS_THICKFRAME.0 | WS_SYSMENU.0 | WS_MINIMIZEBOX.0 | WS_MAXIMIZEBOX.0;
        let new_style = (style & !(remove as isize)) | (WS_CHILD.0 as isize);
        SetWindowLongPtrW(target, GWL_STYLE, new_style);
        SetParent(target, Some(parent)).expect("SetParent");
        let (w, h) = if use_overlay { (860, 540) } else { (860, 540) };
        let (x, y) = if use_overlay { (0, 0) } else { (20, 20) };
        SetWindowPos(target, None, x, y, w, h, SWP_NOZORDER | SWP_FRAMECHANGED).expect("SetWindowPos");
    }
    println!("Reparenté. Regarde la fenêtre \"Showroom Embed Test\".");

    // Test de l'hypothèse "perte de focus = fermeture" : on vole le focus au
    // profit de l'hôte, comme le ferait WebView2 en reprenant la main après
    // le clic sur le bouton dans l'app réelle.
    if args.iter().any(|a| a == "--steal-focus") {
        println!("Vol du focus vers l'hôte dans 1s...");
        std::thread::sleep(Duration::from_secs(1));
        unsafe {
            let _ = SetForegroundWindow(host);
        }
        println!("Focus volé. Le process showroom est-il toujours vivant ?");
    }

    // Boucle de messages : garde tout vivant (hôte + overlay + showroom) tant
    // que la fenêtre hôte n'est pas fermée.
    let mut msg = MSG::default();
    unsafe {
        while GetMessageW(&mut msg, None, 0, 0).as_bool() {
            let _ = TranslateMessage(&msg);
            DispatchMessageW(&msg);
        }
    }

    let _ = child.kill();
    restore_video_ini(&cfg_dir);
}
