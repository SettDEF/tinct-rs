//! Image colorfulness (Hasler–Süsstrunk) and automatic scheme selection.
//! Port of `scheme_for_image.py` without the OpenCV dependency.

use crate::scheme::SchemeVariant;

/// Hasler–Süsstrunk colorfulness over RGB pixels:
/// `sqrt(std_rg² + std_yb²) + 0.3·sqrt(mean_rg² + mean_yb²)`
/// where rg = |R−G| and yb = |(R+G)/2 − B|.
pub fn colorfulness_rgb(pixels: impl Iterator<Item = (f64, f64, f64)>) -> f64 {
    let mut n = 0f64;
    let (mut sum_rg, mut sum_rg2) = (0f64, 0f64);
    let (mut sum_yb, mut sum_yb2) = (0f64, 0f64);
    for (r, g, b) in pixels {
        let rg = (r - g).abs();
        let yb = (0.5 * (r + g) - b).abs();
        n += 1.0;
        sum_rg += rg;
        sum_rg2 += rg * rg;
        sum_yb += yb;
        sum_yb2 += yb * yb;
    }
    if n == 0.0 {
        return 0.0;
    }
    let mean_rg = sum_rg / n;
    let mean_yb = sum_yb / n;
    // Population std (ddof=0), matching numpy's default.
    let var_rg = (sum_rg2 / n - mean_rg * mean_rg).max(0.0);
    let var_yb = (sum_yb2 / n - mean_yb * mean_yb).max(0.0);
    (var_rg + var_yb).sqrt() + 0.3 * (mean_rg * mean_rg + mean_yb * mean_yb).sqrt()
}

/// scheme_for_image.py policy: dull images get scheme-neutral, everything
/// else scheme-tonal-spot.
pub fn pick_scheme(colorfulness: f64) -> SchemeVariant {
    if colorfulness < 40.0 {
        SchemeVariant::Neutral
    } else {
        SchemeVariant::TonalSpot
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn gray_image_has_zero_colorfulness() {
        let px = std::iter::repeat((128.0, 128.0, 128.0)).take(100);
        assert!(colorfulness_rgb(px) < 1e-9);
    }

    #[test]
    fn saturated_mix_is_colorful() {
        let px = (0..100).map(|i| if i % 2 == 0 { (255.0, 0.0, 0.0) } else { (0.0, 0.0, 255.0) });
        assert!(colorfulness_rgb(px) > 40.0);
    }
}
