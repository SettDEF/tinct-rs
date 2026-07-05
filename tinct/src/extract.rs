//! Seed color extraction: decode → resize → QuantizerCelebi → Score.
//! Port of the image path in `generate_colors_material.py`.

use material_colors::color::Argb;
use material_colors::hct::Hct;
use material_colors::quantize::{Quantizer, QuantizerCelebi};
use material_colors::score::Score;

#[derive(Debug, Clone)]
pub struct ExtractOptions {
    /// Target bitmap edge for downscaling before quantization (`--size`).
    pub bitmap_size: u32,
    /// Max quantizer colors (Python hard-codes 128).
    pub max_colors: usize,
}

impl Default for ExtractOptions {
    fn default() -> Self {
        Self { bitmap_size: 128, max_colors: 128 }
    }
}

#[derive(Debug, Clone, Copy)]
pub struct Seed {
    pub argb: Argb,
    pub hue: f64,
    pub chroma: f64,
    pub tone: f64,
}

impl Seed {
    pub fn from_argb(argb: Argb) -> Self {
        let hct: Hct = argb.into();
        Self { argb, hue: hct.get_hue(), chroma: hct.get_chroma(), tone: hct.get_tone() }
    }
}

/// `calculate_optimal_size` port: scale so area ≤ bitmap_size².
fn optimal_size(width: u32, height: u32, bitmap_size: u32) -> (u32, u32) {
    let image_area = (width as f64) * (height as f64);
    let bitmap_area = (bitmap_size as f64) * (bitmap_size as f64);
    let scale = if image_area > bitmap_area { (bitmap_area / image_area).sqrt() } else { 1.0 };
    let w = ((width as f64) * scale).round().max(1.0) as u32;
    let h = ((height as f64) * scale).round().max(1.0) as u32;
    (w, h)
}

/// Quantize + score already-prepared ARGB pixels (the Android / canvas path —
/// no filesystem, no image decoding).
pub fn seed_from_argb_pixels(pixels: &[Argb], max_colors: usize) -> Seed {
    let result = QuantizerCelebi::quantize(pixels, max_colors);
    let ranked = Score::score(&result.color_to_count, None, None, None);
    Seed::from_argb(ranked[0])
}

#[cfg(feature = "image-decode")]
mod decode {
    use super::*;
    use image::imageops::FilterType;
    use image::{DynamicImage, GenericImageView};

    fn seed_from_dynamic_image(img: DynamicImage, opts: &ExtractOptions) -> Seed {
        let (w, h) = img.dimensions();
        let (nw, nh) = optimal_size(w, h, opts.bitmap_size);
        let img = if nw < w || nh < h {
            // PIL uses BICUBIC here; CatmullRom is the image-crate equivalent.
            // Not bit-identical to PIL, but the quantizer input stays visually
            // the same — seed differences are tolerated by the golden tests.
            img.resize_exact(nw, nh, FilterType::CatmullRom)
        } else {
            img
        };
        let rgba = img.to_rgba8();
        // Python's quantizer drops non-opaque pixels; mirror that.
        let pixels: Vec<Argb> = rgba
            .pixels()
            .filter(|p| p.0[3] == 255)
            .map(|p| Argb::new(255, p.0[0], p.0[1], p.0[2]))
            .collect();
        let pixels = if pixels.is_empty() {
            rgba.pixels().map(|p| Argb::new(255, p.0[0], p.0[1], p.0[2])).collect()
        } else {
            pixels
        };
        seed_from_argb_pixels(&pixels, opts.max_colors)
    }

    /// Decode from raw bytes (any supported format). GIF: uses the second
    /// frame when available, matching the Python script's `image.seek(1)`.
    pub fn seed_from_bytes(bytes: &[u8], opts: &ExtractOptions) -> Result<Seed, image::ImageError> {
        let format = image::guess_format(bytes)?;
        if format == image::ImageFormat::Gif {
            use image::codecs::gif::GifDecoder;
            use image::AnimationDecoder;
            let dec = GifDecoder::new(std::io::Cursor::new(bytes))?;
            let mut frames = dec.into_frames();
            let first = frames.next();
            let second = frames.next();
            if let Some(Ok(frame)) = second.or(first) {
                let img = DynamicImage::ImageRgba8(frame.into_buffer());
                return Ok(seed_from_dynamic_image(img, opts));
            }
        }
        let img = image::load_from_memory(bytes)?;
        Ok(seed_from_dynamic_image(img, opts))
    }

    pub fn seed_from_path(
        path: impl AsRef<std::path::Path>,
        opts: &ExtractOptions,
    ) -> Result<Seed, Box<dyn std::error::Error>> {
        let bytes = std::fs::read(path)?;
        Ok(seed_from_bytes(&bytes, opts)?)
    }
}

#[cfg(feature = "image-decode")]
pub use decode::{seed_from_bytes, seed_from_path};

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn optimal_size_matches_python() {
        assert_eq!(optimal_size(1920, 1080, 128), (171, 96));
        assert_eq!(optimal_size(100, 50, 128), (100, 50)); // no upscale
        assert_eq!(optimal_size(10000, 10, 128), (4048, 4));
    }
}
