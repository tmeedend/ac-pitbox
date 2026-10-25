//! Where an install takes a package's items from (SPEC-story-mode, Content:
//! installation and updates): a `.pitstory` on disk, or the URL of a
//! manifest, its items in the same folder of the same host - any host, no
//! list of them. Either way, an item lands in a file of the staging folder
//! only once its size and its SHA-256 are the manifest's.

use std::fs::File;
use std::io::{Read, Write};
use std::path::Path;
use std::sync::atomic::{AtomicBool, Ordering};

use sha2::{Digest, Sha256};
use story_engine::package::{self, Item, ItemKind, Manifest};

use crate::errors;

/// A manifest is a few lines; anything bigger is not one.
const MANIFEST_LIMIT: usize = 1024 * 1024;
/// A host that stops sending for this long has dropped the download.
const TIMEOUT_MS: i32 = 60_000;

/// Set to stop the download under way (`cancel`).
static CANCEL: AtomicBool = AtomicBool::new(false);

/// Stops the download under way: the install ends, nothing changed.
pub fn cancel() {
    CANCEL.store(true, Ordering::Relaxed);
}

/// Where a package is read from.
#[derive(Debug, Clone, Copy)]
pub enum Origin<'a> {
    /// A `.pitstory` file.
    File(&'a Path),
    /// The URL of a package's `manifest.json`.
    Url(&'a str),
}

/// Told the bytes received so far and the bytes to receive in all; `false`
/// stops the download.
pub type Progress<'a> = &'a mut dyn FnMut(u64, u64) -> bool;

/// The items of one package, as they are fetched.
pub(super) struct Fetcher<'a> {
    zip: Option<zip::ZipArchive<File>>,
    /// The manifest's URL, the items' beside it.
    base: Option<String>,
    progress: Progress<'a>,
    done: u64,
    /// What this install downloads in all, once it knows (`set_total`).
    total: u64,
}

impl<'a> Fetcher<'a> {
    /// Opens `origin` and reads its manifest.
    pub fn open(origin: Origin, progress: Progress<'a>) -> Result<(Fetcher<'a>, Manifest), String> {
        CANCEL.store(false, Ordering::Relaxed);
        let mut fetcher = Fetcher {
            zip: None,
            base: None,
            progress,
            done: 0,
            total: 0,
        };
        let manifest = match origin {
            Origin::File(file) => {
                let mut zip = (File::open(file).map_err(|e| e.to_string()))
                    .and_then(|f| zip::ZipArchive::new(f).map_err(|e| e.to_string()))
                    .map_err(|e| {
                        log::warn!("story package {}: not a zip: {e}", file.display());
                        errors::STORY_PACKAGE_UNREADABLE.to_string()
                    })?;
                let mut text = String::new();
                let read = (zip.by_name(package::MANIFEST).map_err(|e| e.to_string()))
                    .and_then(|e| (e.take(MANIFEST_LIMIT as u64).read_to_string(&mut text)).map_err(|e| e.to_string()));
                if let Err(e) = read {
                    log::warn!("story package {}: no manifest: {e}", file.display());
                    return Err(errors::STORY_PACKAGE_UNREADABLE.into());
                }
                fetcher.zip = Some(zip);
                parse_manifest(&text, errors::STORY_PACKAGE_UNREADABLE)?
            }
            Origin::Url(url) => {
                let manifest = manifest_at(url)?;
                fetcher.base = Some(url.to_string());
                manifest
            }
        };
        Ok((fetcher, manifest))
    }

    /// Says how much this install downloads in all, for the progress.
    pub fn set_total(&mut self, total: u64) {
        self.total = total;
    }

    /// Whether this package's items come from the network.
    pub fn downloads(&self) -> bool {
        self.base.is_some()
    }

    /// The item into the file `to`, checked against the manifest.
    pub fn fetch(&mut self, item: &Item, to: &Path) -> Result<(), String> {
        if let Some(zip) = &mut self.zip {
            return copy_checked(zip, item, to);
        }
        let base = self.base.as_deref().unwrap_or_default();
        let url = item_url(base, &item.file);
        let (done, total) = (self.done, self.total);
        let progress = &mut self.progress;
        let got = crate::http::download(&url, to, &crate::cup::user_agent(), TIMEOUT_MS, &mut |received, _| {
            progress(done + received, total) && !CANCEL.load(Ordering::Relaxed)
        });
        match got {
            Ok(_) => {
                self.done += item.size;
                check_file(to, item)
            }
            Err(crate::http::DownloadError::Cancelled) => Err(errors::STORY_PACKAGE_CANCELLED.into()),
            Err(e) => {
                log::warn!("story package: {url} not downloaded: {e:?}");
                Err(errors::STORY_PACKAGE_DOWNLOAD_FAILED.into())
            }
        }
    }
}

/// The manifest at `url`.
pub(super) fn manifest_at(url: &str) -> Result<Manifest, String> {
    let response = crate::http::get_url(url, &crate::cup::user_agent(), TIMEOUT_MS, MANIFEST_LIMIT);
    match response {
        Some(r) if r.status == 200 => {
            let text = String::from_utf8_lossy(&r.body);
            parse_manifest(&text, errors::STORY_LINK_NOT_A_PACKAGE)
        }
        Some(r) => {
            log::warn!("story package: {url} answered {}", r.status);
            Err(errors::STORY_PACKAGE_DOWNLOAD_FAILED.into())
        }
        None => Err(errors::STORY_PACKAGE_DOWNLOAD_FAILED.into()),
    }
}

/// A manifest's text, `unreadable` the error when it is not one.
fn parse_manifest(text: &str, unreadable: &'static str) -> Result<Manifest, String> {
    // The format first: a newer manifest may well not read here at all.
    let format = serde_json::from_str::<serde_json::Value>(text)
        .ok()
        .and_then(|v| v.get("format")?.as_u64());
    if format.is_some_and(|f| f > u64::from(package::FORMAT)) {
        return Err(errors::STORY_PACKAGE_TOO_NEW.into());
    }
    let manifest = Manifest::from_json(text).map_err(|e| {
        log::warn!("story package: {e}");
        unreadable.to_string()
    })?;
    if manifest.items.iter().any(|i| i.kind == ItemKind::Unknown) {
        return Err(errors::STORY_PACKAGE_TOO_NEW.into());
    }
    let plain = |file: &str| !file.is_empty() && !file.starts_with('.') && !file.contains(['/', '\\', ':']);
    if !package::valid_id(&manifest.id) || !manifest.items.iter().all(|i| plain(&i.file)) {
        log::warn!(
            "story package {}: an id or an item's file cannot name a file",
            manifest.id
        );
        return Err(unreadable.into());
    }
    Ok(manifest)
}

/// An item's URL: its file name in the manifest's folder.
fn item_url(manifest_url: &str, file: &str) -> String {
    let folder = manifest_url
        .split(['?', '#'])
        .next()
        .and_then(|u| u.rfind('/').map(|i| &u[..=i]))
        .unwrap_or(manifest_url);
    format!("{folder}{}", crate::http::encode_query_value(file))
}

/// The item's file copied out of the `.pitstory` to `to`, refused unless it
/// has the size and the fingerprint the manifest gives it.
fn copy_checked(zip: &mut zip::ZipArchive<File>, item: &Item, to: &Path) -> Result<(), String> {
    let mut entry = (zip.by_name(&format!("{}{}", package::ITEMS, item.file))).map_err(|e| damaged(item, e))?;
    let mut out = File::create(to).map_err(|e| e.to_string())?;
    let mut hasher = Sha256::new();
    let mut size = 0u64;
    let mut buf = vec![0u8; 64 * 1024];
    loop {
        let n = entry.read(&mut buf).map_err(|e| damaged(item, e))?;
        if n == 0 {
            break;
        }
        size += n as u64;
        if size > item.size {
            return Err(damaged(item, format!("longer than its {} bytes", item.size)));
        }
        hasher.update(&buf[..n]);
        out.write_all(&buf[..n]).map_err(|e| e.to_string())?;
    }
    same(item, size, package::hex(&hasher.finalize()))
}

/// Whether the file `path` is the item the manifest names.
pub(super) fn check_file(path: &Path, item: &Item) -> Result<(), String> {
    let (size, sha256) = (File::open(path).and_then(package::sha256_of)).map_err(|e| damaged(item, e))?;
    same(item, size, sha256)
}

fn same(item: &Item, size: u64, sha256: String) -> Result<(), String> {
    if size != item.size {
        return Err(damaged(item, format!("{size} bytes, {} declared", item.size)));
    }
    if sha256 != item.sha256 {
        return Err(damaged(item, format!("sha256 {sha256}, {} declared", item.sha256)));
    }
    Ok(())
}

fn damaged(item: &Item, why: impl std::fmt::Display) -> String {
    log::warn!("story package: item {}: {why}", item.file);
    errors::STORY_PACKAGE_DAMAGED.to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    // Items live beside their manifest, whatever the host: a release's
    // download folder, a plain web folder, a link with a query.
    #[test]
    fn an_item_is_fetched_beside_its_manifest() {
        assert_eq!(
            item_url("https://example.org/stories/race/manifest.json", "story-ab.zip"),
            "https://example.org/stories/race/story-ab.zip"
        );
        assert_eq!(
            item_url("https://host/r/latest/download/manifest.json?dl=1", "car-x-1.zip"),
            "https://host/r/latest/download/car-x-1.zip"
        );
        assert_eq!(
            item_url("https://h/m.json", "car-my car-1.zip"),
            "https://h/car-my%20car-1.zip"
        );
    }

    #[test]
    fn a_manifest_naming_a_path_is_refused() {
        let text = |file: &str| {
            format!(
                r#"{{"format":1,"id":"a","title":"A","version":"1",
                "items":[{{"kind":"story","file":"{file}","size":1,"sha256":"00"}}]}}"#
            )
        };
        assert!(parse_manifest(&text("story-1.zip"), "x").is_ok());
        for bad in ["../evil.zip", "a/b.zip", ".hidden", ""] {
            assert_eq!(parse_manifest(&text(bad), "x"), Err("x".to_string()), "{bad}");
        }
        assert_eq!(
            parse_manifest(r#"{"format":2}"#, "x"),
            Err(errors::STORY_PACKAGE_TOO_NEW.to_string())
        );
        assert_eq!(parse_manifest("<html>", "x"), Err("x".to_string()), "a web page");
    }
}
