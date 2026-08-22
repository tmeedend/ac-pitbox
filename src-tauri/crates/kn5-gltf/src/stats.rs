//! Per-channel measurement of a texture, for format investigation only.
//!
//! Nothing here is used by [`crate::convert`]. It exists so that a campaign on
//! an undocumented field can be **rejouée** rather than believed on its word:
//! `docs/kn5-format.md` records what AC really puts in its maps, and every
//! entry of that file is supposed to carry the method that produced it. The
//! green channel of `txMaps` was settled that way (écart n°7); R and B were
//! not, and this is the instrument for the second half.
//!
//! Driven by `kn5-tool maps`, never by the application.

use image::RgbaImage;

/// What one texture says about itself, channel by channel.
#[derive(Debug, Clone)]
pub struct ChannelStats {
    pub width: u32,
    pub height: u32,
    /// Mean of each channel, on the 0-255 scale the authors think in.
    pub mean: [f32; 4],
    /// Standard deviation per channel. The figure that decides whether a mean
    /// means anything: an atlas serving several finishes has a wide spread,
    /// and its average describes none of them (écart n°7).
    pub stddev: [f32; 4],
    pub min: [u8; 4],
    pub max: [u8; 4],
    /// Share of pixels where R and B hold exactly the same value. The
    /// observation that made the two look like a single quantity written
    /// twice — and the one that has to be checked before believing it.
    pub rb_equal: f32,
    /// Share of pixels saturated on R, G and B at once: `NULL.dds`, four white
    /// pixels meaning "nothing to say about this surface", scores 1.0 here.
    pub white: f32,
    /// Pearson correlation of R and B with the green channel, and of R with B.
    ///
    /// The measurement that separates a channel carrying a meaning from a
    /// channel left to whatever the exporter put there. If R were, say, a
    /// metallic mask, it would track the gloss closely — metal is glossy — and
    /// the figure would be high on every car. A channel the shader ignores
    /// has no reason to track anything, and its correlation wanders from one
    /// author to the next.
    pub corr_rg: f32,
    pub corr_bg: f32,
    pub corr_rb: f32,
}

/// Decodes a texture blob and measures it. Same decoder as the conversion, so
/// a surprising figure cannot be blamed on a second, divergent DDS reader.
pub fn channel_stats(blob: &[u8]) -> Result<ChannelStats, String> {
    Ok(measure(&crate::texture::decode(blob)?))
}

fn measure(image: &RgbaImage) -> ChannelStats {
    let mut sum = [0f64; 4];
    let mut sum_sq = [0f64; 4];
    let mut min = [u8::MAX; 4];
    let mut max = [0u8; 4];
    let mut rb_equal = 0u64;
    let mut white = 0u64;
    // Produits croisés, pour les corrélations : une seule passe, comme la
    // variance ci-dessus.
    let mut cross = [0f64; 3]; // RG, BG, RB

    for pixel in image.pixels() {
        for channel in 0..4 {
            let value = pixel.0[channel];
            sum[channel] += f64::from(value);
            sum_sq[channel] += f64::from(value) * f64::from(value);
            min[channel] = min[channel].min(value);
            max[channel] = max[channel].max(value);
        }
        cross[0] += f64::from(pixel.0[0]) * f64::from(pixel.0[1]);
        cross[1] += f64::from(pixel.0[2]) * f64::from(pixel.0[1]);
        cross[2] += f64::from(pixel.0[0]) * f64::from(pixel.0[2]);
        if pixel.0[0] == pixel.0[2] {
            rb_equal += 1;
        }
        if pixel.0[0] == 255 && pixel.0[1] == 255 && pixel.0[2] == 255 {
            white += 1;
        }
    }

    let count = (image.width() as f64 * image.height() as f64).max(1.0);
    let mut mean = [0f32; 4];
    let mut stddev = [0f32; 4];
    for channel in 0..4 {
        let m = sum[channel] / count;
        mean[channel] = m as f32;
        // Variance par la somme des carrés : les textures montent à un million
        // de pixels, et deux passes coûteraient un second parcours pour une
        // précision dont une statistique de format n'a pas besoin.
        stddev[channel] = ((sum_sq[channel] / count - m * m).max(0.0)).sqrt() as f32;
    }

    // Corrélation de Pearson à partir des sommes déjà accumulées. Un canal
    // constant n'a pas de corrélation définie (dénominateur nul) : on renvoie
    // 0, ce qui se lit « aucun lien mesurable » et non « lien nul mesuré ».
    let correlation = |a: usize, b: usize, cross: f64| {
        let cov = cross / count - (sum[a] / count) * (sum[b] / count);
        let spread = f64::from(stddev[a]) * f64::from(stddev[b]);
        if spread <= f64::EPSILON {
            0.0
        } else {
            (cov / spread) as f32
        }
    };

    ChannelStats {
        width: image.width(),
        height: image.height(),
        mean,
        stddev,
        min,
        max,
        rb_equal: (rb_equal as f64 / count) as f32,
        white: (white as f64 / count) as f32,
        corr_rg: correlation(0, 1, cross[0]),
        corr_bg: correlation(2, 1, cross[1]),
        corr_rb: correlation(0, 2, cross[2]),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Règle : la mesure décrit bien le contenu — moyenne, dispersion et la
    /// part de pixels où R égale B, qui est le signal sur lequel repose la
    /// question ouverte de `txMaps` (docs/kn5-format.md, écart n°7).
    #[test]
    fn measures_mean_spread_and_red_blue_equality() {
        // Deux pixels : l'un R=B, l'autre non. Vert à 0 puis 200.
        let mut image = RgbaImage::new(2, 1);
        image.put_pixel(0, 0, image::Rgba([10, 0, 10, 255]));
        image.put_pixel(1, 0, image::Rgba([10, 200, 250, 255]));

        let stats = measure(&image);
        assert_eq!(stats.mean[1], 100.0, "moyenne du vert");
        assert_eq!(stats.stddev[0], 0.0, "un canal constant n'a aucune dispersion");
        assert_eq!(stats.stddev[1], 100.0, "dispersion du vert");
        assert_eq!(stats.min[2], 10, "minimum du bleu");
        assert_eq!(stats.max[2], 250, "maximum du bleu");
        assert_eq!(stats.rb_equal, 0.5, "un pixel sur deux porte R == B");
        assert_eq!(stats.white, 0.0, "aucun pixel saturé sur les trois canaux");
        assert_eq!(stats.corr_rg, 0.0, "un canal constant n'a aucune corrélation définie");
        assert!(
            stats.corr_bg > 0.99,
            "bleu et vert montent ensemble sur ces deux pixels"
        );
    }

    /// Règle : `NULL.dds` — la texture blanche qui veut dire « rien à dire sur
    /// cette surface » — est reconnaissable à sa part de pixels saturés, pas
    /// au seul canal vert (écart n°7, second garde-fou).
    #[test]
    fn a_fully_white_texture_is_reported_as_such() {
        let image = RgbaImage::from_pixel(2, 2, image::Rgba([255, 255, 255, 255]));
        let stats = measure(&image);
        assert_eq!(stats.white, 1.0, "tous les pixels sont saturés sur RVB");
        assert_eq!(stats.rb_equal, 1.0, "R et B y sont trivialement égaux");
    }
}
