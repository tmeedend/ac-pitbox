//! Fragments: mod-shaped folders that cannot stand alone, and the search for
//! the mod they belong to (§4.3bis).
//!
//! `modscan::is_car` / `is_track` answer "shaped like a mod", which is not the
//! same question as "is a mod". Both only look at `ui/`, and a folder meant to
//! be **dropped onto** an existing mod carries the very same `ui/` — authors
//! copy it to ship a new `preview.png`. Without geometry
//! ([`modscan::has_geometry`]) such a folder cannot be loaded by the game at
//! all: it is a fragment, and the only useful thing to do with it is to find
//! its host and store it as a layer of that host (§4.4).
//!
//! Real bug this exists for: `Mike08_santamonica01`, a visual overhaul of the
//! Santa Monica Mountains track. It carries `ui/a_stuntRace/`,
//! `ui/c_stuntFreeroam/`, a `texture/` folder and an `extension/ext_config.ini`
//! — no `.kn5`, no `models*.ini`, no `data/`. It was imported as a track of its
//! own, under its own folder name, silently: identity resolution in
//! `importer::process_found` is `dir.file_name()` and nothing else, so a
//! fragment whose folder is named after its author never meets the
//! update-vs-layer arbitration at all. The result is a library entry the game
//! can never load, and a base track that never receives what was meant for it.
//!
//! Geometry is necessary, not sufficient: a track can ship kn5 of its own and
//! still load most of its models from another one (`borrowed_models_host`).
//! It is a layer just the same, only better disguised.
//!
//! The cascade below follows the shape of `submods::sounds::resolve_sound_parent`:
//! ordered sources, surest first, each one documented with what it was measured
//! to be worth on the reference library.

use std::collections::HashMap;
use std::path::{Path, PathBuf};

use rusqlite::Connection;
use walkdir::WalkDir;

use crate::config::AppConfig;
use crate::modscan::ModKind;
use crate::overlay;

/// Where an incoming mod-shaped folder belongs.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Host {
    /// Carries its own geometry: a mod, imported as one. The overwhelming
    /// majority — 224 of the 224 real mods in the reference corpus.
    SelfStanding,
    /// Fragment whose host is in the library (or is stock content) — or a
    /// track that loads that host's models.
    Known(String),
    /// Fragment that names a host which is **not** installed. Nothing can be
    /// composed today, but the id is known, so the content can wait for it.
    Missing(String),
    /// Fragment whose host could not be named. Nothing to attach it to.
    Unknown,
}

/// Number of incoming paths compared against a candidate before giving up.
/// A fragment is small by nature; the cap only guards against an author
/// shipping a whole second copy of the game inside their archive.
const MAX_PROBED_PATHS: usize = 400;

/// Finds the mod an incoming folder belongs to (§4.3bis).
///
/// Returns [`Host::SelfStanding`] for anything that carries its geometry — the
/// gate is checked first and on its own, so a real mod can never be demoted to
/// a fragment by a coincidence in the signals below. The one exception is a
/// track that loads at least `MIN_BORROWED_MODELS` kn5 of a single other
/// track (`borrowed_models_host`).
pub fn resolve(conn: &Connection, cfg: &AppConfig, kind: ModKind, dir: &Path) -> Host {
    if crate::modscan::has_geometry(kind, dir) {
        // Geometry of its own, but maybe not all of it: a track that loads a
        // host's models is a layer of that host however many kn5 it brings.
        return match borrowed_models_host(conn, cfg, kind, dir) {
            Some(id) => Host::Known(id),
            None => Host::SelfStanding,
        };
    }

    let hosts = known_hosts(conn, cfg, kind);
    if hosts.is_empty() {
        // Nothing to attach to yet. The vao-patch may still name the host.
        return match vao_patch_stems(dir).into_iter().find(|s| looks_like_ac_id(s)) {
            Some(stem) => Host::Missing(stem),
            None => Host::Unknown,
        };
    }

    // 1. The folder is already named after a known mod. Nothing to guess: this
    //    is the historical identity rule (`dir.file_name()`), and it stays the
    //    strongest signal there is. What changes for a fragment is not *where*
    //    it goes but *how* — the caller forces a layer rather than letting the
    //    file counts decide, because a fragment can never replace a base.
    if let Some(name) = dir.file_name().and_then(|n| n.to_str()) {
        if hosts.iter().any(|(id, _)| id == name) {
            return Host::Known(name.to_string());
        }
    }

    // 2. The `.vao-patch` names the model it patches — 121/124 in the corpus.
    if let Some(id) = only(vao_patch_hosts(dir, &hosts)) {
        return Host::Known(id);
    }

    // 3. Layout folders (track) or skin folders (car) shared with one host.
    let structural = structural_hosts(kind, dir, &hosts);
    if let Some(id) = only(structural.clone()) {
        return Host::Known(id);
    }
    // Several hosts share the layout name (`reverse`, `normal`, `short`… are
    // used by up to 6 tracks each): the paths that already exist decide.
    if structural.len() > 1 {
        if let Some(id) = path_overlap_host(dir, &hosts_named(&hosts, &structural)) {
            return Host::Known(id);
        }
    }

    // 4. A host id written in the folder name itself.
    if let Some(id) = only(name_hosts(dir, &hosts)) {
        return Host::Known(id);
    }

    // 5. Last resort: which host already owns the paths this folder brings?
    if let Some(id) = path_overlap_host(dir, &hosts) {
        return Host::Known(id);
    }

    match vao_patch_stems(dir).into_iter().find(|s| looks_like_ac_id(s)) {
        Some(stem) => Host::Missing(stem),
        None => Host::Unknown,
    }
}

/// Fewest kn5 a track must borrow from one host to be read as its layer.
///
/// A models ini naming a kn5 the folder does not ship is common in complete,
/// playable tracks — AC skips it. Measured on the reference corpus (124 library
/// track versions, 142 tracks of the AC install): 7 complete tracks do it,
/// Kunos' own `drift` and `spa` among them, and **never for more than 2 kn5**
/// (`11.kn5`/`12.kn5`, a leftover `bal_cocacola.kn5`…). One of those leftovers
/// is owned by exactly one other track (`nurburgring1967` → `willow_springs`),
/// so a unique owner alone is not proof. The layer this exists for borrows 10.
const MIN_BORROWED_MODELS: usize = 3;

/// The host a track with geometry of its own still depends on (§4.3bis).
///
/// Real case this exists for: `grandvalleyspeedway2010.rar`, four 2010
/// layouts meant to be extracted **into** `grand_valley_speedway_LL`. It ships
/// its own main kn5, so it passed the geometry gate and became a track of its
/// own; but every one of its `models_*.ini` also loads ten kn5 of the base
/// track (`cubes_normal.kn5`, `full layout.kn5`…). Launched alone, CSP
/// reports each of them as "KN5 file is damaged or empty".
///
/// A folder already named after a known mod is left to the historical
/// arbitration (update or layer from the file counts): this only catches the
/// content that would otherwise create a new, broken entry.
fn borrowed_models_host(conn: &Connection, cfg: &AppConfig, kind: ModKind, dir: &Path) -> Option<String> {
    if kind != ModKind::Track {
        return None;
    }
    // Read first and alone: the overwhelming majority stops here, without
    // listing the library.
    let missing = missing_models(dir);
    if missing.len() < MIN_BORROWED_MODELS {
        return None;
    }
    let hosts = known_hosts(conn, cfg, kind);
    let name = dir.file_name().and_then(|n| n.to_str());
    if hosts.iter().any(|(id, _)| Some(id.as_str()) == name) {
        return None;
    }
    lender_host(&missing, &hosts)
}

/// The single host that owns every one of the `missing` models.
fn lender_host(missing: &[String], hosts: &[(String, PathBuf)]) -> Option<String> {
    only(
        hosts
            .iter()
            .filter(|(_, host_dir)| missing.iter().all(|m| host_dir.join(m).is_file()))
            .map(|(id, _)| id.clone())
            .collect(),
    )
}

/// The kn5 named by the `models*.ini` of a track that the track does not ship,
/// once each, as written in the ini.
///
/// The ini sits at the root or, for some authors, in the layout folder (same
/// two places as `modscan::has_geometry`); a path is looked up from both the
/// track root and the ini's own folder before being called missing.
fn missing_models(dir: &Path) -> Vec<String> {
    let mut inis: Vec<PathBuf> = models_inis(dir);
    for sub in std::fs::read_dir(dir).into_iter().flatten().flatten() {
        if sub.path().is_dir() {
            inis.extend(models_inis(&sub.path()));
        }
    }
    let mut out: Vec<String> = Vec::new();
    for ini in inis {
        let ini_dir = ini.parent().unwrap_or(dir);
        for file in model_files(&read_ini(&ini)) {
            let present = dir.join(&file).is_file() || ini_dir.join(&file).is_file();
            if !present && !out.iter().any(|o| o.eq_ignore_ascii_case(&file)) {
                out.push(file);
            }
        }
    }
    out
}

/// `models.ini` / `models_<layout>.ini` directly in `dir`.
fn models_inis(dir: &Path) -> Vec<PathBuf> {
    std::fs::read_dir(dir)
        .into_iter()
        .flatten()
        .flatten()
        .map(|e| e.path())
        .filter(|p| {
            p.is_file()
                && p.file_name()
                    .and_then(|n| n.to_str())
                    .is_some_and(crate::modscan::is_models_ini)
        })
        .collect()
}

/// An ini as text. Not every author writes UTF-8, and a kn5 name is where the
/// accents are (`señales y cajones normales.kn5`): a Windows-1252 file read
/// lossily would name a file that exists nowhere. Latin-1 is Windows-1252
/// for every letter a file name uses.
fn read_ini(path: &Path) -> String {
    let bytes = std::fs::read(path).unwrap_or_default();
    match String::from_utf8(bytes) {
        Ok(text) => text,
        Err(e) => e.into_bytes().iter().map(|&b| char::from(b)).collect(),
    }
}

/// The `FILE=` values of a models ini, comments skipped.
fn model_files(ini: &str) -> Vec<String> {
    ini.lines()
        .filter_map(|line| {
            let line = line.trim_start_matches('\u{feff}').split(';').next().unwrap_or("");
            let (key, value) = line.split_once('=')?;
            let value = value.trim().replace('\\', "/");
            (key.trim().eq_ignore_ascii_case("FILE") && !value.is_empty()).then_some(value)
        })
        .collect()
}

/// The single element of a candidate list, or `None` if there is not exactly
/// one. Guessing between two hosts is worse than not guessing: the content
/// would silently land on a mod it was never meant for.
fn only(mut candidates: Vec<String>) -> Option<String> {
    (candidates.len() == 1).then(|| candidates.remove(0))
}

/// Every mod of this kind we could attach a fragment to, with its content
/// folder on disk. Stock and unmanaged content included: a fragment posted on
/// a Kunos track is the very case §4.4 calls out as always becoming a layer.
fn known_hosts(conn: &Connection, cfg: &AppConfig, kind: ModKind) -> Vec<(String, PathBuf)> {
    let want = format!("{kind:?}");
    overlay::list_mods(conn)
        .unwrap_or_default()
        .into_iter()
        .filter(|m| m.kind == want)
        .filter_map(|m| {
            let dir = crate::submods::parent_content_dir(conn, cfg, &m.id_interne)?;
            dir.is_dir().then_some((m.id_interne, dir))
        })
        .collect()
}

/// Restricts the host list to a set of ids, keeping their resolved folders.
fn hosts_named(hosts: &[(String, PathBuf)], ids: &[String]) -> Vec<(String, PathBuf)> {
    hosts.iter().filter(|(id, _)| ids.contains(id)).cloned().collect()
}

/// Stems of the `*.vao-patch` files at the root of `dir`.
fn vao_patch_stems(dir: &Path) -> Vec<String> {
    std::fs::read_dir(dir)
        .into_iter()
        .flatten()
        .flatten()
        .map(|e| e.path())
        .filter(|p| p.extension().is_some_and(|x| x.eq_ignore_ascii_case("vao-patch")))
        .filter_map(|p| p.file_stem().map(|s| s.to_string_lossy().into_owned()))
        .collect()
}

/// Hosts that own the model a `.vao-patch` patches.
///
/// A vao-patch is **not** named after the track id — that only holds for 32 of
/// the 124 in the corpus (`models_chicanes.vao-patch`, `sx_lemans_bldg`,
/// `paris_seine`…). It is named after the `.kn5` or the `models*.ini` it goes
/// with, and *that* holds **121/124** (the 3 others are leftovers of models
/// their track no longer ships). Since a kn5 name is distinctive, matching the
/// stem against the files a host actually owns is both reliable and precise —
/// far more so than matching it against the id.
fn vao_patch_hosts(dir: &Path, hosts: &[(String, PathBuf)]) -> Vec<String> {
    let stems = vao_patch_stems(dir);
    if stems.is_empty() {
        return Vec::new();
    }
    let mut out: Vec<String> = hosts
        .iter()
        .filter(|(_, host_dir)| {
            stems
                .iter()
                .any(|s| host_dir.join(format!("{s}.kn5")).is_file() || host_dir.join(format!("{s}.ini")).is_file())
        })
        .map(|(id, _)| id.clone())
        .collect();
    out.dedup();
    out
}

/// Names of the direct subfolders of `dir/<sub>`, lowercased.
fn subfolder_names(dir: &Path, sub: &str) -> Vec<String> {
    std::fs::read_dir(dir.join(sub))
        .into_iter()
        .flatten()
        .flatten()
        .filter(|e| e.path().is_dir())
        .map(|e| e.file_name().to_string_lossy().to_lowercase())
        .collect()
}

/// Hosts sharing a layout (track) or skin (car) folder name with the fragment.
///
/// Layout names are author-chosen and mostly distinctive: **185 of the 224**
/// (layout, track) pairs in the library carry a name that belongs to exactly
/// one track. The remaining 39 come from 13 generic names — `reverse` and
/// `normal` are used by 6 tracks each, `short` by 4 — so a single shared name
/// is *not* proof. Two of them landing on the same host is, which is why the
/// caller keeps the whole candidate list rather than the first hit.
fn structural_hosts(kind: ModKind, dir: &Path, hosts: &[(String, PathBuf)]) -> Vec<String> {
    let (sub, mine) = match kind {
        ModKind::Track => ("ui", subfolder_names(dir, "ui")),
        ModKind::Car => ("skins", subfolder_names(dir, "skins")),
    };
    if mine.is_empty() {
        return Vec::new();
    }
    let mut scored: Vec<(String, usize)> = hosts
        .iter()
        .filter_map(|(id, host_dir)| {
            let theirs = subfolder_names(host_dir, sub);
            let hits = mine.iter().filter(|n| theirs.contains(n)).count();
            (hits > 0).then(|| (id.clone(), hits))
        })
        .collect();
    if scored.is_empty() {
        return Vec::new();
    }
    // Two or more shared names on a single host settle it on their own, even
    // when other hosts share one generic name with the fragment.
    let best = scored.iter().map(|(_, n)| *n).max().unwrap_or(0);
    if best >= 2 && scored.iter().filter(|(_, n)| *n == best).count() == 1 {
        scored.retain(|(_, n)| *n == best);
    }
    scored.into_iter().map(|(id, _)| id).collect()
}

/// Hosts whose id appears in the fragment's folder name.
///
/// Same idea as `submods::sounds::guess_sound_parent`, and the same caution: this is
/// the weakest signal of the cascade, kept because authors often do write the
/// target in the folder name (`ks_nordschleife_extra_trees`). Only an id
/// written **in full** counts — a fuzzy segment match, useful for sounds where
/// nothing else exists, would here fire on half the library.
fn name_hosts(dir: &Path, hosts: &[(String, PathBuf)]) -> Vec<String> {
    let Some(name) = dir.file_name().map(|n| n.to_string_lossy().to_lowercase()) else {
        return Vec::new();
    };
    hosts
        .iter()
        .filter(|(id, _)| id.len() >= 5 && name.contains(&id.to_lowercase()))
        .map(|(id, _)| id.clone())
        .collect()
}

/// The host that already owns the paths this fragment brings.
///
/// The generic net under the rest: whatever the fragment is, the files it
/// overwrites are files of its host. It needs a clear winner rather than a
/// mere maximum — `extension/ext_config.ini` alone exists in dozens of tracks,
/// so a one-path lead proves nothing. Requiring at least two matches and twice
/// the runner-up keeps it silent unless the overlap is real: on
/// `Mike08_santamonica01` the score is 9 against 1 for every other track.
fn path_overlap_host(dir: &Path, hosts: &[(String, PathBuf)]) -> Option<String> {
    let rels: Vec<PathBuf> = WalkDir::new(dir)
        .into_iter()
        .flatten()
        .filter(|e| e.file_type().is_file())
        .filter_map(|e| e.path().strip_prefix(dir).ok().map(|p| p.to_path_buf()))
        .take(MAX_PROBED_PATHS)
        .collect();
    if rels.is_empty() {
        return None;
    }
    let mut scores: HashMap<&str, usize> = HashMap::new();
    for (id, host_dir) in hosts {
        let hits = rels.iter().filter(|rel| host_dir.join(rel).exists()).count();
        if hits > 0 {
            scores.insert(id.as_str(), hits);
        }
    }
    let mut ranked: Vec<(&str, usize)> = scores.into_iter().collect();
    ranked.sort_by(|a, b| b.1.cmp(&a.1).then(a.0.cmp(b.0)));
    let (id, best) = *ranked.first()?;
    let runner_up = ranked.get(1).map(|(_, n)| *n).unwrap_or(0);
    (best >= 2 && best >= runner_up * 2).then(|| id.to_string())
}

/// Has the shape of an AC content id: a compound word, not a generic label.
/// Same test as `submods::sounds::looks_like_ac_id`, for the same reason — it keeps a
/// `models.vao-patch` from being read as the name of a mod to wait for.
fn looks_like_ac_id(name: &str) -> bool {
    name.len() >= 5 && name.contains('_') && !name.to_ascii_lowercase().starts_with("models")
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::testutil::temp_dir;

    /// Builds a mod-shaped folder; `geometry` decides whether it can stand alone.
    fn make_track(dir: &Path, layouts: &[&str], geometry: bool) {
        for l in layouts {
            let ui = dir.join("ui").join(l);
            std::fs::create_dir_all(&ui).unwrap();
            std::fs::write(ui.join("ui_track.json"), r#"{"name":"T"}"#).unwrap();
            std::fs::write(ui.join("preview.png"), b"p").unwrap();
        }
        if geometry {
            std::fs::write(dir.join("track.kn5"), b"k").unwrap();
        }
    }

    /// Règle : un dossier qui porte sa géométrie reste un mod (§4.3bis).
    #[test]
    fn a_mod_with_geometry_is_never_a_fragment() {
        let base = temp_dir("frag-standalone");
        let dir = base.join("some_track");
        make_track(&dir, &["reverse"], true);
        assert!(
            crate::modscan::has_geometry(ModKind::Track, &dir),
            "a track with a kn5 stands on its own"
        );
    }

    /// Règle : sans kn5 ni models*.ini, le dossier n'est pas jouable (§4.3bis).
    #[test]
    fn a_ui_only_folder_has_no_geometry() {
        let base = temp_dir("frag-noeom");
        let dir = base.join("Mike08_santamonica01");
        make_track(&dir, &["a_stuntRace"], false);
        assert!(
            !crate::modscan::has_geometry(ModKind::Track, &dir),
            "ui/ alone cannot be loaded by the game"
        );
    }

    /// Règle : un models_<layout>.ini vaut géométrie, même sans kn5 à la racine.
    #[test]
    fn models_ini_counts_as_geometry() {
        let base = temp_dir("frag-models");
        let dir = base.join("t");
        make_track(&dir, &["gp"], false);
        std::fs::write(dir.join("models_gp.ini"), b"[MODEL_0]").unwrap();
        assert!(
            crate::modscan::has_geometry(ModKind::Track, &dir),
            "models_<layout>.ini names the kn5 the track loads"
        );
    }

    /// Rule: a models ini line names a kn5 whatever its spelling — comments,
    /// a BOM, backslashes and spaces in the name (§4.3bis).
    #[test]
    fn model_files_reads_every_file_line() {
        let ini = "\u{feff}[MODEL_0]\nFILE=main.kn5\n; FILE=old.kn5\n[MODEL_1]\nfile = full layout.kn5 ; tunnel\n\
                   [MODEL_2]\nFILE=sub\\cubes.kn5\nPOSITION=0,0,0\nFILE=\n";
        assert_eq!(
            model_files(ini),
            vec!["main.kn5", "full layout.kn5", "sub/cubes.kn5"],
            "commented lines and empty values name nothing"
        );
    }

    /// Rule: a Windows-1252 ini still names its accented kn5 (§4.3bis).
    #[test]
    fn a_windows_1252_ini_names_its_accented_models() {
        let base = temp_dir("frag-cp1252");
        let dir = base.join("t");
        std::fs::create_dir_all(&dir).unwrap();
        let mut ini = b"[MODEL_0]\nFILE=se".to_vec();
        ini.push(0xF1); // « ñ » in Windows-1252: invalid UTF-8 on purpose
        ini.extend_from_slice(b"ales.kn5\n");
        std::fs::write(dir.join("models_gp.ini"), ini).unwrap();
        std::fs::write(dir.join("señales.kn5"), b"k").unwrap();
        assert!(missing_models(&dir).is_empty(), "the file exists under its real name");
    }

    /// Rule: a model is missing only if neither the track root nor the ini's
    /// own folder has it (§4.3bis).
    #[test]
    fn a_model_beside_its_layout_ini_is_not_missing() {
        let base = temp_dir("frag-ini-in-layout");
        let dir = base.join("t");
        std::fs::create_dir_all(dir.join("gp")).unwrap();
        std::fs::write(dir.join("root.kn5"), b"k").unwrap();
        std::fs::write(dir.join("gp").join("local.kn5"), b"k").unwrap();
        std::fs::write(
            dir.join("gp").join("models_gp.ini"),
            "FILE=root.kn5\nFILE=local.kn5\nFILE=gone.kn5\nFILE=GONE.kn5\n",
        )
        .unwrap();
        assert_eq!(missing_models(&dir), vec!["gone.kn5"], "listed once, case aside");
    }

    /// Rule: borrowed models name a host only when one track owns them all.
    #[test]
    fn borrowed_models_need_a_single_owner() {
        let base = temp_dir("frag-lender");
        let owns = |name: &str, files: &[&str]| {
            let d = base.join(name);
            std::fs::create_dir_all(&d).unwrap();
            for f in files {
                std::fs::write(d.join(f), b"k").unwrap();
            }
            (name.to_string(), d)
        };
        let missing: Vec<String> = ["a.kn5", "b.kn5", "c.kn5"].map(String::from).to_vec();
        let full = owns("full_owner", &["a.kn5", "b.kn5", "c.kn5"]);
        let partial = owns("partial_owner", &["a.kn5", "b.kn5"]);
        assert_eq!(
            lender_host(&missing, &[full.clone(), partial.clone()]),
            Some("full_owner".to_string()),
            "owning only some of them is no claim"
        );
        let twin = owns("twin_owner", &["a.kn5", "b.kn5", "c.kn5"]);
        assert_eq!(
            lender_host(&missing, &[full, partial, twin]),
            None,
            "two owners are never decided between"
        );
    }

    /// Règle : deux layouts concordants désignent l'hôte à eux seuls (§4.3bis).
    #[test]
    fn two_shared_layouts_name_the_host() {
        let base = temp_dir("frag-layouts");
        let host = base.join("santa_monica_mtns");
        make_track(&host, &["a_stuntRace", "c_stuntFreeroam", "b_theSnakeRace"], true);
        let other = base.join("other_track");
        make_track(&other, &["reverse"], true);
        let frag = base.join("Mike08_santamonica01");
        make_track(&frag, &["a_stuntRace", "c_stuntFreeroam"], false);

        let hosts = vec![
            ("santa_monica_mtns".to_string(), host.clone()),
            ("other_track".to_string(), other.clone()),
        ];
        assert_eq!(
            structural_hosts(ModKind::Track, &frag, &hosts),
            vec!["santa_monica_mtns".to_string()],
            "the two layout names belong to one track only"
        );
    }

    /// Règle : un nom de layout générique ne suffit pas à désigner un hôte.
    #[test]
    fn one_generic_layout_name_leaves_several_candidates() {
        let base = temp_dir("frag-generic");
        let a = base.join("track_a");
        make_track(&a, &["reverse"], true);
        let b = base.join("track_b");
        make_track(&b, &["reverse"], true);
        let frag = base.join("some_overhaul");
        make_track(&frag, &["reverse"], false);

        let hosts = vec![("track_a".to_string(), a.clone()), ("track_b".to_string(), b.clone())];
        let got = structural_hosts(ModKind::Track, &frag, &hosts);
        assert_eq!(got.len(), 2, "`reverse` is shared: not enough to decide alone");
        assert!(only(got).is_none(), "an ambiguous list must never be resolved");
    }

    /// Règle : le vao-patch désigne le kn5 qu'il accompagne, pas l'id (§4.3bis).
    #[test]
    fn vao_patch_points_at_the_model_it_patches() {
        let base = temp_dir("frag-vao");
        let host = base.join("sx_lemans");
        make_track(&host, &["gp"], true);
        std::fs::write(host.join("sx_lemans_bldg.kn5"), b"k").unwrap();
        let other = base.join("other_track");
        make_track(&other, &["gp"], true);
        let frag = base.join("lemans_ao");
        make_track(&frag, &["gp"], false);
        std::fs::write(frag.join("sx_lemans_bldg.vao-patch"), b"v").unwrap();

        let hosts = vec![
            ("sx_lemans".to_string(), host.clone()),
            ("other_track".to_string(), other.clone()),
        ];
        assert_eq!(
            vao_patch_hosts(&frag, &hosts),
            vec!["sx_lemans".to_string()],
            "the stem names a kn5 only one track owns"
        );
    }

    /// Règle : le recouvrement de chemins départage, mais exige un vrai écart.
    #[test]
    fn path_overlap_needs_a_clear_winner() {
        let base = temp_dir("frag-overlap");
        let host = base.join("santa_monica_mtns");
        make_track(&host, &["a_stuntRace", "c_stuntFreeroam"], true);
        std::fs::create_dir_all(host.join("extension")).unwrap();
        std::fs::write(host.join("extension").join("ext_config.ini"), b"[]").unwrap();
        let other = base.join("other_track");
        make_track(&other, &["gp"], true);
        std::fs::create_dir_all(other.join("extension")).unwrap();
        std::fs::write(other.join("extension").join("ext_config.ini"), b"[]").unwrap();

        let frag = base.join("Mike08_santamonica01");
        make_track(&frag, &["a_stuntRace", "c_stuntFreeroam"], false);
        std::fs::create_dir_all(frag.join("extension")).unwrap();
        std::fs::write(frag.join("extension").join("ext_config.ini"), b"[]").unwrap();

        let hosts = vec![
            ("santa_monica_mtns".to_string(), host.clone()),
            ("other_track".to_string(), other.clone()),
        ];
        assert_eq!(
            path_overlap_host(&frag, &hosts),
            Some("santa_monica_mtns".to_string()),
            "4 shared paths against 1 is a clear winner"
        );

        // The shared `ext_config.ini` alone must not name a host.
        let thin = base.join("thin_frag");
        std::fs::create_dir_all(thin.join("extension")).unwrap();
        std::fs::write(thin.join("extension").join("ext_config.ini"), b"[]").unwrap();
        assert_eq!(
            path_overlap_host(&thin, &hosts),
            None,
            "one path shared by every track proves nothing"
        );
    }

    /// Règle : un `models.vao-patch` ne nomme pas un mod à attendre.
    #[test]
    fn generic_vao_patch_stem_is_not_an_id() {
        assert!(looks_like_ac_id("santa_monica_mtns"), "a compound id is usable");
        assert!(!looks_like_ac_id("models"), "generic stem, names nothing");
        assert!(!looks_like_ac_id("models_gp"), "a models ini names a layout, not a mod");
        assert!(!looks_like_ac_id("track"), "too short and not compound");
    }
}
