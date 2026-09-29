//! Extraction d'archives (zip/rar/7z) via le 7-Zip configuré, et déplacement
//! de dossiers entre volumes — porté de la logique de `common.py`.

use std::io::Read;
use std::path::Path;
use std::process::{Command, Stdio};

#[cfg(windows)]
use std::os::windows::process::CommandExt;

/// Évite l'ouverture d'une fenêtre de console à chaque appel 7-Zip.
#[cfg(windows)]
const CREATE_NO_WINDOW: u32 = 0x0800_0000;

/// 7-Zip exit code for a command line it does not understand. Used to tell an
/// unsupported `-bsp1` (added in 7-Zip 15.06) apart from a real extraction
/// failure, so an old binary degrades to a mute progress bar instead of
/// breaking import outright.
const SEVENZIP_BAD_COMMAND_LINE: i32 = 7;

/// Extrait `archive` dans `dest` via `7z x`. `dest` doit exister.
pub fn extract(sevenzip: &Path, archive: &Path, dest: &Path) -> Result<(), String> {
    extract_with_progress(sevenzip, archive, dest, &|_| {}, &|| false)
}

/// Same as [`extract`], reporting 7-Zip's own completion percentage as it goes
/// and honouring a cancellation request.
///
/// `-bsp1` redirects the progress indicator to stdout (it goes to the console
/// otherwise, i.e. nowhere here). 7-Zip separates those updates with carriage
/// returns rather than newlines — reading whole lines would therefore block
/// until the very end, which is exactly the mute bar this replaces.
///
/// stderr is drained by a dedicated thread: reading both pipes from one thread
/// deadlocks as soon as the one we are *not* reading fills its buffer, and a
/// 7-Zip that warns on every file fills stderr quickly.
pub fn extract_with_progress(
    sevenzip: &Path,
    archive: &Path,
    dest: &Path,
    on_percent: &(dyn Fn(u8) + Sync),
    cancelled: &(dyn Fn() -> bool + Sync),
) -> Result<(), String> {
    match run_extract(sevenzip, archive, dest, true, on_percent, cancelled) {
        // The binary predates `-bsp1`: same extraction, no progress.
        Err(ExtractError::BadCommandLine) => {
            run_extract(sevenzip, archive, dest, false, on_percent, cancelled).map_err(|e| e.into_message())
        }
        Err(e) => Err(e.into_message()),
        Ok(()) => Ok(()),
    }
}

enum ExtractError {
    BadCommandLine,
    Cancelled,
    Failed(String),
}

impl ExtractError {
    fn into_message(self) -> String {
        match self {
            ExtractError::Cancelled => crate::errors::IMPORT_CANCELLED.to_string(),
            ExtractError::Failed(m) => m,
            // Reported as a plain failure: the caller already retried without
            // the progress switch, so reaching here means something else broke.
            ExtractError::BadCommandLine => "7-Zip : ligne de commande refusée".to_string(),
        }
    }
}

fn run_extract(
    sevenzip: &Path,
    archive: &Path,
    dest: &Path,
    with_progress: bool,
    on_percent: &(dyn Fn(u8) + Sync),
    cancelled: &(dyn Fn() -> bool + Sync),
) -> Result<(), ExtractError> {
    let mut cmd = Command::new(sevenzip);
    cmd.arg("x").arg(archive).arg(format!("-o{}", dest.display())).arg("-y");
    if with_progress {
        cmd.arg("-bsp1");
    }
    cmd.stdout(Stdio::piped()).stderr(Stdio::piped()).stdin(Stdio::null());
    #[cfg(windows)]
    cmd.creation_flags(CREATE_NO_WINDOW);

    let mut child = cmd
        .spawn()
        .map_err(|e| ExtractError::Failed(format!("impossible de lancer 7-Zip : {e}")))?;
    let mut stdout = child.stdout.take().expect("stdout piped");
    let mut stderr_pipe = child.stderr.take().expect("stderr piped");
    let stderr_thread = std::thread::spawn(move || {
        let mut buf = String::new();
        let _ = stderr_pipe.read_to_string(&mut buf);
        buf
    });

    let mut chunk = [0u8; 512];
    let mut pending = String::new();
    let mut killed = false;
    loop {
        if cancelled() && !killed {
            let _ = child.kill();
            killed = true;
        }
        match stdout.read(&mut chunk) {
            Ok(0) => break,
            Ok(n) => {
                pending.push_str(&String::from_utf8_lossy(&chunk[..n]));
                // Keep only the tail after the last separator: everything before
                // it is a complete update, and only the most recent one matters.
                if let Some(cut) = pending.rfind(['\r', '\n']) {
                    let complete = pending[..cut].to_string();
                    pending = pending[cut + 1..].to_string();
                    if let Some(pct) = complete.rsplit(['\r', '\n']).find_map(parse_percent) {
                        on_percent(pct);
                    }
                }
            }
            Err(_) => break,
        }
    }

    let status = child
        .wait()
        .map_err(|e| ExtractError::Failed(format!("7-Zip : attente du processus : {e}")))?;
    let stderr = stderr_thread.join().unwrap_or_default();
    if status.success() {
        return Ok(());
    }
    if killed || cancelled() {
        return Err(ExtractError::Cancelled);
    }
    if status.code() == Some(SEVENZIP_BAD_COMMAND_LINE) && with_progress {
        return Err(ExtractError::BadCommandLine);
    }
    Err(ExtractError::Failed(format!(
        "7-Zip a échoué (code {:?}) : {}",
        status.code(),
        stderr.trim()
    )))
}

/// Percentage out of a single 7-Zip progress update (`" 45% 12 - file.dds"`).
/// `None` for every other line it prints (banner, file list, errors).
fn parse_percent(update: &str) -> Option<u8> {
    let trimmed = update.trim_start();
    let digits: String = trimmed.chars().take_while(|c| c.is_ascii_digit()).collect();
    if digits.is_empty() || !trimmed[digits.len()..].starts_with('%') {
        return None;
    }
    digits.parse::<u8>().ok().map(|p| p.min(100))
}

/// Crée une archive `.7z` à partir du **contenu** de `src_dir` (chemins relatifs
/// préservés), via `7z a`. Utilisé par l'export autonome (§10).
pub fn create_7z(sevenzip: &Path, src_dir: &Path, archive: &Path) -> Result<(), String> {
    if archive.exists() {
        let _ = std::fs::remove_file(archive);
    }
    let mut cmd = Command::new(sevenzip);
    cmd.current_dir(src_dir)
        .arg("a")
        .arg("-t7z")
        .arg(archive)
        .arg("*")
        .arg("-y");
    #[cfg(windows)]
    cmd.creation_flags(CREATE_NO_WINDOW);

    let output = cmd.output().map_err(|e| format!("impossible de lancer 7-Zip : {e}"))?;
    if output.status.success() {
        Ok(())
    } else {
        Err(format!(
            "7-Zip a échoué (code {:?}) : {}",
            output.status.code(),
            String::from_utf8_lossy(&output.stderr).trim()
        ))
    }
}

/// Octets copiés, signalés fichier par fichier. Permet à la barre de progression
/// d'avancer pendant la copie d'un mod de plusieurs Go, qui est autrement une
/// seule opération opaque (§4.2bis).
pub type BytesReport<'a> = dyn Fn(u64) + 'a;

/// Déplace un dossier : `rename` si même volume, sinon copie récursive + suppression.
pub fn move_dir(src: &Path, dst: &Path) -> std::io::Result<()> {
    move_dir_reported(src, dst, &|_| {})
}

/// Comme [`move_dir`], en signalant les octets copiés. Un `rename` réussi ne
/// signale rien : il est instantané, il n'y a pas de progression à montrer.
pub fn move_dir_reported(src: &Path, dst: &Path, on_bytes: &BytesReport) -> std::io::Result<()> {
    if let Some(parent) = dst.parent() {
        std::fs::create_dir_all(parent)?;
    }
    if std::fs::rename(src, dst).is_ok() {
        return Ok(());
    }
    copy_dir_reported(src, dst, on_bytes)?;
    std::fs::remove_dir_all(src)?;
    Ok(())
}

/// Copie récursive d'un dossier (fallback inter-volumes).
pub fn copy_dir(src: &Path, dst: &Path) -> std::io::Result<()> {
    copy_dir_reported(src, dst, &|_| {})
}

/// Comme [`copy_dir`], en signalant les octets de chaque fichier copié.
pub fn copy_dir_reported(src: &Path, dst: &Path, on_bytes: &BytesReport) -> std::io::Result<()> {
    std::fs::create_dir_all(dst)?;
    for entry in std::fs::read_dir(src)? {
        let entry = entry?;
        let from = entry.path();
        let to = dst.join(entry.file_name());
        if from.is_dir() {
            copy_dir_reported(&from, &to, on_bytes)?;
        } else {
            on_bytes(std::fs::copy(&from, &to)?);
        }
    }
    Ok(())
}

// --- Where a downloaded archive came from (ESPACE§8) ---------------------------

/// What Windows noted about a downloaded archive, reduced to what lasts.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct Origin {
    /// The site the download started from — its origin only, never a page.
    pub site: Option<String>,
    /// The archive's name as the site served it, before any renaming: the
    /// best key there is to find it again.
    pub file_name: Option<String>,
}

/// Reads the `Zone.Identifier` stream Windows (the browser, in fact) attaches
/// to a downloaded file. `None` when there is none — an archive extracted from
/// another, copied from another disk, or downloaded by a browser set otherwise
/// — which is not an error.
pub fn read_origin(path: &Path) -> Option<Origin> {
    let mut stream = path.as_os_str().to_owned();
    stream.push(":Zone.Identifier");
    let raw = std::fs::read(std::path::PathBuf::from(stream)).ok()?;
    parse_zone_identifier(&String::from_utf8_lossy(&raw))
}

/// The `[ZoneTransfer]` section, as measured on 2026-09-26 (ESPACE§8.1):
/// `ReferrerUrl` is the site (the browser cuts it to the origin as soon as
/// another domain serves the file), `HostUrl` a signed link that expires a
/// minute after the click — but whose `response-content-disposition`
/// carries the original file name. **No query parameter is ever kept**: they
/// are signed tokens, worthless a minute later.
fn parse_zone_identifier(text: &str) -> Option<Origin> {
    let mut in_section = false;
    let (mut referrer, mut host) = (None, None);
    for line in text.lines().map(str::trim) {
        if line.starts_with('[') {
            in_section = line.eq_ignore_ascii_case("[ZoneTransfer]");
            continue;
        }
        let Some((key, value)) = line.split_once('=').filter(|_| in_section) else {
            continue;
        };
        match key.trim() {
            k if k.eq_ignore_ascii_case("ReferrerUrl") => referrer = Some(value.trim().to_string()),
            k if k.eq_ignore_ascii_case("HostUrl") => host = Some(value.trim().to_string()),
            _ => {}
        }
    }
    let site = referrer.as_deref().and_then(web_origin);
    let file_name = host
        .as_deref()
        .and_then(|h| query_param(h, "response-content-disposition"))
        .and_then(|d| disposition_file_name(&d))
        .or_else(|| host.as_deref().and_then(last_segment));
    (site.is_some() || file_name.is_some()).then_some(Origin { site, file_name })
}

/// `https://www.overtake.gg/threads/x?y` → `https://www.overtake.gg/`.
fn web_origin(url: &str) -> Option<String> {
    let (scheme, rest) = url.split_once("://")?;
    if !scheme.eq_ignore_ascii_case("https") && !scheme.eq_ignore_ascii_case("http") {
        return None;
    }
    let host = rest.split(['/', '?', '#']).next().filter(|h| !h.is_empty())?;
    Some(format!("{}://{host}/", scheme.to_ascii_lowercase()))
}

/// One parameter of a URL's query, percent-decoded.
fn query_param(url: &str, name: &str) -> Option<String> {
    let query = url.split_once('?')?.1.split('#').next()?;
    query
        .split('&')
        .filter_map(|pair| pair.split_once('='))
        .find(|(k, _)| k.eq_ignore_ascii_case(name))
        .map(|(_, v)| percent_decode(v))
}

/// The file name of a `Content-Disposition` value: `filename*=UTF-8''…`
/// (RFC 6266, percent-encoded) wins over `filename="…"` or `filename=…`.
fn disposition_file_name(disposition: &str) -> Option<String> {
    let params: Vec<(&str, &str)> = disposition
        .split(';')
        .filter_map(|p| p.split_once('='))
        .map(|(k, v)| (k.trim(), v.trim()))
        .collect();
    let extended = params
        .iter()
        .find(|(k, _)| k.eq_ignore_ascii_case("filename*"))
        .and_then(|(_, v)| v.split_once("''"))
        .map(|(_, encoded)| percent_decode(encoded));
    let plain = params
        .iter()
        .find(|(k, _)| k.eq_ignore_ascii_case("filename"))
        .map(|(_, v)| v.trim_matches('"').to_string());
    extended.or(plain).filter(|n| !n.trim().is_empty())
}

/// The last segment of a URL's path, when it looks like a file.
fn last_segment(url: &str) -> Option<String> {
    let path = url.split(['?', '#']).next()?;
    let segment = percent_decode(path.rsplit('/').next()?);
    segment.contains('.').then_some(segment)
}

fn percent_decode(s: &str) -> String {
    let bytes = s.as_bytes();
    let mut out = Vec::with_capacity(bytes.len());
    let mut i = 0;
    while i < bytes.len() {
        let hex = bytes.get(i + 1..i + 3).and_then(|h| std::str::from_utf8(h).ok());
        match (bytes[i], hex.and_then(|h| u8::from_str_radix(h, 16).ok())) {
            (b'%', Some(b)) => {
                out.push(b);
                i += 3;
            }
            (c, _) => {
                out.push(c);
                i += 1;
            }
        }
    }
    String::from_utf8_lossy(&out).into_owned()
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Rule (ESPACE§8.1, ESPACE§9.4): the stream measured on a real download from
    /// overtake.gg gives the site and the original file name — and none of
    /// the signed link's parameters.
    #[test]
    fn a_real_zone_identifier_gives_the_site_and_the_file_name() {
        let text = "[ZoneTransfer]\r\nZoneId=3\r\n\
            ReferrerUrl=https://www.overtake.gg/\r\n\
            HostUrl=https://abc.r2.cloudflarestorage.com/attachments/123/456?\
            X-Amz-Content-Sha256=UNSIGNED-PAYLOAD&X-Amz-Credential=secret%2F20260926&\
            response-content-disposition=attachment%3B%20filename%3D%22camtool-v3.0.0-beta.3.zip%22&\
            X-Amz-Expires=60&X-Amz-Signature=deadbeef\r\n";
        let origin = parse_zone_identifier(text).expect("an origin");
        assert_eq!(origin.site.as_deref(), Some("https://www.overtake.gg/"));
        assert_eq!(origin.file_name.as_deref(), Some("camtool-v3.0.0-beta.3.zip"));
        let kept = format!("{origin:?}");
        assert!(
            !kept.contains("X-Amz") && !kept.contains("secret"),
            "no signed parameter kept"
        );
    }

    /// Rule (ESPACE§8.2): the RFC 6266 form wins, quotes go, and without a
    /// disposition the link's last segment names the file.
    #[test]
    fn the_file_name_is_read_in_every_form_the_sites_use() {
        assert_eq!(
            disposition_file_name("attachment; filename=\"a b.7z\"; filename*=UTF-8''%C3%A9t%C3%A9.7z").as_deref(),
            Some("été.7z"),
            "the extended form wins"
        );
        assert_eq!(
            disposition_file_name("attachment; filename=plain.zip").as_deref(),
            Some("plain.zip")
        );
        assert_eq!(
            parse_zone_identifier("[ZoneTransfer]\nHostUrl=https://cdn.example/files/my%20mod.rar?token=1")
                .unwrap()
                .file_name
                .as_deref(),
            Some("my mod.rar"),
            "the link's last segment, without its query"
        );
        assert_eq!(
            parse_zone_identifier("[ZoneTransfer]\nZoneId=3"),
            None,
            "nothing to keep"
        );
        assert_eq!(web_origin("file:///C:/x"), None, "never a local file");
    }

    /// Rule (ESPACE§8.2): the stream is read on the real file system, by name —
    /// and a file without one is not an error.
    #[test]
    fn the_stream_is_read_next_to_the_file() {
        let base = crate::testutil::temp_dir("zone-identifier");
        let archive = base.join("mod.7z");
        std::fs::write(&archive, b"7z").unwrap();
        assert_eq!(read_origin(&archive), None, "a file without the stream");
        let mut stream = archive.as_os_str().to_owned();
        stream.push(":Zone.Identifier");
        std::fs::write(
            std::path::PathBuf::from(stream),
            "[ZoneTransfer]\r\nReferrerUrl=https://www.racedepartment.com/downloads/x\r\n",
        )
        .unwrap();
        assert_eq!(
            read_origin(&archive).and_then(|o| o.site).as_deref(),
            Some("https://www.racedepartment.com/"),
            "the origin only, never the page"
        );
    }

    /// Règle : seule une mise à jour de progression 7-Zip donne un pourcentage.
    /// Tout le reste de ce que 7-Zip imprime (bannière, liste de fichiers) doit
    /// être ignoré, sans quoi la barre saute sur un nombre pris au hasard.
    #[test]
    fn parse_percent_reads_only_progress_updates() {
        assert_eq!(parse_percent("  0%"), Some(0), "pourcentage seul");
        assert_eq!(
            parse_percent(" 45% 12 - cars/rss_gtm/body.kn5"),
            Some(45),
            "avec fichier"
        );
        assert_eq!(parse_percent("100% 87"), Some(100), "fin d'extraction");
        assert_eq!(parse_percent("7-Zip 24.09 (x64)"), None, "bannière ignorée");
        assert_eq!(
            parse_percent("Extracting  cars/50%weird.dds"),
            None,
            "% en fin de ligne ignoré"
        );
        assert_eq!(parse_percent(""), None, "ligne vide ignorée");
    }
}
