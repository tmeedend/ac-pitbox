//! Lists the sample story's files for `sample.rs`, which carries them inside
//! Pit Box: every file of `sample/` outside `media/` as text, every file of
//! `media/` as bytes. Written by hand, the list forgot a file as soon as one
//! was added to the folder - the played sample then differed from the
//! tested one, and a copy missed its pictures.

use std::fmt::Write;
use std::path::Path;

fn walk(dir: &Path, prefix: &str, out: &mut Vec<String>) {
    for entry in std::fs::read_dir(dir).expect("sample folder").flatten() {
        let name = entry.file_name().to_string_lossy().into_owned();
        let path = format!("{prefix}{name}");
        if entry.path().is_dir() {
            walk(&entry.path(), &format!("{path}/"), out);
        } else {
            out.push(path);
        }
    }
}

fn main() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("sample");
    // A directory is scanned whole: a file added, changed or removed
    // anywhere under it builds the list again.
    println!("cargo:rerun-if-changed=sample");
    let mut paths = Vec::new();
    walk(&root, "", &mut paths);
    paths.sort();
    // Carried as bytes: `media/`, and a skin's own files beside its
    // `skin.yaml` - the same rule as `load::read_skin_files`, which this
    // script cannot call.
    let (media, text): (Vec<_>, Vec<_>) = paths
        .iter()
        .partition(|p| p.starts_with("media/") || (p.starts_with("frames/skins/") && !p.ends_with(".yaml")));
    let source = |p: &str| root.join(p).to_string_lossy().into_owned();
    let mut code = String::from("const FILES: &[(&str, &str)] = &[\n");
    for p in text {
        writeln!(code, "    ({p:?}, include_str!({:?})),", source(p)).unwrap();
    }
    code.push_str("];\n\nconst MEDIA: &[(&str, &[u8])] = &[\n");
    for p in media {
        writeln!(code, "    ({p:?}, include_bytes!({:?})),", source(p)).unwrap();
    }
    code.push_str("];\n");
    let out = Path::new(&std::env::var("OUT_DIR").unwrap()).join("sample_files.rs");
    std::fs::write(out, code).expect("sample list written");
}
