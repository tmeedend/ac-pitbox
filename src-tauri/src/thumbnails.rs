//! Cache de miniatures pour les galeries médias (§6.1).
//!
//! Les captures d'écran AC sont en pleine résolution jeu (plusieurs Mo,
//! parfois par dizaines dans `screens/`) et n'ont jamais été retaillées :
//! chaque ouverture de galerie redécodait l'original en entier pour
//! l'afficher en 150px, à chaque fois, y compris après redémarrage de l'app
//! (`media.rs` ne met explicitement rien en cache — c'est son choix pour les
//! métadonnées de rattachement, pas pour les pixels). Ce module génère une
//! miniature JPEG une seule fois par fichier et la persiste sur disque
//! (`app_cache_dir()/thumbnails/`), réutilisée telle quelle ensuite.
//!
//! Pas de politique d'éviction pour l'instant : le cache grossit avec le
//! nombre de captures vues, jamais purgé automatiquement — cohérent avec le
//! reste de l'app (l'overlay ne purge rien non plus), à reconsidérer si ça
//! devient gênant en pratique (bouton "vider le cache" dans Maintenance).

use std::path::{Path, PathBuf};

use sha2::{Digest, Sha256};
use tauri::{AppHandle, Manager};

fn cache_dir(app: &AppHandle) -> PathBuf {
    app.path().app_cache_dir().unwrap_or_default().join("thumbnails")
}

/// Hash du chemin source + sa date de modification + la taille cible : un
/// fichier remplacé (même nom, contenu différent) régénère sa miniature au
/// lieu de resservir l'ancienne.
fn cache_key(source: &Path, max_dim: u32) -> Option<String> {
    let meta = std::fs::metadata(source).ok()?;
    let modified = meta.modified().ok()?.duration_since(std::time::UNIX_EPOCH).ok()?;
    let mut hasher = Sha256::new();
    hasher.update(source.to_string_lossy().to_lowercase().as_bytes());
    hasher.update(b"|");
    hasher.update(modified.as_secs().to_le_bytes());
    hasher.update(b"|");
    hasher.update(max_dim.to_le_bytes());
    Some(format!("{:x}", hasher.finalize()))
}

/// Chemin d'une miniature JPEG de `source`, générée au besoin (décodage +
/// redimensionnement rapide qui préserve le ratio, `DynamicImage::thumbnail`).
/// Une source illisible ou dans un format non géré renvoie une erreur — à
/// charge de l'appelant (façade de commande) de la traiter comme "pas de
/// miniature" plutôt que de faire planter toute une galerie pour un fichier.
pub fn get_or_create(app: &AppHandle, source: &Path, max_dim: u32) -> Result<PathBuf, String> {
    let dir = cache_dir(app);
    std::fs::create_dir_all(&dir).map_err(|e| e.to_string())?;
    let key = cache_key(source, max_dim).ok_or_else(|| format!("fichier source introuvable : {}", source.display()))?;
    let cached = dir.join(format!("{key}.jpg"));
    if cached.is_file() {
        return Ok(cached);
    }

    let img = image::open(source).map_err(|e| e.to_string())?;
    let thumb = img.thumbnail(max_dim, max_dim);

    // Écriture atomique (fichier temporaire du PID courant, puis renommage) :
    // deux requêtes concurrentes pour la même image ne doivent jamais se
    // marcher dessus ni laisser de fichier à moitié écrit derrière une
    // lecture ratée.
    let tmp = dir.join(format!("{key}.{}.tmp", std::process::id()));
    thumb.save_with_format(&tmp, image::ImageFormat::Jpeg).map_err(|e| e.to_string())?;
    std::fs::rename(&tmp, &cached).map_err(|e| e.to_string())?;
    Ok(cached)
}

#[cfg(test)]
mod tests {
    use super::*;
    use image::{ImageBuffer, Rgb};

    fn write_test_png(path: &Path, w: u32, h: u32) {
        let img: ImageBuffer<Rgb<u8>, Vec<u8>> = ImageBuffer::from_fn(w, h, |x, y| {
            Rgb([(x % 256) as u8, (y % 256) as u8, 128])
        });
        img.save(path).unwrap();
    }

    #[test]
    fn generates_a_smaller_cached_thumbnail() {
        let dir = crate::testutil::temp_dir("thumbnails-generate");
        let source = dir.join("shot.png");
        write_test_png(&source, 800, 600);

        let cache_root = dir.join("cache");
        std::fs::create_dir_all(&cache_root).unwrap();
        let thumb_path = cache_root.join("thumbnails");
        std::fs::create_dir_all(&thumb_path).unwrap();
        let key = cache_key(&source, 150).expect("clé calculable pour un fichier existant");
        let cached = thumb_path.join(format!("{key}.jpg"));

        let img = image::open(&source).unwrap();
        let thumb = img.thumbnail(150, 150);
        thumb.save_with_format(&cached, image::ImageFormat::Jpeg).unwrap();

        assert!(cached.is_file(), "la miniature doit être écrite sur disque");
        assert!(thumb.width() <= 150 && thumb.height() <= 150, "la miniature respecte la dimension max");
        let original_size = std::fs::metadata(&source).unwrap().len();
        let thumb_size = std::fs::metadata(&cached).unwrap().len();
        assert!(thumb_size < original_size, "la miniature doit peser moins que l'original");
    }

    #[test]
    fn cache_key_changes_with_max_dim_and_mtime() {
        let dir = crate::testutil::temp_dir("thumbnails-key");
        let source = dir.join("shot.png");
        write_test_png(&source, 100, 100);

        let key_150 = cache_key(&source, 150).unwrap();
        let key_320 = cache_key(&source, 320).unwrap();
        assert_ne!(key_150, key_320, "deux tailles cibles différentes -> deux fichiers de cache différents");

        let key_again = cache_key(&source, 150).unwrap();
        assert_eq!(key_150, key_again, "même fichier, même taille -> même clé (cache réutilisé)");
    }

    #[test]
    fn cache_key_is_none_for_missing_source() {
        assert!(cache_key(Path::new(r"Z:\does-not-exist-pitbox.png"), 150).is_none());
    }
}
