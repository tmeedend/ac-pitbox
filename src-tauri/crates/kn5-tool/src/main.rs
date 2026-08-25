//! Developer CLI for the KN5 pipeline — never shipped to users (spec §5.1).
//!
//! Exists so that every lot before the viewer can be verified without
//! launching the application: `inspect` proves the parser against one file,
//! `scan` proves it against a whole `content/cars` folder.
//!
//! ```text
//! kn5-tool inspect <file.kn5 | car_dir> [--tree] [--materials] [--textures]
//! kn5-tool scan <dir> [--details]
//! kn5-tool maps <dir | car_dir>
//! ```

mod report;

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::process::ExitCode;
use std::time::Instant;

use kn5::{Kn5Model, Kn5NodeKind};
use kn5_gltf::{prepare_textures, resolve_model, resolve_skin, ModelSource, TextureOptions, TextureOrigin};

use report::{by_count, human_bytes, Stats};

const USAGE: &str = "\
kn5-tool — inspect Assetto Corsa KN5 models

USAGE:
    kn5-tool inspect <file.kn5 | car_dir> [--tree] [--materials] [--textures] [--skin=<id>]
    kn5-tool scan <dir> [--details]
    kn5-tool extract-textures <car_dir> --out=<dir> [--skin=<id>]
    kn5-tool convert <car_dir> --out=<file.glb> [--skin=<id>]
    kn5-tool maps <dir | car_dir>

COMMANDS:
    inspect            Parse one model and report what it contains.
    scan               Parse every car of a folder (e.g. content/cars) and aggregate.
    extract-textures   Decode, resize and re-encode the textures to a folder.
    convert            Write the whole car as a self-contained glTF binary.
    maps               Measure every `txMaps` texture, channel by channel (CSV
                       on stdout). Investigation instrument for the undocumented
                       R and B channels — docs/kn5-format.md, écart n°7.

OPTIONS:
    --tree        Print the node hierarchy.
    --materials   Print every material with its shader, properties and samplers.
    --textures    Print every embedded texture with its sniffed format.
    --details     scan: also print the aggregate shader / property tables.
    --out=<path>  extract-textures: destination folder. convert: .glb file.
    --skin=<id>   extract-textures: skin whose files override the embedded ones
                  (default: first skin in alphabetical order).
";

/// Minimal stderr logger. The parser reports its suspicions through the `log`
/// facade (unknown version, trailing bytes after the node tree) and those are
/// exactly the signals worth seeing while validating the format — inside the
/// application they land in the log file instead, via `tauri-plugin-log`.
struct StderrLogger;

impl log::Log for StderrLogger {
    fn enabled(&self, _: &log::Metadata) -> bool {
        true
    }

    fn log(&self, record: &log::Record) {
        eprintln!("[{}] {}", record.level(), record.args());
    }

    fn flush(&self) {}
}

static LOGGER: StderrLogger = StderrLogger;

fn main() -> ExitCode {
    let _ = log::set_logger(&LOGGER).map(|()| log::set_max_level(log::LevelFilter::Warn));

    let args: Vec<String> = std::env::args().skip(1).collect();
    let Some(command) = args.first() else {
        eprint!("{USAGE}");
        return ExitCode::FAILURE;
    };

    let flags: Vec<&str> = args
        .iter()
        .skip(1)
        .filter(|a| a.starts_with("--"))
        .map(|a| a.as_str())
        .collect();
    let positional: Vec<&str> = args
        .iter()
        .skip(1)
        .filter(|a| !a.starts_with("--"))
        .map(|a| a.as_str())
        .collect();

    let result = match command.as_str() {
        "inspect" => match positional.first() {
            Some(path) => inspect(Path::new(path), &flags),
            None => Err("inspect needs a path".to_string()),
        },
        "scan" => match positional.first() {
            Some(path) => scan(Path::new(path), &flags),
            None => Err("scan needs a path".to_string()),
        },
        "convert" => match positional.first() {
            Some(path) => convert(Path::new(path), &flags),
            None => Err("convert needs a car folder".to_string()),
        },
        "maps" => match positional.first() {
            Some(path) => maps(Path::new(path)),
            None => Err("maps needs a folder of cars, or one car folder".to_string()),
        },
        "extract-textures" => match positional.first() {
            Some(path) => extract_textures(Path::new(path), &flags),
            None => Err("extract-textures needs a car folder".to_string()),
        },
        "-h" | "--help" | "help" => {
            print!("{USAGE}");
            return ExitCode::SUCCESS;
        }
        other => Err(format!("unknown command `{other}`")),
    };

    match result {
        Ok(()) => ExitCode::SUCCESS,
        Err(message) => {
            eprintln!("error: {message}");
            ExitCode::FAILURE
        }
    }
}

/// Accepts either a `.kn5` directly or a car folder, in which case the model
/// is resolved the way the application will resolve it (§4.2).
fn model_path(path: &Path) -> Result<(PathBuf, Option<ModelSource>), String> {
    if path.is_file() {
        return Ok((path.to_path_buf(), None));
    }
    if path.is_dir() {
        return match resolve_model(path) {
            Some(resolved) => Ok((resolved.path, Some(resolved.source))),
            None => Err(format!("no model found in {}", path.display())),
        };
    }
    Err(format!("{} does not exist", path.display()))
}

fn inspect(path: &Path, flags: &[&str]) -> Result<(), String> {
    let (file, source) = model_path(path)?;
    let bytes = std::fs::read(&file).map_err(|e| format!("{}: {e}", file.display()))?;
    let mut model = kn5::parse(&bytes).map_err(|e| format!("{}: {e}", file.display()))?;

    // Only when a whole car folder was named: `ext_config.ini` is relative to
    // it, and inspecting a lone KN5 must keep showing that file untouched —
    // that is the point of having an inspectable intermediate.
    if path.is_dir() {
        let skin = resolve_skin(path, option_value(flags, "--skin"));
        let ext = kn5_gltf::apply_ext_config(&mut model, path, &file, skin.as_deref());
        report_ext_config(&ext);
    }

    let mut stats = Stats::default();
    stats.add(&model);

    println!("file      {}", file.display());
    if let Some(source) = source {
        println!(
            "resolved  {}",
            match source {
                ModelSource::LodsIni => "data/lods.ini [LOD_0]",
                ModelSource::Heuristic => "heuristic (largest non-LOD kn5)",
            }
        );
    }
    println!("size      {}", human_bytes(bytes.len() as u64));
    println!(
        "version   {}{}",
        model.version,
        match model.extra {
            Some(extra) => format!(" (extra header word: {extra})"),
            None => String::new(),
        }
    );
    println!(
        "textures  {} ({}) {}{}",
        stats.textures,
        human_bytes(stats.texture_bytes),
        stats
            .texture_formats
            .iter()
            .map(|(format, count)| format!("{format}:{count}"))
            .collect::<Vec<_>>()
            .join(" "),
        match stats.texture_placeholders {
            0 => String::new(),
            n => format!(" + {n} empty (type 0)"),
        }
    );
    println!("materials {}", stats.materials);
    println!(
        "nodes     {} ({} dummy, {} mesh, {} skinned)",
        stats.nodes, stats.dummies, stats.meshes, stats.skinned_meshes
    );
    println!(
        "geometry  {} triangles, {} vertices, {} mesh(es) flagged non-renderable",
        stats.triangles, stats.vertices, stats.not_renderable
    );
    if let Some(size) = stats.size() {
        println!(
            "bounds    {:.2} x {:.2} x {:.2} m (raw vertices, node transforms not applied)",
            size[0], size[1], size[2]
        );
    }

    println!("\nshaders");
    for (shader, count) in by_count(&stats.shaders) {
        println!("  {count:>4}  {shader}");
    }

    println!("\nblend modes");
    for ((mode, tested), count) in &stats.blend_modes {
        println!(
            "  {count:>4}  blend {mode}{}",
            if *tested { ", alpha tested" } else { "" }
        );
    }

    println!("\nmesh flags: cast_shadows / is_visible / is_transparent");
    for ((cast, visible, transparent), count) in &stats.mesh_flags {
        println!(
            "  {count:>4}  {} {} {}",
            u8::from(*cast),
            u8::from(*visible),
            u8::from(*transparent)
        );
    }

    if !stats.transparent_mesh_shaders.is_empty() {
        println!("\nshaders of the meshes flagged transparent");
        for (shader, count) in by_count(&stats.transparent_mesh_shaders) {
            println!("  {count:>4}  {shader}");
        }
    }

    if flags.contains(&"--textures") {
        println!("\ntextures");
        for texture in &model.textures {
            println!(
                "  {:>10}  {:<6} kind={} {}",
                human_bytes(texture.data.len() as u64),
                kn5::ImageFormat::sniff(&texture.data).as_str(),
                texture.kind,
                texture.name
            );
        }
    }

    if flags.contains(&"--materials") {
        println!("\nmaterials");
        for (index, material) in model.materials.iter().enumerate() {
            println!(
                "  [{index}] {} — shader {} (blend {}{})",
                material.name,
                material.shader,
                material.blend_mode,
                if material.alpha_tested { ", alpha tested" } else { "" }
            );
            for property in &material.properties {
                let extra = if property.extra.iter().any(|v| *v != 0.0) {
                    format!("  extra {:?}", property.extra)
                } else {
                    String::new()
                };
                println!("        {} = {}{}", property.name, property.value, extra);
            }
            for sampler in &material.samplers {
                println!(
                    "        {} [slot {}] -> {}",
                    sampler.name, sampler.slot, sampler.texture
                );
            }
        }
    }

    if flags.contains(&"--tree") {
        println!("\nnodes");
        print_tree(&model.root, 0, &model);
    }

    Ok(())
}

fn print_tree(node: &kn5::Kn5Node, depth: usize, model: &Kn5Model) {
    let indent = "  ".repeat(depth + 1);
    match &node.kind {
        Kn5NodeKind::Dummy { .. } => {
            println!("{indent}[dummy] {}", node.name);
        }
        Kn5NodeKind::Mesh(mesh) | Kn5NodeKind::SkinnedMesh(kn5::Kn5SkinnedMesh { mesh, .. }) => {
            let skinned = matches!(node.kind, Kn5NodeKind::SkinnedMesh(_));
            let material = model
                .materials
                .get(mesh.material_id as usize)
                .map(|m| m.name.as_str())
                .unwrap_or("?");
            println!(
                "{indent}[{}] {} — {} tris, material {} {}",
                if skinned { "skin" } else { "mesh" },
                node.name,
                mesh.indices.len() / 3,
                material,
                if mesh.is_renderable { "" } else { "(not renderable)" }
            );
        }
    }
    for child in &node.children {
        print_tree(child, depth + 1, model);
    }
}

fn scan(dir: &Path, flags: &[&str]) -> Result<(), String> {
    let entries = std::fs::read_dir(dir).map_err(|e| format!("{}: {e}", dir.display()))?;
    let mut car_dirs: Vec<PathBuf> = entries
        .flatten()
        .map(|entry| entry.path())
        .filter(|path| path.is_dir())
        .collect();
    car_dirs.sort();

    let mut stats = Stats::default();
    let mut failures: Vec<(String, String)> = Vec::new();
    let mut skipped: Vec<String> = Vec::new();
    let mut sources: BTreeMap<&'static str, usize> = BTreeMap::new();
    let mut ok = 0usize;

    for car_dir in &car_dirs {
        let name = car_dir.file_name().and_then(|n| n.to_str()).unwrap_or("?").to_string();
        let Some(resolved) = resolve_model(car_dir) else {
            skipped.push(name);
            continue;
        };
        *sources
            .entry(match resolved.source {
                ModelSource::LodsIni => "lods.ini",
                ModelSource::Heuristic => "heuristic",
            })
            .or_default() += 1;

        let bytes = match std::fs::read(&resolved.path) {
            Ok(bytes) => bytes,
            Err(e) => {
                failures.push((name, e.to_string()));
                continue;
            }
        };
        match kn5::parse(&bytes) {
            Ok(model) => {
                ok += 1;
                println!(
                    "ok    {name:<40} {:>10}  {:>7} tris  {:>3} mats  {:>3} tex",
                    human_bytes(bytes.len() as u64),
                    model.triangle_count(),
                    model.materials.len(),
                    model.textures.len()
                );
                stats.add(&model);
            }
            Err(e) => {
                println!("FAIL  {name:<40} {e}");
                failures.push((name, e.to_string()));
            }
        }
    }

    println!(
        "\n=== {} folder(s): {ok} parsed, {} failed, {} without a model",
        car_dirs.len(),
        failures.len(),
        skipped.len()
    );
    println!(
        "model resolution: {}",
        sources
            .iter()
            .map(|(source, count)| format!("{source} {count}"))
            .collect::<Vec<_>>()
            .join(", ")
    );
    println!(
        "versions: {}",
        stats
            .versions
            .iter()
            .map(|(version, count)| format!("v{version} x{count}"))
            .collect::<Vec<_>>()
            .join(", ")
    );
    println!(
        "totals: {} triangles, {} materials, {} textures ({}) + {} empty (type 0)",
        stats.triangles,
        stats.materials,
        stats.textures,
        human_bytes(stats.texture_bytes),
        stats.texture_placeholders
    );
    println!(
        "textures whose real container contradicts their extension: {} of {}",
        stats.texture_extension_mismatch, stats.textures
    );
    println!(
        "texture containers: {}",
        stats
            .texture_formats
            .iter()
            .map(|(format, count)| format!("{format} {count}"))
            .collect::<Vec<_>>()
            .join(", ")
    );
    println!(
        "blend modes: {}",
        stats
            .blend_modes
            .iter()
            .map(|((mode, tested), count)| format!("blend {mode}{} x{count}", if *tested { "+tested" } else { "" }))
            .collect::<Vec<_>>()
            .join(", ")
    );
    println!("mesh flag combinations, cast/visible/transparent:");
    for ((cast, visible, transparent), count) in &stats.mesh_flags {
        println!(
            "  {} {} {}  x{count}",
            u8::from(*cast),
            u8::from(*visible),
            u8::from(*transparent)
        );
    }
    println!(
        "material property extra bytes non-zero (§12 q5): {} of {} properties",
        stats.properties_with_extra,
        stats.property_names.values().sum::<usize>()
    );
    println!("shaders of the meshes flagged transparent (§12 q1):");
    for (shader, count) in by_count(&stats.transparent_mesh_shaders) {
        println!("  {count:>5}  {shader}");
    }

    if flags.contains(&"--details") {
        println!("\nshaders across the library");
        for (shader, count) in by_count(&stats.shaders) {
            println!("  {count:>5}  {shader}");
        }
        println!("\nmaterial properties");
        for (property, count) in by_count(&stats.property_names) {
            println!("  {count:>5}  {property}");
        }
        println!("\ntexture samplers");
        for (sampler, count) in by_count(&stats.sampler_names) {
            println!("  {count:>5}  {sampler}");
        }
    }

    if !failures.is_empty() {
        println!("\nfailures");
        for (name, error) in &failures {
            println!("  {name}: {error}");
        }
    }

    Ok(())
}

/// Measures every texture bound to a `txMaps` sampler, one CSV row per
/// (car, material) pair, on stdout.
///
/// Written for one question, and it is worth stating it: the green channel of
/// `txMaps` is documented as the gloss (docs/kn5-format.md, écart n°7), R and
/// B are not. `metallicFactor` is held at zero until they are, because §6.2
/// of the spec asks for a plausible result rather than a guessed one — so the
/// chrome, the rims and the bare metal of every car currently render as
/// glossy paint.
///
/// Everything a hypothesis might need is on the row: the surface is named by
/// the material, the shader says which family it belongs to, `ksSpecular*`
/// says what the author declared, and `also_diffuse` flags the first
/// guard-rail of that same écart — a `txMaps` pointing at a colour texture,
/// whose green is a green and not a gloss.
fn maps(path: &Path) -> Result<(), String> {
    let mut car_dirs: Vec<PathBuf> = if resolve_model(path).is_some() {
        vec![path.to_path_buf()]
    } else {
        let entries = std::fs::read_dir(path).map_err(|e| format!("{}: {e}", path.display()))?;
        let mut dirs: Vec<PathBuf> = entries
            .flatten()
            .map(|entry| entry.path())
            .filter(|path| path.is_dir())
            .collect();
        dirs.sort();
        dirs
    };
    // Bibliothèque Pit Box : chaque voiture y porte un dossier de version
    // intermédiaire, alors que `content/cars` est plat. On descend d'un cran
    // quand le dossier lui-même ne contient aucun modèle.
    car_dirs = car_dirs
        .into_iter()
        .flat_map(|dir| match resolve_model(&dir) {
            Some(_) => vec![dir],
            None => std::fs::read_dir(&dir)
                .map(|entries| {
                    entries
                        .flatten()
                        .map(|entry| entry.path())
                        .filter(|path| resolve_model(path).is_some())
                        .collect::<Vec<_>>()
                })
                .unwrap_or_default(),
        })
        .collect();

    println!(
        "car,material,shader,texture,width,height,r_mean,g_mean,b_mean,r_std,g_std,b_std,r_min,r_max,b_min,b_max,rb_equal,white,corr_rg,corr_bg,corr_rb,ks_specular,ks_specular_exp,ks_ambient,ks_diffuse,fresnel_c,fresnel_exp,fresnel_max,also_diffuse"
    );

    for car_dir in &car_dirs {
        let car = car_dir.file_name().and_then(|n| n.to_str()).unwrap_or("?").to_string();
        let Some(resolved) = resolve_model(car_dir) else {
            continue;
        };
        let Ok(bytes) = std::fs::read(&resolved.path) else {
            continue;
        };
        let Ok(model) = kn5::parse(&bytes) else {
            eprintln!("{car}: unreadable model, skipped");
            continue;
        };

        // Noms liés à un `txDiffuse` quelque part dans le modèle : c'est le
        // garde-fou n°1, et il se juge à l'échelle du modèle entier, pas du
        // matériau courant.
        let diffuse: std::collections::BTreeSet<&str> = model
            .materials
            .iter()
            .filter_map(|material| material.texture_for("txDiffuse"))
            .collect();

        for material in &model.materials {
            let Some(name) = material.texture_for("txMaps") else {
                continue;
            };
            if name.is_empty() {
                continue;
            }
            let Some(texture) = model.textures.iter().find(|t| t.name == name) else {
                // Nom non embarqué : surcharge de skin, mesurée ailleurs.
                eprintln!("{car}: {name} not embedded, skipped");
                continue;
            };
            let stats = match kn5_gltf::channel_stats(&texture.data) {
                Ok(stats) => stats,
                Err(e) => {
                    eprintln!("{car}: {name} unreadable — {e}");
                    continue;
                }
            };
            let property = |key: &str| material.property(key).map(|v| format!("{v}")).unwrap_or_default();
            println!(
                "{car},{},{},{name},{},{},{:.1},{:.1},{:.1},{:.1},{:.1},{:.1},{},{},{},{},{:.3},{:.3},{:.3},{:.3},{:.3},{},{},{},{},{},{},{},{}",
                material.name,
                material.shader,
                stats.width,
                stats.height,
                stats.mean[0],
                stats.mean[1],
                stats.mean[2],
                stats.stddev[0],
                stats.stddev[1],
                stats.stddev[2],
                stats.min[0],
                stats.max[0],
                stats.min[2],
                stats.max[2],
                stats.rb_equal,
                stats.white,
                stats.corr_rg,
                stats.corr_bg,
                stats.corr_rb,
                property("ksSpecular"),
                property("ksSpecularEXP"),
                property("ksAmbient"),
                property("ksDiffuse"),
                property("fresnelC"),
                property("fresnelEXP"),
                property("fresnelMaxLevel"),
                diffuse.contains(name),
            );
        }
    }
    Ok(())
}

/// Value of a `--key=value` flag. Keeping option syntax to a single token
/// avoids a stateful argument parser for what is a two-option tool.
fn option_value<'a>(flags: &[&'a str], key: &str) -> Option<&'a str> {
    let prefix = format!("{key}=");
    flags.iter().find_map(|flag| flag.strip_prefix(&prefix))
}

/// Texture names come from an untrusted file and are written to disk here, so
/// anything that could escape the destination folder is flattened first.
fn safe_file_name(name: &str) -> String {
    name.chars()
        .map(|c| {
            if c.is_ascii_alphanumeric() || "-_. ".contains(c) {
                c
            } else {
                '_'
            }
        })
        .collect()
}

fn extract_textures(car_dir: &Path, flags: &[&str]) -> Result<(), String> {
    let out = option_value(flags, "--out").ok_or("extract-textures needs --out=<dir>")?;
    let out = Path::new(out);
    let (file, _) = model_path(car_dir)?;
    let skin = resolve_skin(car_dir, option_value(flags, "--skin"));

    let bytes = std::fs::read(&file).map_err(|e| format!("{}: {e}", file.display()))?;
    let started = Instant::now();
    let model = kn5::parse(&bytes).map_err(|e| format!("{}: {e}", file.display()))?;
    let parsed = started.elapsed();

    let started = Instant::now();
    let set = prepare_textures(&model, skin.as_deref(), &TextureOptions::default());
    let prepared = started.elapsed();

    std::fs::create_dir_all(out).map_err(|e| format!("{}: {e}", out.display()))?;
    for texture in &set.textures {
        let extension = if texture.mime == "image/jpeg" { "jpg" } else { "png" };
        // The source name keeps its own extension in the output file name: it
        // is the key materials refer to, and a `.dds` that is really a PNG
        // would otherwise be impossible to trace back.
        let path = out.join(format!("{}.{extension}", safe_file_name(&texture.name)));
        std::fs::write(&path, &texture.bytes).map_err(|e| format!("{}: {e}", path.display()))?;
    }

    if flags.contains(&"--alpha-stats") {
        // Diagnostic : AC se sert parfois du canal alpha d'une texture diffuse
        // pour autre chose que la transparence. Le savoir change la façon de
        // l'encoder — un alpha nul sur des pixels dont le RVB est plein est le
        // signe qu'il ne faut surtout pas le conserver.
        println!(
            "
alpha des textures de couleur"
        );
        for texture in &set.textures {
            if texture.role != kn5_gltf::TextureRole::Color {
                continue;
            }
            let Some((zero, total, rgb)) = kn5_gltf::alpha_stats(texture) else {
                continue;
            };
            if zero == 0 {
                continue;
            }
            println!(
                "  {:<34} {:>5.1} % à alpha=0, RVB moyen sous ces pixels = {rgb:?}",
                texture.name,
                100.0 * zero as f64 / total as f64
            );
        }
    }

    println!("model     {}", file.display());
    println!(
        "skin      {}",
        skin.as_ref()
            .map(|p| p.display().to_string())
            .unwrap_or_else(|| "none (embedded textures only)".to_string())
    );
    println!("output    {}", out.display());
    println!(
        "textures  {} written, {} unreferenced left alone, {} warning(s)",
        set.textures.len(),
        set.unreferenced.len(),
        set.warnings.len()
    );
    let overridden = set
        .textures
        .iter()
        .filter(|t| matches!(t.origin, TextureOrigin::Skin(_)))
        .count();
    println!("          {overridden} taken from the skin folder");
    println!(
        "size      {} in, {} out ({:.0} %)",
        human_bytes(set.source_bytes() as u64),
        human_bytes(set.total_bytes() as u64),
        100.0 * set.total_bytes() as f64 / set.source_bytes().max(1) as f64
    );
    println!(
        "time      {:.0} ms parsing, {:.0} ms transcoding",
        parsed.as_secs_f64() * 1000.0,
        prepared.as_secs_f64() * 1000.0
    );

    if !set.warnings.is_empty() {
        println!("\nwarnings");
        for warning in &set.warnings {
            println!("  {}: {}", warning.name, warning.reason);
        }
    }

    Ok(())
}

/// Regression guard for the coordinate conversion (§4.4, §12 q4).
///
/// **Calibré sur un rendu, pas déduit** — et recalibré une fois, ce qui est
/// tout l'intérêt de l'avoir. La référence est `abarth500` : la conversion
/// actuelle y affiche `ABARTH` de gauche à droite sur le bas de caisse. Ce
/// contrôle enregistre la géométrie des roues de cet état vérifié, pour qu'un
/// changement de conversion se signale ici au lieu de mettre en miroir toutes
/// les voitures en silence.
///
/// La première calibration s'appuyait sur `ks_mazda_mx5_cup` et s'est révélée
/// fausse : l'atlas de cette voiture range ses deux flancs côte à côte et
/// presque identiques, et son îlot UV est tourné à 90°, si bien qu'un modèle
/// en miroir échantillonnant le mauvais flanc y paraissait juste. Une voiture
/// dont l'atlas est symétrique ne prouve rien.
fn orientation_verdict(model: &kn5::Kn5Model) -> Option<String> {
    let centers = kn5_gltf::node_world_centers(model);
    let find = |suffix: &str| -> Option<[f32; 3]> {
        centers
            .iter()
            .find(|(name, _)| name.to_ascii_uppercase() == format!("WHEEL_{suffix}"))
            .map(|(_, center)| *center)
    };
    let (lf, rf, lr, rr) = (find("LF")?, find("RF")?, find("LR")?, find("RR")?);

    let front_z = (lf[2] + rf[2]) / 2.0;
    let rear_z = (lr[2] + rr[2]) / 2.0;
    let forward = (front_z - rear_z).signum();
    // État vérifié : `WHEEL_LF` se retrouve du côté du nez, `WHEEL_RF` en face.
    let ok = (lf[0] > 0.0) == (forward > 0.0) && (rf[0] > 0.0) == (forward < 0.0);

    Some(format!(
        "{} — nose at z={front_z:+.2}, tail at z={rear_z:+.2}, WHEEL_LF at x={:+.2}, WHEEL_RF at x={:+.2}",
        if ok {
            "matches the verified render"
        } else {
            "CHANGED — re-check the livery text before trusting this build"
        },
        lf[0],
        rf[0]
    ))
}

/// Prints what the CSP replacement pass did, and only when it did something:
/// the overwhelming majority of cars carry no `ext_config.ini` at all, and a
/// line of zeroes on every one of them would drown the reports that matter.
fn report_ext_config(ext: &kn5_gltf::ExtConfigStats) {
    if ext.applied == 0 {
        return;
    }
    println!(
        "ext_config {} section(s), {} node(s) hidden, {} model(s) inserted (+{} triangles)",
        ext.applied, ext.hidden_nodes, ext.inserted_models, ext.inserted_triangles
    );
    for failure in &ext.failures {
        println!("           ! {failure}");
    }
}

fn convert(car_dir: &Path, flags: &[&str]) -> Result<(), String> {
    let out = option_value(flags, "--out").ok_or("convert needs --out=<file.glb>")?;
    let out = Path::new(out);
    let (file, _) = model_path(car_dir)?;
    let skin = resolve_skin(car_dir, option_value(flags, "--skin"));

    let bytes = std::fs::read(&file).map_err(|e| format!("{}: {e}", file.display()))?;
    let started = Instant::now();
    let mut model = kn5::parse(&bytes).map_err(|e| format!("{}: {e}", file.display()))?;
    let parsed = started.elapsed();

    // Same pass the application runs (`preview.rs`): a tuning mod keeps part
    // of its geometry in separate KN5 files that only `ext_config.ini` knows
    // about, so converting without it produces a car with holes in it.
    let ext = kn5_gltf::apply_ext_config(&mut model, car_dir, &file, skin.as_deref());
    report_ext_config(&ext);

    let started = Instant::now();
    let conversion = kn5_gltf::convert(
        &model,
        skin.as_deref(),
        &kn5_gltf::ConvertOptions::default(),
        &|stage| {
            eprintln!("[stage] {}", stage.as_str());
        },
    )?;
    let converted = started.elapsed();

    if let Some(parent) = out.parent() {
        if !parent.as_os_str().is_empty() {
            std::fs::create_dir_all(parent).map_err(|e| format!("{}: {e}", parent.display()))?;
        }
    }
    std::fs::write(out, &conversion.glb).map_err(|e| format!("{}: {e}", out.display()))?;

    let stats = &conversion.geometry;
    println!("model     {}", file.display());
    println!(
        "skin      {}",
        skin.as_ref()
            .map(|p| p.display().to_string())
            .unwrap_or_else(|| "none".to_string())
    );
    println!(
        "output    {} ({})",
        out.display(),
        human_bytes(conversion.glb.len() as u64)
    );
    println!(
        "meshes    {} kept, merged into {} draw calls — skipped: {} hidden, {} by name, {} empty, {} distant LOD, \
         {} broken glass ({} mirrored nodes)",
        stats.kept,
        stats.merged,
        stats.skipped_hidden,
        stats.skipped_by_name,
        stats.skipped_empty,
        stats.skipped_distant_lod,
        stats.skipped_broken_glass,
        stats.mirrored
    );
    println!(
        "content   {} triangles, {} materials, {} textures",
        conversion.triangle_count, conversion.material_count, conversion.texture_count
    );
    println!(
        "time      {:.0} ms parsing, {:.0} ms converting (total {:.1} s)",
        parsed.as_secs_f64() * 1000.0,
        converted.as_secs_f64() * 1000.0,
        (parsed + converted).as_secs_f64()
    );
    let (agreeing, total) = kn5_gltf::winding_consistency(&model);
    println!(
        "winding   {:.1} % of {total} triangles agree with their normals (internal consistency, not chirality)",
        100.0 * agreeing as f64 / total.max(1) as f64
    );
    match orientation_verdict(&model) {
        Some(verdict) => println!("handedness {verdict}"),
        None => println!("handedness unchecked — no WHEEL_LF/RF/LR/RR nodes in this model"),
    }

    if !conversion.texture_warnings.is_empty() {
        println!("\ntexture warnings");
        for warning in &conversion.texture_warnings {
            println!("  {}: {}", warning.name, warning.reason);
        }
    }

    Ok(())
}
