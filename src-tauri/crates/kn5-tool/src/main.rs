//! Developer CLI for the KN5 pipeline — never shipped to users (spec §5.1).
//!
//! Exists so that every lot before the viewer can be verified without
//! launching the application: `inspect` proves the parser against one file,
//! `scan` proves it against a whole `content/cars` folder.
//!
//! ```text
//! kn5-tool inspect <file.kn5 | car_dir> [--tree] [--materials] [--textures]
//! kn5-tool scan <dir> [--details]
//! ```

mod report;
mod resolve;

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::process::ExitCode;

use kn5::{Kn5Model, Kn5NodeKind};

use report::{by_count, human_bytes, Stats};
use resolve::{resolve_model, ModelSource};

const USAGE: &str = "\
kn5-tool — inspect Assetto Corsa KN5 models

USAGE:
    kn5-tool inspect <file.kn5 | car_dir> [--tree] [--materials] [--textures]
    kn5-tool scan <dir> [--details]

COMMANDS:
    inspect   Parse one model and report what it contains.
    scan      Parse every car of a folder (e.g. content/cars) and aggregate.

OPTIONS:
    --tree        Print the node hierarchy.
    --materials   Print every material with its shader, properties and samplers.
    --textures    Print every embedded texture with its sniffed format.
    --details     scan: also print the aggregate shader / property tables.
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
    let model = kn5::parse(&bytes).map_err(|e| format!("{}: {e}", file.display()))?;

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
