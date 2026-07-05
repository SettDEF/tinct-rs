//! Terminal ANSI palette derivation: harmonize a base-16 scheme toward the
//! generated palette's primary key color. Port of the harmonize /
//! boost_chroma_tone / blend_bg_fg logic in `generate_colors_material.py`.

use material_colors::color::Argb;
use material_colors::hct::Hct;
use material_colors::utils::math::{difference_degrees, sanitize_degrees_double};

use crate::color::{argb_to_hex, hex_to_argb};
use crate::palette::Palette;

/// Ordered base-16 seed colors, e.g. term0..term15 as read from
/// `scheme-base.json` (order preserved — it defines output order).
pub type Base16 = Vec<(String, String)>;

#[derive(Debug, Clone)]
pub struct TerminalOptions {
    /// (0-1) hue shift strength toward the accent (`--harmony`).
    pub harmony: f64,
    /// (0-180) max hue-rotation angle (`--harmonize_threshold`).
    pub harmonize_threshold: f64,
    /// Extra fg/bg tone separation (`--term_fg_boost`).
    pub term_fg_boost: f64,
    /// Blend term0 toward surfaceContainerLow and term15 toward onSurface.
    pub blend_bg_fg: bool,
    /// Pass colors through untouched. NOTE: the Python script compared the
    /// scheme name against the literal "monochrome", which never matches the
    /// "scheme-monochrome" values callers pass — so in the legacy pipeline
    /// this is effectively always false. Kept for explicit opt-in.
    pub passthrough: bool,
}

impl Default for TerminalOptions {
    fn default() -> Self {
        Self {
            harmony: 0.8,
            harmonize_threshold: 100.0,
            term_fg_boost: 0.35,
            blend_bg_fg: false,
            passthrough: false,
        }
    }
}

fn rotation_direction(from_hue: f64, to_hue: f64) -> f64 {
    let increasing_difference = sanitize_degrees_double(to_hue - from_hue);
    if increasing_difference <= 180.0 {
        1.0
    } else {
        -1.0
    }
}

/// Rotate `design_color`'s hue toward `source_color` — parameterized version
/// (material-colors' built-in `blend::harmonize` hard-codes 0.5/15°).
pub fn harmonize(design_color: Argb, source_color: Argb, threshold: f64, harmony: f64) -> Argb {
    let from_hct: Hct = design_color.into();
    let to_hct: Hct = source_color.into();
    let diff = difference_degrees(from_hct.get_hue(), to_hct.get_hue());
    let rotation = (diff * harmony).min(threshold);
    let output_hue = sanitize_degrees_double(
        from_hct.get_hue() + rotation * rotation_direction(from_hct.get_hue(), to_hct.get_hue()),
    );
    Hct::from(output_hue, from_hct.get_chroma(), from_hct.get_tone()).into()
}

/// Multiply chroma and tone in HCT space.
pub fn boost_chroma_tone(argb: Argb, chroma: f64, tone: f64) -> Argb {
    let hct: Hct = argb.into();
    Hct::from(hct.get_hue(), hct.get_chroma() * chroma, hct.get_tone() * tone).into()
}

/// Derive term0..term15 from a base scheme, harmonized toward the palette's
/// `primary_paletteKeyColor`. Parity detail: the accent is parsed back from
/// the palette's HEX string (not kept as raw ARGB) exactly like the Python
/// script, so rounding matches.
pub fn derive_terminal(palette: &Palette, base: &Base16, opts: &TerminalOptions) -> Vec<(String, String)> {
    let dark = palette.dark;
    let accent = palette
        .get("primary_paletteKeyColor")
        .and_then(|h| hex_to_argb(h).ok());

    base.iter()
        .map(|(name, hex)| {
            if opts.passthrough {
                return (name.clone(), hex.clone());
            }
            let Ok(base_argb) = hex_to_argb(hex) else {
                return (name.clone(), hex.clone());
            };
            let out = if opts.blend_bg_fg && name == "term0" {
                let src = palette
                    .get("surfaceContainerLow")
                    .and_then(|h| hex_to_argb(h).ok())
                    .unwrap_or(base_argb);
                boost_chroma_tone(src, 1.2, 0.95)
            } else if opts.blend_bg_fg && name == "term15" {
                let src = palette
                    .get("onSurface")
                    .and_then(|h| hex_to_argb(h).ok())
                    .unwrap_or(base_argb);
                boost_chroma_tone(src, 3.0, 1.0)
            } else {
                let harmonized = match accent {
                    Some(a) => harmonize(base_argb, a, opts.harmonize_threshold, opts.harmony),
                    None => base_argb,
                };
                let boost = 1.0 + opts.term_fg_boost * if dark { 1.0 } else { -1.0 };
                boost_chroma_tone(harmonized, 1.0, boost)
            };
            (name.clone(), argb_to_hex(out))
        })
        .collect()
}
