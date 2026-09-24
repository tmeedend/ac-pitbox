//! Brand logos (TAXO§3 to §5, §9): one CANONICAL logo per brand, elected
//! among the `badge.png` of its cars, and the light plate for a logo whose
//! background is baked in.
//!
//! Two levels, never mixed (TAXO§3): a car keeps ITS badge wherever the car
//! is shown - a 1969 Fairlady with the period logo, a 2013 GT-R with the
//! modern one, that is right, not inconsistent. The canonical logo is for
//! where the BRAND is shown: the index tile, the Brands tab.
//!
//! No logo ships with Pit Box (TAXO§13, they are trademarks): every one comes
//! from the user's mods, or from a file he gave, copied into Pit Box's own
//! folder - never into the game's (TAXO§9).

use std::collections::{BTreeMap, HashMap};
use std::path::{Path, PathBuf};
use std::sync::Mutex;
use std::time::SystemTime;

use serde::{Deserialize, Serialize};

// --- What a logo file is ---------------------------------------------------------

/// What surrounds the drawing, read on the image's border ring (TAXO§4).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Background {
    /// The border is see-through: the logo sits on any background.
    Transparent,
    /// Opaque and one flat colour - a white square around the Porsche crest.
    /// Drawn on the light plate (TAXO§5), so it reads as a choice.
    Baked,
    /// Opaque but not flat: the drawing reaches the edges (a round badge
    /// filling its square, a photo). Shown as it is.
    Opaque,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct Analysis {
    pub width: u32,
    pub height: u32,
    pub background: Background,
    /// FNV-1a of the file's bytes: the same logo shipped by forty cars is ONE
    /// variant with forty votes.
    pub hash: String,
}

/// Share of the border ring that must be opaque, and flat, for a baked
/// background (TAXO§4: "more than 95 %", hard-coded, not exposed).
const BAKED_OPAQUE: f64 = 0.95;
/// Two border pixels are "the same colour" within this distance (sum of the
/// RGB differences): JPEG noise and antialiasing stay under it, a drawing
/// reaching the edge does not.
const FLAT_DISTANCE: i32 = 36;
/// Share of the ring that must be see-through for a transparent background.
const TRANSPARENT_SHARE: f64 = 0.5;

fn fnv(bytes: &[u8]) -> String {
    let mut h: u64 = 0xcbf29ce484222325;
    for b in bytes {
        h ^= u64::from(*b);
        h = h.wrapping_mul(0x100000001b3);
    }
    format!("{h:016x}")
}

/// The verdict on a decoded image - separate from the file for the tests.
pub fn classify(img: &image::RgbaImage) -> Background {
    let (w, h) = img.dimensions();
    if w == 0 || h == 0 {
        return Background::Opaque;
    }
    // A ring as thick as 1/32 of the smaller side: a pixel is too thin on a
    // 512 px badge, where an antialiased edge alone would decide.
    let ring = (w.min(h) / 32).max(1);
    let mut border: Vec<[u8; 4]> = Vec::new();
    for y in 0..h {
        for x in 0..w {
            if x < ring || y < ring || x >= w - ring || y >= h - ring {
                border.push(img.get_pixel(x, y).0);
            }
        }
    }
    let n = border.len() as f64;
    let clear = border.iter().filter(|p| p[3] <= 16).count() as f64;
    if clear / n >= TRANSPARENT_SHARE {
        return Background::Transparent;
    }
    let opaque: Vec<&[u8; 4]> = border.iter().filter(|p| p[3] >= 250).collect();
    if (opaque.len() as f64) / n <= BAKED_OPAQUE {
        return Background::Opaque;
    }
    // Flat: most opaque border pixels close to their median colour.
    let median = |c: usize| {
        let mut v: Vec<u8> = opaque.iter().map(|p| p[c]).collect();
        v.sort_unstable();
        i32::from(v[v.len() / 2])
    };
    let m = [median(0), median(1), median(2)];
    let flat = opaque
        .iter()
        .filter(|p| (0..3).map(|c| (i32::from(p[c]) - m[c]).abs()).sum::<i32>() <= FLAT_DISTANCE)
        .count() as f64;
    if flat / n > BAKED_OPAQUE {
        Background::Baked
    } else {
        Background::Opaque
    }
}

/// Modification time and size: what tells a changed file.
type Stamp = (SystemTime, u64);
type Cache = HashMap<PathBuf, (Stamp, Option<Analysis>)>;
static CACHE: Mutex<Option<Cache>> = Mutex::new(None);

/// Reads and classifies a logo file. Cached by path, size and modification
/// time: the library listing asks for every badge, every time, and a badge
/// changes only when a mod does.
pub fn analyze(path: &Path) -> Option<Analysis> {
    let meta = std::fs::metadata(path).ok()?;
    let stamp = (meta.modified().unwrap_or(SystemTime::UNIX_EPOCH), meta.len());
    if let Ok(cache) = CACHE.lock() {
        if let Some((s, a)) = cache.as_ref().and_then(|c| c.get(path)) {
            if *s == stamp {
                return a.clone();
            }
        }
    }
    let result = read(path);
    if let Ok(mut cache) = CACHE.lock() {
        cache
            .get_or_insert_with(HashMap::new)
            .insert(path.to_path_buf(), (stamp, result.clone()));
    }
    result
}

fn read(path: &Path) -> Option<Analysis> {
    let bytes = std::fs::read(path).ok()?;
    let hash = fnv(&bytes);
    if path.extension().is_some_and(|e| e.eq_ignore_ascii_case("svg")) {
        // A vector file the user gave: drawn at any size, and a logo
        // exported as SVG has no baked square around it.
        return Some(Analysis {
            width: u32::MAX,
            height: u32::MAX,
            background: Background::Transparent,
            hash,
        });
    }
    match image::load_from_memory(&bytes) {
        Ok(img) => {
            let rgba = img.to_rgba8();
            Some(Analysis {
                width: rgba.width(),
                height: rgba.height(),
                background: classify(&rgba),
                hash,
            })
        }
        Err(e) => {
            log::warn!("logo {} unreadable: {e}", path.display());
            None
        }
    }
}

// --- The user's choices --------------------------------------------------------

/// What the user decided for one brand. Nothing means "automatic".
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct BrandPref {
    /// A variant he picked, by its hash.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub variant: Option<String>,
    /// A file of his, in `logos/` (TAXO§9). Wins over any variant.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub custom: Option<String>,
    /// Forces the plate on or off for the canonical logo - a background can
    /// be detected wrong (TAXO§5). Only there: at car level the detection
    /// stands, nobody corrects 312 badges.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub plaque: Option<bool>,
}

impl BrandPref {
    pub fn is_empty(&self) -> bool {
        self == &BrandPref::default()
    }
}

pub type Prefs = BTreeMap<String, BrandPref>;

pub const PREFS_FILE: &str = "brand_logos.json";
pub const LOGOS_DIR: &str = "logos";

/// Read at each use (a few lines): written synchronously by `save_prefs`,
/// never kept in browser storage (CLAUDE.md rule 6).
pub fn load_prefs(dir: &Path) -> Prefs {
    let path = dir.join(PREFS_FILE);
    match std::fs::read_to_string(&path) {
        Ok(s) => serde_json::from_str(&s).unwrap_or_else(|e| {
            log::warn!("{} unreadable, logo choices ignored: {e}", path.display());
            Prefs::new()
        }),
        Err(_) => Prefs::new(),
    }
}

pub fn save_prefs(dir: &Path, prefs: &Prefs) -> Result<(), String> {
    let prefs: Prefs = prefs
        .iter()
        .filter(|(_, p)| !p.is_empty())
        .map(|(k, v)| (k.clone(), v.clone()))
        .collect();
    let json = serde_json::to_string_pretty(&prefs).map_err(|e| e.to_string())?;
    std::fs::write(dir.join(PREFS_FILE), json).map_err(|e| e.to_string())
}

/// Copies a logo file of the user's into `logos/` and returns its name
/// there. PNG of at least 64 px on its smaller side, or SVG (TAXO§9). The
/// name carries the content's hash: a replaced logo gets a new URL, and the
/// webview's cache of the old one cannot show through.
pub fn import_file(dir: &Path, brand: &str, source: &Path) -> Result<String, String> {
    let ext = source
        .extension()
        .and_then(|e| e.to_str())
        .map(str::to_lowercase)
        .filter(|e| e == "png" || e == "svg")
        .ok_or(crate::errors::LOGO_FORMAT)?;
    let a = read(source).ok_or(crate::errors::LOGO_FORMAT)?;
    if ext == "png" && a.width.min(a.height) < 64 {
        return Err(crate::errors::LOGO_TOO_SMALL.to_string());
    }
    let slug: String = brand
        .to_lowercase()
        .chars()
        .map(|c| if c.is_ascii_alphanumeric() { c } else { '-' })
        .collect();
    let name = format!("{}-{}.{ext}", slug.trim_matches('-'), &a.hash[..8]);
    let logos = dir.join(LOGOS_DIR);
    std::fs::create_dir_all(&logos).map_err(|e| e.to_string())?;
    std::fs::copy(source, logos.join(&name)).map_err(|e| e.to_string())?;
    Ok(name)
}

/// Removes a file of ours from `logos/` - only a bare name we wrote there,
/// never a path that could reach elsewhere.
pub fn remove_file(dir: &Path, name: &str) {
    if name.contains(['/', '\\']) || name.contains("..") {
        log::warn!("logo file name refused: {name}");
        return;
    }
    let path = dir.join(LOGOS_DIR).join(name);
    if let Err(e) = std::fs::remove_file(&path) {
        log::warn!("logo {} not removed: {e}", path.display());
    }
}

/// Records his choice for a brand. A file of his that the new choice no
/// longer names is deleted from `logos/` - it is ours, and nothing else
/// points to it.
pub fn set_pref(dir: &Path, brand: &str, pref: BrandPref) -> Result<(), String> {
    let mut prefs = load_prefs(dir);
    let old = prefs.get(brand).and_then(|p| p.custom.clone());
    if pref.is_empty() {
        prefs.remove(brand);
    } else {
        prefs.insert(brand.to_string(), pref.clone());
    }
    save_prefs(dir, &prefs)?;
    if let Some(old) = old.filter(|o| pref.custom.as_ref() != Some(o)) {
        remove_file(dir, &old);
    }
    Ok(())
}

// --- The election ------------------------------------------------------------------

/// One logo a brand's cars ship.
#[derive(Debug, Clone, Serialize)]
pub struct Variant {
    pub hash: String,
    /// One of the files with this content (the first car's, by id).
    pub path: String,
    pub width: u32,
    pub height: u32,
    pub background: Background,
    /// Cars shipping it: the vote (TAXO§4, criterion 3).
    pub cars: usize,
}

#[derive(Debug, Clone, Serialize)]
pub struct BrandLogo {
    pub brand: String,
    /// The logo shown for the brand, `None` when none of its cars has one.
    pub path: Option<String>,
    pub plaque: bool,
    /// `auto`, `variant` (picked) or `custom` (his file).
    pub choice: &'static str,
    /// By the order of the election: the first is the automatic choice.
    pub variants: Vec<Variant>,
    pub pref: BrandPref,
}

/// Resolution stops counting past this smaller side: a logo is drawn at 32 px
/// at most (TAXO§10), 96 on a 3x screen. Measured on the dev library: without
/// the cap, a lone 4096 px Nissan badge beat the one seven cars share - the
/// vote the spec relies on never got a say.
const ENOUGH_PX: u32 = 128;

/// TAXO§4, in this order: transparent background, then resolution (up to
/// what a screen shows), then the most cars, then the first car id - two
/// launches give the same result. No recency rule, on purpose: the vote
/// adapts to the collection, a library of 1960s cars elects the period logo.
fn rank(variants: &mut [(Variant, String)]) {
    let res = |v: &Variant| v.width.min(v.height).min(ENOUGH_PX);
    variants.sort_by(|(a, ida), (b, idb)| {
        (b.background == Background::Transparent)
            .cmp(&(a.background == Background::Transparent))
            .then_with(|| res(b).cmp(&res(a)))
            .then_with(|| b.cars.cmp(&a.cars))
            .then_with(|| ida.cmp(idb))
    });
}

/// Every brand's logos, from `(car id, brand, badge path)` of the library.
/// Brands with a choice but no car left (TAXO§9: the curation survives) are
/// included, with their file if they have one.
pub fn elect(cars: &[(String, String, String)], prefs: &Prefs, dir: &Path) -> BTreeMap<String, BrandLogo> {
    let mut by_brand: BTreeMap<&str, BTreeMap<String, (Variant, String)>> = BTreeMap::new();
    let mut sorted: Vec<&(String, String, String)> = cars.iter().collect();
    sorted.sort_by(|a, b| a.0.cmp(&b.0));
    for (id, brand, path) in sorted {
        let Some(a) = analyze(Path::new(path)) else {
            continue;
        };
        let e = by_brand
            .entry(brand.as_str())
            .or_default()
            .entry(a.hash.clone())
            .or_insert_with(|| {
                (
                    Variant {
                        hash: a.hash.clone(),
                        path: path.clone(),
                        width: a.width,
                        height: a.height,
                        background: a.background,
                        cars: 0,
                    },
                    id.clone(),
                )
            });
        e.0.cars += 1;
    }
    let mut out = BTreeMap::new();
    let brands: Vec<String> = by_brand
        .keys()
        .map(|b| b.to_string())
        .chain(prefs.keys().cloned())
        .collect();
    for brand in brands {
        if out.contains_key(&brand) {
            continue;
        }
        let mut variants: Vec<(Variant, String)> = by_brand
            .get(brand.as_str())
            .map(|m| m.values().cloned().collect())
            .unwrap_or_default();
        rank(&mut variants);
        let variants: Vec<Variant> = variants.into_iter().map(|(v, _)| v).collect();
        let pref = prefs.get(&brand).cloned().unwrap_or_default();
        let custom = pref
            .custom
            .as_ref()
            .map(|f| dir.join(LOGOS_DIR).join(f))
            .filter(|p| p.is_file())
            .and_then(|p| analyze(&p).map(|a| (p, a)));
        let picked = pref
            .variant
            .as_ref()
            .and_then(|h| variants.iter().find(|v| &v.hash == h));
        let (path, background, choice) = match (&custom, picked, variants.first()) {
            (Some((p, a)), _, _) => (Some(p.to_string_lossy().into_owned()), Some(a.background), "custom"),
            (None, Some(v), _) => (Some(v.path.clone()), Some(v.background), "variant"),
            (None, None, Some(v)) => (Some(v.path.clone()), Some(v.background), "auto"),
            (None, None, None) => (None, None, "auto"),
        };
        let plaque = pref.plaque.unwrap_or(background == Some(Background::Baked));
        out.insert(
            brand.clone(),
            BrandLogo {
                brand,
                path,
                plaque,
                choice,
                variants,
                pref,
            },
        );
    }
    out
}

/// What the application needs of the logos: each brand's (TAXO§4), and which
/// car badges go on the light plate - a property of the file, wherever it is
/// shown (TAXO§5): the Kunos Porsche badge stays the car's, and is drawn right.
#[derive(Debug, Clone, Serialize)]
pub struct LogosView {
    pub brands: BTreeMap<String, BrandLogo>,
    pub plaque: Vec<String>,
}

pub fn view(cars: &[(String, String, String)], dir: &Path) -> LogosView {
    let brands = elect(cars, &load_prefs(dir), dir);
    let mut plaque: Vec<String> = cars
        .iter()
        .filter(|(_, _, p)| analyze(Path::new(p)).is_some_and(|a| a.background == Background::Baked))
        .map(|(_, _, p)| p.clone())
        .collect();
    plaque.sort();
    plaque.dedup();
    LogosView { brands, plaque }
}

#[cfg(test)]
mod tests {
    use super::*;
    use image::{Rgba, RgbaImage};

    fn square(bg: [u8; 4]) -> RgbaImage {
        let mut img = RgbaImage::from_pixel(64, 64, Rgba(bg));
        for y in 16..48 {
            for x in 16..48 {
                img.put_pixel(x, y, Rgba([200, 20, 20, 255]));
            }
        }
        img
    }

    /// TAXO§4: the border ring says the background - see-through, a flat
    /// baked colour (the Kunos Porsche's white square), or a drawing that
    /// reaches the edge.
    #[test]
    fn the_border_ring_tells_the_background() {
        assert_eq!(classify(&square([0, 0, 0, 0])), Background::Transparent);
        assert_eq!(classify(&square([255, 255, 255, 255])), Background::Baked);
        let mut noisy = square([250, 250, 250, 255]);
        noisy.put_pixel(0, 0, Rgba([240, 244, 238, 255])); // JPEG noise: still flat
        assert_eq!(classify(&noisy), Background::Baked);
        let mut drawn = RgbaImage::new(64, 64);
        for (x, y, p) in drawn.enumerate_pixels_mut() {
            *p = Rgba([(x * 4) as u8, (y * 4) as u8, 128, 255]);
        }
        assert_eq!(
            classify(&drawn),
            Background::Opaque,
            "a gradient to the edge is no baked square"
        );
    }

    fn write(dir: &Path, name: &str, img: &RgbaImage) -> String {
        let p = dir.join(name);
        img.save(&p).unwrap();
        p.to_string_lossy().into_owned()
    }

    /// TAXO§4: transparent first - even outvoted - then resolution, then
    /// votes; a pick and a file of his override; the plate follows the chosen
    /// file unless he forced it.
    #[test]
    fn the_election_prefers_transparency_then_size_then_votes() {
        let base = crate::testutil::temp_dir("logos-elect");
        let white = write(&base, "white.png", &square([255, 255, 255, 255]));
        let clear = write(&base, "clear.png", &square([0, 0, 0, 0]));
        let cars = vec![
            ("ks_porsche_911".to_string(), "Porsche".to_string(), white.clone()),
            ("ks_porsche_718".to_string(), "Porsche".to_string(), white.clone()),
            ("rss_porsche_gt".to_string(), "Porsche".to_string(), clear.clone()),
        ];
        let auto = elect(&cars, &Prefs::new(), &base);
        let p = &auto["Porsche"];
        assert_eq!(p.path.as_deref(), Some(clear.as_str()), "transparent beats two votes");
        assert!(!p.plaque);
        assert_eq!(p.variants.len(), 2, "two files, two variants");
        assert_eq!(p.variants[1].cars, 2);

        let mut prefs = Prefs::new();
        prefs.insert(
            "Porsche".into(),
            BrandPref {
                variant: Some(p.variants[1].hash.clone()),
                ..Default::default()
            },
        );
        let picked = &elect(&cars, &prefs, &base)["Porsche"];
        assert_eq!(picked.path.as_deref(), Some(white.as_str()));
        assert!(picked.plaque, "a baked background is drawn on the plate");
        prefs.get_mut("Porsche").unwrap().plaque = Some(false);
        assert!(!elect(&cars, &prefs, &base)["Porsche"].plaque, "his override stands");

        // Past what a screen shows, size stops counting and the vote decides.
        let mut big = RgbaImage::new(512, 512);
        big.put_pixel(256, 256, Rgba([1, 2, 3, 255]));
        let big = write(&base, "big.png", &big);
        let mut small = RgbaImage::new(128, 128);
        small.put_pixel(64, 64, Rgba([1, 2, 3, 255]));
        let small = write(&base, "small.png", &small);
        let nissan = vec![
            ("a".to_string(), "Nissan".to_string(), big),
            ("b".to_string(), "Nissan".to_string(), small.clone()),
            ("c".to_string(), "Nissan".to_string(), small.clone()),
        ];
        assert_eq!(
            elect(&nissan, &Prefs::new(), &base)["Nissan"].path,
            Some(small),
            "two votes beat a lone 512 px"
        );
    }

    /// TAXO§9: a file of his is copied into Pit Box's folder, too small is
    /// refused, and it survives the brand losing its last car.
    #[test]
    fn a_logo_of_his_is_copied_and_survives_its_cars() {
        let base = crate::testutil::temp_dir("logos-custom");
        let small = write(&base, "small.png", &RgbaImage::new(32, 32));
        assert_eq!(
            import_file(&base, "Alfa Romeo", Path::new(&small)).unwrap_err(),
            crate::errors::LOGO_TOO_SMALL
        );
        let good = write(&base, "good.png", &square([0, 0, 0, 0]));
        let name = import_file(&base, "Alfa Romeo", Path::new(&good)).unwrap();
        assert!(name.starts_with("alfa-romeo-") && base.join(LOGOS_DIR).join(&name).is_file());
        let mut prefs = Prefs::new();
        prefs.insert(
            "Alfa Romeo".into(),
            BrandPref {
                custom: Some(name),
                ..Default::default()
            },
        );
        save_prefs(&base, &prefs).unwrap();
        let logos = elect(&[], &load_prefs(&base), &base);
        assert_eq!(logos["Alfa Romeo"].choice, "custom", "no car left, his logo stays");

        // Back to automatic: his file goes, it was ours to remove.
        let file = base.join(LOGOS_DIR).join(prefs["Alfa Romeo"].custom.as_ref().unwrap());
        set_pref(&base, "Alfa Romeo", BrandPref::default()).unwrap();
        assert!(!file.exists(), "the unused file is removed");
        assert!(load_prefs(&base).is_empty());
        std::fs::write(base.join("keep.txt"), "x").unwrap();
        remove_file(&base, "../keep.txt");
        assert!(base.join("keep.txt").is_file(), "never a path outside logos/");
    }

    /// The detection on a real library, read-only: per brand, its variants
    /// and their verdicts, and the time it takes. What the thresholds are
    /// checked against (`--nocapture` to read). Paths are never printed:
    /// the output may be pasted in an issue.
    #[test]
    #[ignore = "reads the Pit Box configuration and library of this machine"]
    fn real_install_logos() {
        let src = std::env::var_os("PITBOX_CONFIG_DIR")
            .map(std::path::PathBuf::from)
            .or_else(|| std::env::var_os("APPDATA").map(|d| Path::new(&d).join("com.pitbox.app")))
            .expect("PITBOX_CONFIG_DIR or APPDATA");
        let work = crate::testutil::temp_dir("real-logos");
        std::fs::copy(src.join("config.json"), work.join("config.json")).unwrap();
        std::fs::copy(src.join("overlay.sqlite"), work.join("overlay.sqlite")).unwrap();
        let cfg: crate::config::AppConfig =
            serde_json::from_str(&std::fs::read_to_string(work.join("config.json")).unwrap()).unwrap();
        let conn = crate::overlay::open(&work.join("overlay.sqlite")).unwrap();
        let cars = crate::library::car_badges(&conn, &cfg).unwrap();
        let t = std::time::Instant::now();
        let v = view(&cars, &work);
        println!(
            "{} badges, {} brands, first pass {:?}",
            cars.len(),
            v.brands.len(),
            t.elapsed()
        );
        let t = std::time::Instant::now();
        let _ = view(&cars, &work);
        println!("cached pass {:?}", t.elapsed());
        println!("{} badges on the plate", v.plaque.len());
        for (brand, b) in &v.brands {
            let vs: Vec<String> = b
                .variants
                .iter()
                .map(|x| format!("{:?} {}x{} ×{}", x.background, x.width, x.height, x.cars))
                .collect();
            println!("{brand:<24} plaque={:<5} {}", b.plaque, vs.join(" | "));
        }
    }
}
