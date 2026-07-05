//! Material You scheme generation with Python-parity role names/order.

use material_colors::color::Argb;
use material_colors::dynamic_color::{DynamicScheme, Variant};
use crate::compat::Role;

use crate::color::argb_to_hex;
use crate::palette::Palette;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SchemeVariant {
    TonalSpot,
    Vibrant,
    Expressive,
    FruitSalad,
    Monochrome,
    Rainbow,
    Neutral,
    Fidelity,
    Content,
}

impl SchemeVariant {
    /// Accepts the CLI names used across the pipeline ("scheme-tonal-spot"…).
    /// Unknown names fall back to TonalSpot — same as the Python script's
    /// trailing `else` branch.
    pub fn from_cli_name(name: &str) -> Self {
        match name {
            "scheme-fruit-salad" => Self::FruitSalad,
            "scheme-expressive" => Self::Expressive,
            "scheme-monochrome" => Self::Monochrome,
            "scheme-rainbow" => Self::Rainbow,
            "scheme-tonal-spot" => Self::TonalSpot,
            "scheme-neutral" => Self::Neutral,
            "scheme-fidelity" => Self::Fidelity,
            "scheme-content" => Self::Content,
            "scheme-vibrant" => Self::Vibrant,
            _ => Self::TonalSpot,
        }
    }

    pub fn cli_name(&self) -> &'static str {
        match self {
            Self::FruitSalad => "scheme-fruit-salad",
            Self::Expressive => "scheme-expressive",
            Self::Monochrome => "scheme-monochrome",
            Self::Rainbow => "scheme-rainbow",
            Self::TonalSpot => "scheme-tonal-spot",
            Self::Neutral => "scheme-neutral",
            Self::Fidelity => "scheme-fidelity",
            Self::Content => "scheme-content",
            Self::Vibrant => "scheme-vibrant",
        }
    }

    fn to_variant(self) -> Variant {
        match self {
            Self::TonalSpot => Variant::TonalSpot,
            Self::Vibrant => Variant::Vibrant,
            Self::Expressive => Variant::Expressive,
            Self::FruitSalad => Variant::FruitSalad,
            Self::Monochrome => Variant::Monochrome,
            Self::Rainbow => Variant::Rainbow,
            Self::Neutral => Variant::Neutral,
            Self::Fidelity => Variant::Fidelity,
            Self::Content => Variant::Content,
        }
    }
}

/// Python-parity scheme construction. `material-colors` and Python's
/// materialyoucolor agree on every variant's palette tables except Vibrant,
/// where Python uses neutral-variant chroma 12 (Rust: 10) — so Vibrant is
/// built by hand with the Python table.
#[doc(hidden)]
pub fn debug_scheme(seed: Argb, variant: SchemeVariant, dark: bool, contrast: f64) -> DynamicScheme {
    build_scheme(seed, variant, dark, contrast)
}

fn build_scheme(seed: Argb, variant: SchemeVariant, dark: bool, contrast: f64) -> DynamicScheme {
    use material_colors::hct::Hct;
    use material_colors::palette::TonalPalette;
    use material_colors::scheme::variant::SchemeVibrant;

    if variant != SchemeVariant::Vibrant {
        return DynamicScheme::by_variant(seed, &variant.to_variant(), dark, Some(contrast));
    }
    let hct: Hct = seed.into();
    let hue = hct.get_hue();
    DynamicScheme::new(
        seed,
        None,
        Variant::Vibrant,
        dark,
        Some(contrast),
        TonalPalette::of(hue, 200.0),
        TonalPalette::of(
            DynamicScheme::get_rotated_hue(hue, &SchemeVibrant::HUES, &SchemeVibrant::SECONDARY_ROTATIONS),
            24.0,
        ),
        TonalPalette::of(
            DynamicScheme::get_rotated_hue(hue, &SchemeVibrant::HUES, &SchemeVibrant::TERTIARY_ROTATIONS),
            32.0,
        ),
        TonalPalette::of(hue, 10.0),
        TonalPalette::of(hue, 12.0),
        None,
    )
}

const ROLES: &[(&str, Role)] = &[
    ("primary_paletteKeyColor", Role::PrimaryPaletteKeyColor),
    ("secondary_paletteKeyColor", Role::SecondaryPaletteKeyColor),
    ("tertiary_paletteKeyColor", Role::TertiaryPaletteKeyColor),
    ("neutral_paletteKeyColor", Role::NeutralPaletteKeyColor),
    ("neutral_variant_paletteKeyColor", Role::NeutralVariantPaletteKeyColor),
    ("background", Role::Background),
    ("onBackground", Role::OnBackground),
    ("surface", Role::Surface),
    ("surfaceDim", Role::SurfaceDim),
    ("surfaceBright", Role::SurfaceBright),
    ("surfaceContainerLowest", Role::SurfaceContainerLowest),
    ("surfaceContainerLow", Role::SurfaceContainerLow),
    ("surfaceContainer", Role::SurfaceContainer),
    ("surfaceContainerHigh", Role::SurfaceContainerHigh),
    ("surfaceContainerHighest", Role::SurfaceContainerHighest),
    ("onSurface", Role::OnSurface),
    ("surfaceVariant", Role::SurfaceVariant),
    ("onSurfaceVariant", Role::OnSurfaceVariant),
    ("inverseSurface", Role::InverseSurface),
    ("inverseOnSurface", Role::InverseOnSurface),
    ("outline", Role::Outline),
    ("outlineVariant", Role::OutlineVariant),
    ("shadow", Role::Shadow),
    ("scrim", Role::Scrim),
    ("surfaceTint", Role::SurfaceTint),
    ("primary", Role::Primary),
    ("onPrimary", Role::OnPrimary),
    ("primaryContainer", Role::PrimaryContainer),
    ("onPrimaryContainer", Role::OnPrimaryContainer),
    ("inversePrimary", Role::InversePrimary),
    ("secondary", Role::Secondary),
    ("onSecondary", Role::OnSecondary),
    ("secondaryContainer", Role::SecondaryContainer),
    ("onSecondaryContainer", Role::OnSecondaryContainer),
    ("tertiary", Role::Tertiary),
    ("onTertiary", Role::OnTertiary),
    ("tertiaryContainer", Role::TertiaryContainer),
    ("onTertiaryContainer", Role::OnTertiaryContainer),
    ("error", Role::Error),
    ("onError", Role::OnError),
    ("errorContainer", Role::ErrorContainer),
    ("onErrorContainer", Role::OnErrorContainer),
    ("primaryFixed", Role::PrimaryFixed),
    ("primaryFixedDim", Role::PrimaryFixedDim),
    ("onPrimaryFixed", Role::OnPrimaryFixed),
    ("onPrimaryFixedVariant", Role::OnPrimaryFixedVariant),
    ("secondaryFixed", Role::SecondaryFixed),
    ("secondaryFixedDim", Role::SecondaryFixedDim),
    ("onSecondaryFixed", Role::OnSecondaryFixed),
    ("onSecondaryFixedVariant", Role::OnSecondaryFixedVariant),
    ("tertiaryFixed", Role::TertiaryFixed),
    ("tertiaryFixedDim", Role::TertiaryFixedDim),
    ("onTertiaryFixed", Role::OnTertiaryFixed),
    ("onTertiaryFixedVariant", Role::OnTertiaryFixedVariant),
];

/// Generate the full Material palette (incl. the pipeline's hard-coded
/// extended `success` roles) for a seed color.
pub fn generate_palette(seed: Argb, variant: SchemeVariant, dark: bool, contrast: f64) -> Palette {
    let scheme = build_scheme(seed, variant, dark, contrast);
    let mut entries: Vec<(String, String)> = Vec::with_capacity(ROLES.len() + 4);

    for (name, role) in ROLES {
        entries.push((name.to_string(), argb_to_hex(crate::compat::get_argb(*role, &scheme))));
    }

    // Extended material — same constants as generate_colors_material.py.
    let success: [(&str, &str); 4] = if dark {
        [
            ("success", "#B5CCBA"),
            ("onSuccess", "#213528"),
            ("successContainer", "#374B3E"),
            ("onSuccessContainer", "#D1E9D6"),
        ]
    } else {
        [
            ("success", "#4F6354"),
            ("onSuccess", "#FFFFFF"),
            ("successContainer", "#D1E8D5"),
            ("onSuccessContainer", "#0C1F13"),
        ]
    };
    for (k, v) in success {
        entries.push((k.to_string(), v.to_string()));
    }

    Palette { dark, entries }
}
