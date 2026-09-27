//! Measurement benches on a real Assetto Corsa install, every one `#[ignore]`d:
//! instruments that answered a question once and are kept to answer it again,
//! not checks. Each says which environment variables it reads.

use super::*;

/// La question du volant : la pose de repos écarte les mains de 55 cm, ce
/// qui n'est pas une prise de volant. **Est-ce que la pose de la voiture
/// (hiérarchie + animation de braquage) les rapproche d'un cerceau
/// crédible ?**
///
/// ```text
/// PITBOX_AC_ROOT="D:\...\assettocorsa" PITBOX_CARS="D:\...\content\cars" cargo test --lib driver -- --ignored --nocapture what_the_car_pose
/// ```
#[test]
#[ignore = "needs a real Assetto Corsa install; measurement, not a check"]
fn what_the_car_pose_does_to_the_grip() {
    let (Ok(ac_root), Ok(cars)) = (std::env::var("PITBOX_AC_ROOT"), std::env::var("PITBOX_CARS")) else {
        eprintln!("PITBOX_AC_ROOT / PITBOX_CARS unset, skipping");
        return;
    };
    let root = PathBuf::from(ac_root);
    let mut checked = 0;
    for entry in std::fs::read_dir(PathBuf::from(cars)).expect("content/cars").flatten() {
        if checked >= 12 {
            break;
        }
        let car_dir = entry.path();
        if !car_dir.is_dir() {
            continue;
        }
        let car_id = entry.file_name().to_string_lossy().into_owned();
        let Some(graft) = resolve(&root, &car_dir, &car_id, None, 0.0, &OutfitOverride::default()) else {
            continue;
        };
        // Hôte vide : `graft` place le mannequin dans l'espace de la
        // voiture, ce qui est exactement ce qu'on veut mesurer.
        let mut host = kn5::Kn5Model {
            version: 6,
            extra: None,
            textures: Vec::new(),
            materials: Vec::new(),
            root: kn5::Kn5Node {
                name: "ROOT".into(),
                active: true,
                kind: kn5::Kn5NodeKind::Dummy {
                    transform: [
                        1.0, 0.0, 0.0, 0.0, 0.0, 1.0, 0.0, 0.0, 0.0, 0.0, 1.0, 0.0, 0.0, 0.0, 0.0, 1.0,
                    ],
                },
                children: Vec::new(),
            },
        };
        let _ = kn5_gltf::graft_driver(&mut host, &graft);
        let centers = kn5_gltf::node_world_centers(&host);
        let find = |bone: &str| {
            centers
                .iter()
                .find(|(n, _)| n.len() >= bone.len() && n[n.len() - bone.len()..].eq_ignore_ascii_case(bone))
                .map(|(_, c)| *c)
        };
        let (Some(l), Some(r)) = (find("RIG_HAND_L"), find("RIG_HAND_R")) else {
            continue;
        };
        let dist =
            |a: [f32; 3], b: [f32; 3]| ((a[0] - b[0]).powi(2) + (a[1] - b[1]).powi(2) + (a[2] - b[2]).powi(2)).sqrt();
        // Le poignet n'est pas le jonc : les doigts se referment devant la
        // paume. `HAND_Middle2` / `HAND_Middle5` sont les deuxièmes
        // phalanges des majeurs gauche et droit, au contact du cerceau.
        let (fl, fr) = (find("HAND_Middle2"), find("HAND_Middle5"));
        let offset = fl.zip(fr).map(|(a, b)| {
            let mid_w = [(l[0] + r[0]) / 2.0, (l[1] + r[1]) / 2.0, (l[2] + r[2]) / 2.0];
            let mid_f = [(a[0] + b[0]) / 2.0, (a[1] + b[1]) / 2.0, (a[2] + b[2]) / 2.0];
            [mid_f[0] - mid_w[0], mid_f[1] - mid_w[1], mid_f[2] - mid_w[2]]
        });
        eprintln!(
            "{car_id:38} poignets {:.3}  doigts {}  décalage {}",
            dist(l, r),
            fl.zip(fr)
                .map(|(a, b)| format!("{:.3}", dist(a, b)))
                .unwrap_or_else(|| "—".into()),
            offset
                .map(|o| format!("[{:+.3} {:+.3} {:+.3}]", o[0], o[1], o[2]))
                .unwrap_or_else(|| "—".into()),
        );
        checked += 1;
    }
}

/// Les cartes de normales d'un mannequin sont-elles vraiment en espace
/// **objet**, comme `nmObjectSpace = 1` et le suffixe `_OS` l'annoncent ?
///
/// La différence se voit dans la moyenne : une carte **tangente** est
/// quasi plate autour de (128, 128, 255) — la normale ne s'écarte guère de
/// la surface — alors qu'une carte **objet** encode la direction dans
/// l'espace du modèle, donc balaie tout le cube et s'étale.
///
/// ```text
/// PITBOX_AC_ROOT="D:\...\assettocorsa" cargo test --lib driver -- --ignored --nocapture what_normal_maps
/// ```
#[test]
#[ignore = "needs a real Assetto Corsa install; measurement, not a check"]
fn what_normal_maps_a_mannequin_carries() {
    let Ok(ac_root) = std::env::var("PITBOX_AC_ROOT") else {
        eprintln!("PITBOX_AC_ROOT unset, skipping");
        return;
    };
    let root = PathBuf::from(ac_root);
    let bytes = std::fs::read(body_file(&root, "driver")).expect("driver.kn5");
    let model = kn5::parse(&bytes).expect("parse");

    for material in &model.materials {
        let Some(normal) = material.texture_for("txNormal").filter(|n| !n.is_empty()) else {
            continue;
        };
        let flag = material.property("nmObjectSpace").unwrap_or(0.0);
        let Some(texture) = model.texture(normal) else { continue };
        match kn5_gltf::channel_stats(&texture.data) {
            Ok(stats) => eprintln!(
                "{:26} nmObjectSpace={flag}  {normal:24} moyenne RGB ({:.0} {:.0} {:.0})  écart-type ({:.0} {:.0} {:.0})",
                material.name,
                stats.mean[0],
                stats.mean[1],
                stats.mean[2],
                stats.stddev[0],
                stats.stddev[1],
                stats.stddev[2],
            ),
            Err(e) => eprintln!("{:26} {normal} illisible — {e}", material.name),
        }
    }
}

/// Ce que le `.glb` d'un mannequin garde comme noms : c'est ce qui décide
/// si le frontend peut retrouver la texture d'une pièce pour l'échanger
/// lui-même, au lieu de redemander une conversion à chaque survol.
///
/// ```text
/// PITBOX_AC_ROOT="D:\...\assettocorsa" cargo test --lib driver -- --ignored --nocapture what_the_glb
/// ```
#[test]
#[ignore = "needs a real Assetto Corsa install; measurement, not a check"]
fn what_the_glb_keeps_of_the_names() {
    let Ok(ac_root) = std::env::var("PITBOX_AC_ROOT") else {
        eprintln!("PITBOX_AC_ROOT unset, skipping");
        return;
    };
    let root = PathBuf::from(ac_root);
    let graft = kn5_gltf::DriverGraft {
        model: body_file(&root, "driver"),
        anchor: None,
        texture_dirs: Vec::new(),
        base_pose: None,
        animation: None,
        lock_degrees: 360.0,
        steer_degrees: 0.0,
    };
    let (model, stats, rig) = kn5_gltf::standalone_driver(&graft).expect("mannequin converti");
    eprintln!("rig: {rig:?}");
    eprintln!("stats: {} triangles, {} habillées", stats.triangles, stats.dressed);

    let conversion =
        kn5_gltf::convert(&model, None, &kn5_gltf::ConvertOptions::default(), &|_| {}).expect("conversion");
    // Chunk JSON d'un GLB : en-tête de 12 octets, puis longueur + type.
    let glb = conversion.to_glb();
    let len = u32::from_le_bytes(glb[12..16].try_into().unwrap()) as usize;
    let json: serde_json::Value = serde_json::from_slice(&glb[20..20 + len]).expect("json");
    for key in ["images", "textures", "materials"] {
        let names: Vec<String> = json[key]
            .as_array()
            .map(|a| {
                a.iter()
                    .map(|v| v["name"].as_str().unwrap_or("<sans nom>").to_string())
                    .collect()
            })
            .unwrap_or_default();
        eprintln!("{key} ({}) : {}", names.len(), names.join(", "));
    }
}

/// Où un mannequin tient ses mains, sa tête et ses pieds dans sa **pose de
/// repos** — celle qu'il a sans voiture autour de lui, donc celle du
/// plateau d'essayage (PILOTE§5.1).
///
/// La question à laquelle ce test répond : peut-on poser un volant
/// générique à un endroit fixe, ou faut-il le calculer par mannequin ?
///
/// ```text
/// PITBOX_AC_ROOT="D:\...\assettocorsa" cargo test --lib driver -- --ignored --nocapture where_the_hands
/// ```
#[test]
#[ignore = "needs a real Assetto Corsa install; measurement, not a check"]
fn where_the_hands_rest() {
    let Ok(ac_root) = std::env::var("PITBOX_AC_ROOT") else {
        eprintln!("PITBOX_AC_ROOT unset, skipping");
        return;
    };
    let root = PathBuf::from(ac_root);
    let wanted = ["RIG_HAND_L", "RIG_HAND_R", "RIG_Head", "RIG_Hips"];
    for body in bodies(&root).bodies {
        let Ok(bytes) = std::fs::read(body_file(&root, &body.id)) else {
            continue;
        };
        let Ok(model) = kn5::parse(&bytes) else { continue };
        let centers = kn5_gltf::node_world_centers(&model);
        let mut line = format!("{:32}", body.id);
        for name in wanted {
            let found = centers
                .iter()
                .find(|(n, _)| n.len() >= name.len() && n[n.len() - name.len()..].eq_ignore_ascii_case(name));
            match found {
                Some((_, c)) => line.push_str(&format!(" {name}[{:+.3} {:+.3} {:+.3}]", c[0], c[1], c[2])),
                None => line.push_str(&format!(" {name}[—]")),
            }
        }
        eprintln!("{line}");
    }
}

/// Where the whole pipeline actually puts each driver, against what the car
/// says with `DRIVEREYES`.
///
/// The instrument that matters, and the one the crate-level test cannot
/// be: only the application resolves the real mannequin (`driver3d.ini`
/// lives in the encrypted container) and the real anchor. A driver placed
/// by a metre reads here as a metre, before anyone has to notice it on
/// screen.
///
/// The residual is expected to be the eye-above-bone offset — a few
/// centimetres up and forward, near zero sideways. Anything past 15 cm on
/// any axis is a car to go and look at.
///
/// ```text
/// PITBOX_AC_ROOT="D:\...\assettocorsa" cargo test --lib driver -- --ignored --nocapture where_every
/// ```
#[test]
#[ignore = "needs a real Assetto Corsa install; measurement, not a check"]
fn where_every_installed_driver_lands() {
    let Ok(ac_root) = std::env::var("PITBOX_AC_ROOT") else {
        eprintln!("PITBOX_AC_ROOT unset, skipping");
        return;
    };
    let root = PathBuf::from(ac_root);
    let mut checked = 0usize;
    let mut off: Vec<(f32, String, [f32; 3], [f32; 3])> = Vec::new();
    let mut residuals: Vec<[f32; 3]> = Vec::new();

    for entry in std::fs::read_dir(root.join("content").join("cars"))
        .expect("read content/cars")
        .flatten()
    {
        let car_dir = entry.path();
        let car_id = car_dir.file_name().unwrap_or_default().to_string_lossy().into_owned();
        let Some(outfit) = outfit_of(&car_dir, &car_id, None) else {
            continue;
        };
        let Some(eyes) = outfit.eyes else { continue };
        let Some(wanted) = graft_for(&root, &car_dir, &outfit, 0.0) else {
            continue;
        };

        // Grafted into an empty car, so what comes out is the driver alone,
        // already in the car's own space — offset, hierarchy and all.
        let mut host = kn5::Kn5Model {
            version: 6,
            extra: None,
            textures: Vec::new(),
            materials: Vec::new(),
            root: kn5::Kn5Node {
                name: "root".to_string(),
                active: true,
                kind: kn5::Kn5NodeKind::Dummy {
                    transform: [
                        1.0, 0.0, 0.0, 0.0, //
                        0.0, 1.0, 0.0, 0.0, //
                        0.0, 0.0, 1.0, 0.0, //
                        0.0, 0.0, 0.0, 1.0,
                    ],
                },
                children: Vec::new(),
            },
        };
        let stats = kn5_gltf::graft_driver(&mut host, &wanted);
        let Some(head) = kn5_gltf::node_world_centers(&host)
            .into_iter()
            .find(|(name, _)| name.to_lowercase().ends_with("rig_head"))
            .map(|(_, c)| c)
        else {
            continue;
        };

        checked += 1;
        let residual = [eyes[0] - head[0], eyes[1] - head[1], eyes[2] - head[2]];
        residuals.push(residual);
        let worst = residual.iter().fold(0.0f32, |a, v| a.max(v.abs()));
        if worst > 0.15 {
            off.push((
                worst,
                format!(
                    "{car_id} [{}{}]",
                    if stats.seated.is_some() { "knh" } else { "eyes" },
                    if stats.posed.is_some() { "+anim" } else { "" }
                ),
                head,
                eyes,
            ));
        }
    }

    for (axis, label) in [(0, "x"), (1, "y"), (2, "z")] {
        let mut values: Vec<f32> = residuals.iter().map(|r| r[axis]).collect();
        values.sort_by(|a, b| a.total_cmp(b));
        let n = values.len();
        eprintln!(
            "  residual {label}: min {:+.3}  p10 {:+.3}  median {:+.3}  p90 {:+.3}  max {:+.3}",
            values[0],
            values[n / 10],
            values[n / 2],
            values[9 * n / 10],
            values[n - 1]
        );
    }
    off.sort_by(|a, b| b.0.total_cmp(&a.0));
    eprintln!("\n=== {checked} drivers placed, {} past 15 cm ===", off.len());
    for (worst, who, head, eyes) in off.iter().take(30) {
        eprintln!(
            "  {worst:.2} m  {who:52} head {:?} eyes {:?}",
            head.map(|v| (v * 1000.0).round() / 1000.0),
            eyes.map(|v| (v * 1000.0).round() / 1000.0)
        );
    }
}

/// Convertit une voiture **avec son pilote** et écrit le `.glb`, pour
/// pouvoir le regarder.
///
/// C'est l'instrument qui manquait : `kn5-tool` ne sait pas greffer de
/// pilote — la résolution passe par `data.acd`, que le crate de conversion
/// ne lit délibérément pas — donc rien ne permettait d'inspecter un
/// mannequin exporté en squelette sans lancer l'application entière.
///
/// ```text
/// PITBOX_AC_ROOT=... PITBOX_CAR_DIR=... PITBOX_GLB_OUT=...
///   cargo test --lib driver -- --ignored --nocapture convert_one_car_with_its_driver
/// ```
#[test]
#[ignore = "needs a real Assetto Corsa install; instrument, not a check"]
fn convert_one_car_with_its_driver() {
    let (Ok(ac_root), Ok(car_dir), Ok(out)) = (
        std::env::var("PITBOX_AC_ROOT"),
        std::env::var("PITBOX_CAR_DIR"),
        std::env::var("PITBOX_GLB_OUT"),
    ) else {
        eprintln!("PITBOX_AC_ROOT / PITBOX_CAR_DIR / PITBOX_GLB_OUT unset, skipping");
        return;
    };
    let car_dir = std::path::PathBuf::from(car_dir);
    let car_id = car_dir.file_name().unwrap_or_default().to_string_lossy().into_owned();
    let skin = kn5_gltf::resolve_skin(&car_dir, None);
    let graft = resolve(
        std::path::Path::new(&ac_root),
        &car_dir,
        &car_id,
        skin.as_deref(),
        0.0,
        &OutfitOverride::default(),
    )
    .expect("cette voiture nomme un pilote installé");

    let resolved = kn5_gltf::resolve_model(&car_dir).expect("un modèle");
    let bytes = std::fs::read(&resolved.path).expect("lire le modèle");
    let mut model = kn5::parse(&bytes).expect("parser le modèle");
    let stats = kn5_gltf::graft_driver(&mut model, &graft);
    eprintln!(
        "pilote : {} triangles, {} texture(s) habillée(s), {:?} assis, {:?} posés",
        stats.triangles, stats.dressed, stats.seated, stats.posed
    );
    for failure in &stats.failures {
        eprintln!("  échec : {failure}");
    }

    let steering = crate::steering::read(&car_dir, &car_id);
    let animation = graft.animation.as_ref().and_then(|path| {
        let bytes = std::fs::read(path).ok()?;
        kn5::parse_animation(&bytes).ok()
    });
    eprintln!(
        "animation : {} nœud(s), {} image(s), lock {}°, ratio {}",
        animation.as_ref().map(|a| a.nodes.len()).unwrap_or(0),
        animation.as_ref().map(|a| a.frame_count()).unwrap_or(0),
        graft.lock_degrees,
        steering.ratio
    );
    let options = kn5_gltf::ConvertOptions {
        geometry: kn5_gltf::GeometryOptions {
            steering: kn5_gltf::SteerLimits {
                lock: steering.lock,
                ratio: steering.ratio,
            },
            ..Default::default()
        },
        driver_rig: animation.map(|animation| kn5_gltf::DriverRigSource {
            animation,
            lock_degrees: graft.lock_degrees,
        }),
        ..Default::default()
    };
    let conversion = kn5_gltf::convert(&model, skin.as_deref(), &options, &|_| {}).expect("convertir");
    let glb = conversion.to_glb();
    eprintln!(
        "converti : {} triangles, {} matériaux, {} textures, {:.1} Mo",
        conversion.triangle_count,
        conversion.material_count,
        conversion.texture_count,
        glb.len() as f32 / (1024.0 * 1024.0)
    );
    std::fs::write(&out, &glb).expect("écrire le glb");
    eprintln!("écrit dans {out}");
}
