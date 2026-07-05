//! Python-materialyoucolor parity engine.
//!
//! The Rust `material-colors` crate and the Python `materialyoucolor` package
//! port different snapshots of material-color-utilities, and the Python lib
//! carries several quirks of its own that the whole downstream rice depends
//! on. Instead of patching individual roles, this module reimplements the
//! Python `DynamicColor.get_tone` engine + the full role table verbatim.
//!
//! Python quirks intentionally preserved (do NOT "fix" these):
//!  - `is_background` on every role holds the *class* `bool` (truthy), so the
//!    tone-50..60 avoidance rule applies to EVERY role, not just backgrounds.
//!  - `tertiary_container` / `on_tertiary_container` have inverted
//!    monochrome/fidelity guards (regular schemes get 60/49 and 0/100).
//!  - `secondary_container` runs `find_desired_chroma_by_tone` for regular
//!    schemes (upstream reserves it for fidelity).
//!  - On-color contrast curves are the older (3, 4.5, 7, 11) generation.
//!  - The dual-background branch returns `dark_option` unless BOTH a light
//!    preference exists AND one option is unavailable.
//!  - `primary` in monochrome is tone 100 in both modes; `primary_container`
//!    is 85 in both; `on_primary_container` 0 in both.
//!  - Tone rounding helpers use Python's banker's rounding.

use material_colors::color::Argb;
use material_colors::contrast::{darker, lighter, ratio_of_tones};
use material_colors::dynamic_color::{ContrastCurve, DynamicScheme, Variant};
use material_colors::hct::Hct;
use material_colors::palette::TonalPalette;

// ── helpers ─────────────────────────────────────────────────────────────

fn is_fidelity(s: &DynamicScheme) -> bool {
    matches!(s.variant, Variant::Fidelity | Variant::Content)
}

fn is_monochrome(s: &DynamicScheme) -> bool {
    matches!(s.variant, Variant::Monochrome)
}

/// Python round(): banker's rounding (half to even).
fn py_round(x: f64) -> f64 {
    let floor = x.floor();
    if (x - floor - 0.5).abs() < f64::EPSILON {
        if (floor as i64) % 2 == 0 {
            floor
        } else {
            floor + 1.0
        }
    } else {
        x.round()
    }
}

fn tone_prefers_light_foreground(tone: f64) -> bool {
    py_round(tone) < 60.0
}

fn lighter_unsafe(tone: f64, ratio: f64) -> f64 {
    let safe = lighter(tone, ratio);
    if safe < 0.0 {
        100.0
    } else {
        safe
    }
}

fn darker_unsafe(tone: f64, ratio: f64) -> f64 {
    let safe = darker(tone, ratio);
    if safe < 0.0 {
        0.0
    } else {
        safe
    }
}

fn foreground_tone(bg_tone: f64, ratio: f64) -> f64 {
    let lighter_tone = lighter_unsafe(bg_tone, ratio);
    let darker_tone = darker_unsafe(bg_tone, ratio);
    let lighter_ratio = ratio_of_tones(lighter_tone, bg_tone);
    let darker_ratio = ratio_of_tones(darker_tone, bg_tone);

    if tone_prefers_light_foreground(bg_tone) {
        let negligible = (lighter_ratio - darker_ratio).abs() < 0.1
            && lighter_ratio < ratio
            && darker_ratio < ratio;
        if lighter_ratio >= ratio || lighter_ratio >= darker_ratio || negligible {
            lighter_tone
        } else {
            darker_tone
        }
    } else if darker_ratio >= ratio || darker_ratio >= lighter_ratio {
        darker_tone
    } else {
        lighter_tone
    }
}

/// Python `KeyColor.create()` — binary-search the tone (pivot 50) whose max
/// available chroma satisfies the palette's requested chroma. The Rust crate
/// uses a different key-color search that lands on different tones (badly so
/// for chroma-0 monochrome palettes), so this is ported verbatim.
fn key_color_tone(hue: f64, requested_chroma: f64) -> f64 {
    let max_chroma = |tone: i32| Hct::from(hue, 200.0, tone as f64).get_chroma();
    let pivot_tone = 50i32;
    let tone_step_size = 1i32;
    let epsilon = 0.01f64;
    let mut lower_tone = 0i32;
    let mut upper_tone = 100i32;
    while lower_tone < upper_tone {
        let mid_tone = (lower_tone + upper_tone) / 2;
        let is_ascending = max_chroma(mid_tone) < max_chroma(mid_tone + tone_step_size);
        let sufficient_chroma = max_chroma(mid_tone) >= requested_chroma - epsilon;

        if sufficient_chroma {
            if (lower_tone - pivot_tone).abs() < (upper_tone - pivot_tone).abs() {
                upper_tone = mid_tone;
            } else {
                if lower_tone == mid_tone {
                    return Hct::from(hue, requested_chroma, lower_tone as f64).get_tone();
                }
                lower_tone = mid_tone;
            }
        } else if is_ascending {
            lower_tone = mid_tone + tone_step_size;
        } else {
            upper_tone = mid_tone;
        }
    }
    Hct::from(hue, requested_chroma, lower_tone as f64).get_tone()
}

/// Python `find_desired_chroma_by_tone`.
fn find_desired_chroma_by_tone(hue: f64, chroma: f64, tone: f64, by_decreasing_tone: bool) -> f64 {
    let mut answer = tone;
    let mut closest_to_chroma = Hct::from(hue, chroma, tone);
    if closest_to_chroma.get_chroma() < chroma {
        let mut chroma_peak = closest_to_chroma.get_chroma();
        while closest_to_chroma.get_chroma() < chroma {
            answer += if by_decreasing_tone { -1.0 } else { 1.0 };
            let potential = Hct::from(hue, chroma, answer);
            if chroma_peak > potential.get_chroma() {
                break;
            }
            if (potential.get_chroma() - chroma).abs() < 0.4 {
                break;
            }
            if (potential.get_chroma() - chroma).abs() < (closest_to_chroma.get_chroma() - chroma).abs() {
                closest_to_chroma = potential;
            }
            chroma_peak = chroma_peak.max(potential.get_chroma());
        }
    }
    answer
}

// ── role table ──────────────────────────────────────────────────────────

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Role {
    PrimaryPaletteKeyColor,
    SecondaryPaletteKeyColor,
    TertiaryPaletteKeyColor,
    NeutralPaletteKeyColor,
    NeutralVariantPaletteKeyColor,
    Background,
    OnBackground,
    Surface,
    SurfaceDim,
    SurfaceBright,
    SurfaceContainerLowest,
    SurfaceContainerLow,
    SurfaceContainer,
    SurfaceContainerHigh,
    SurfaceContainerHighest,
    OnSurface,
    SurfaceVariant,
    OnSurfaceVariant,
    InverseSurface,
    InverseOnSurface,
    Outline,
    OutlineVariant,
    Shadow,
    Scrim,
    SurfaceTint,
    Primary,
    OnPrimary,
    PrimaryContainer,
    OnPrimaryContainer,
    InversePrimary,
    Secondary,
    OnSecondary,
    SecondaryContainer,
    OnSecondaryContainer,
    Tertiary,
    OnTertiary,
    TertiaryContainer,
    OnTertiaryContainer,
    Error,
    OnError,
    ErrorContainer,
    OnErrorContainer,
    PrimaryFixed,
    PrimaryFixedDim,
    OnPrimaryFixed,
    OnPrimaryFixedVariant,
    SecondaryFixed,
    SecondaryFixedDim,
    OnSecondaryFixed,
    OnSecondaryFixedVariant,
    TertiaryFixed,
    TertiaryFixedDim,
    OnTertiaryFixed,
    OnTertiaryFixedVariant,
}

use Role::*;

#[derive(Clone, Copy, PartialEq)]
enum Polarity {
    Nearer,
    Lighter,
}

struct Pair {
    a: Role,
    b: Role,
    delta: f64,
    polarity: Polarity,
    stay_together: bool,
}

fn highest_surface(s: &DynamicScheme) -> Role {
    if s.is_dark {
        SurfaceBright
    } else {
        SurfaceDim
    }
}

fn palette_of<'a>(role: Role, s: &'a DynamicScheme) -> &'a TonalPalette {
    match role {
        PrimaryPaletteKeyColor | SurfaceTint | Primary | OnPrimary | PrimaryContainer
        | OnPrimaryContainer | InversePrimary | PrimaryFixed | PrimaryFixedDim
        | OnPrimaryFixed | OnPrimaryFixedVariant => &s.primary_palette,
        SecondaryPaletteKeyColor | Secondary | OnSecondary | SecondaryContainer
        | OnSecondaryContainer | SecondaryFixed | SecondaryFixedDim | OnSecondaryFixed
        | OnSecondaryFixedVariant => &s.secondary_palette,
        TertiaryPaletteKeyColor | Tertiary | OnTertiary | TertiaryContainer
        | OnTertiaryContainer | TertiaryFixed | TertiaryFixedDim | OnTertiaryFixed
        | OnTertiaryFixedVariant => &s.tertiary_palette,
        NeutralPaletteKeyColor | Background | OnBackground | Surface | SurfaceDim
        | SurfaceBright | SurfaceContainerLowest | SurfaceContainerLow | SurfaceContainer
        | SurfaceContainerHigh | SurfaceContainerHighest | OnSurface | InverseSurface
        | InverseOnSurface | Shadow | Scrim => &s.neutral_palette,
        NeutralVariantPaletteKeyColor | SurfaceVariant | OnSurfaceVariant | Outline
        | OutlineVariant => &s.neutral_variant_palette,
        Error | OnError | ErrorContainer | OnErrorContainer => &s.error_palette,
    }
}

fn raw_tone(role: Role, s: &DynamicScheme) -> f64 {
    let dark = s.is_dark;
    let cl = s.contrast_level;
    let curve = |l: f64, n: f64, m: f64, h: f64| ContrastCurve { low: l, normal: n, medium: m, high: h }.get(cl);
    match role {
        PrimaryPaletteKeyColor => key_color_tone(s.primary_palette.hue(), s.primary_palette.chroma()),
        SecondaryPaletteKeyColor => key_color_tone(s.secondary_palette.hue(), s.secondary_palette.chroma()),
        TertiaryPaletteKeyColor => {
            // Fidelity/Content build the tertiary palette from an explicit
            // HCT (dislike-fixed analogous/complement); Python's from_hct
            // keeps that HCT as the key color instead of searching.
            if is_fidelity(s) {
                s.tertiary_palette.key_color().get_tone()
            } else {
                key_color_tone(s.tertiary_palette.hue(), s.tertiary_palette.chroma())
            }
        }
        NeutralPaletteKeyColor => key_color_tone(s.neutral_palette.hue(), s.neutral_palette.chroma()),
        NeutralVariantPaletteKeyColor => key_color_tone(s.neutral_variant_palette.hue(), s.neutral_variant_palette.chroma()),
        Background | Surface => if dark { 6.0 } else { 98.0 },
        OnBackground | OnSurface => if dark { 90.0 } else { 10.0 },
        SurfaceDim => if dark { 6.0 } else { curve(87.0, 87.0, 80.0, 75.0) },
        SurfaceBright => if dark { curve(24.0, 24.0, 29.0, 34.0) } else { 98.0 },
        SurfaceContainerLowest => if dark { curve(4.0, 4.0, 2.0, 0.0) } else { 100.0 },
        SurfaceContainerLow => if dark { curve(10.0, 10.0, 11.0, 12.0) } else { curve(96.0, 96.0, 96.0, 95.0) },
        SurfaceContainer => if dark { curve(12.0, 12.0, 16.0, 20.0) } else { curve(94.0, 94.0, 92.0, 90.0) },
        SurfaceContainerHigh => if dark { curve(17.0, 17.0, 21.0, 25.0) } else { curve(92.0, 92.0, 88.0, 85.0) },
        SurfaceContainerHighest => if dark { curve(22.0, 22.0, 26.0, 30.0) } else { curve(90.0, 90.0, 84.0, 80.0) },
        SurfaceVariant => if dark { 30.0 } else { 90.0 },
        OnSurfaceVariant => if dark { 80.0 } else { 30.0 },
        InverseSurface => if dark { 90.0 } else { 20.0 },
        InverseOnSurface => if dark { 20.0 } else { 95.0 },
        Outline => if dark { 60.0 } else { 50.0 },
        OutlineVariant => if dark { 30.0 } else { 80.0 },
        Shadow | Scrim => 0.0,
        SurfaceTint => if dark { 80.0 } else { 40.0 },
        Primary => if is_monochrome(s) { 100.0 } else if dark { 80.0 } else { 40.0 },
        OnPrimary => if is_monochrome(s) { 10.0 } else if dark { 20.0 } else { 100.0 },
        PrimaryContainer => {
            if is_fidelity(s) { s.source_color_hct.get_tone() }
            else if is_monochrome(s) { 85.0 }
            else if dark { 30.0 } else { 90.0 }
        }
        OnPrimaryContainer => {
            if is_fidelity(s) { foreground_tone(raw_tone(PrimaryContainer, s), 4.5) }
            else if is_monochrome(s) { 0.0 }
            else if dark { 90.0 } else { 30.0 }
        }
        InversePrimary => if dark { 40.0 } else { 80.0 },
        Secondary => if dark { 80.0 } else { 40.0 },
        OnSecondary => if is_monochrome(s) { 10.0 } else if dark { 20.0 } else { 100.0 },
        SecondaryContainer => {
            let initial = if dark { 30.0 } else { 90.0 };
            if is_monochrome(s) { if dark { 30.0 } else { 85.0 } }
            else if is_fidelity(s) { initial }
            else {
                find_desired_chroma_by_tone(
                    s.secondary_palette.hue(),
                    s.secondary_palette.chroma(),
                    initial,
                    !dark,
                )
            }
        }
        OnSecondaryContainer => {
            if is_monochrome(s) { if dark { 90.0 } else { 10.0 } }
            else if !is_fidelity(s) { if dark { 90.0 } else { 30.0 } }
            else { foreground_tone(raw_tone(SecondaryContainer, s), 4.5) }
        }
        Tertiary => {
            if is_monochrome(s) { if dark { 90.0 } else { 25.0 } }
            else if dark { 80.0 } else { 40.0 }
        }
        OnTertiary => {
            if is_monochrome(s) { if dark { 10.0 } else { 90.0 } }
            else if dark { 20.0 } else { 100.0 }
        }
        TertiaryContainer => {
            // Inverted-guard quirk: regular schemes get 60/49.
            if !is_monochrome(s) { if dark { 60.0 } else { 49.0 } }
            else if dark { 30.0 } else { 90.0 }
        }
        OnTertiaryContainer => {
            if !is_monochrome(s) { if dark { 0.0 } else { 100.0 } }
            else if dark { 90.0 } else { 30.0 }
        }
        Error => if dark { 80.0 } else { 40.0 },
        OnError => if dark { 20.0 } else { 100.0 },
        ErrorContainer => if dark { 30.0 } else { 90.0 },
        OnErrorContainer => {
            if dark { 90.0 } else if is_monochrome(s) { 10.0 } else { 30.0 }
        }
        PrimaryFixed => if is_monochrome(s) { 40.0 } else { 90.0 },
        PrimaryFixedDim => if is_monochrome(s) { 30.0 } else { 80.0 },
        OnPrimaryFixed => if is_monochrome(s) { 100.0 } else { 10.0 },
        OnPrimaryFixedVariant => if is_monochrome(s) { 90.0 } else { 30.0 },
        SecondaryFixed => if is_monochrome(s) { 80.0 } else { 90.0 },
        SecondaryFixedDim => if is_monochrome(s) { 70.0 } else { 80.0 },
        OnSecondaryFixed => 10.0,
        OnSecondaryFixedVariant => if is_monochrome(s) { 25.0 } else { 30.0 },
        TertiaryFixed => if is_monochrome(s) { 40.0 } else { 90.0 },
        TertiaryFixedDim => if is_monochrome(s) { 30.0 } else { 80.0 },
        OnTertiaryFixed => if is_monochrome(s) { 100.0 } else { 10.0 },
        OnTertiaryFixedVariant => if is_monochrome(s) { 90.0 } else { 30.0 },
    }
}

fn background_of(role: Role, s: &DynamicScheme) -> Option<Role> {
    match role {
        OnBackground => Some(Background),
        OnSurface | OnSurfaceVariant | Outline | OutlineVariant | Primary | PrimaryContainer
        | Secondary | SecondaryContainer | Tertiary | TertiaryContainer | Error
        | ErrorContainer | PrimaryFixed | PrimaryFixedDim | SecondaryFixed
        | SecondaryFixedDim | TertiaryFixed | TertiaryFixedDim => Some(highest_surface(s)),
        InverseOnSurface | InversePrimary => Some(InverseSurface),
        OnPrimary => Some(Primary),
        OnPrimaryContainer => Some(PrimaryContainer),
        OnSecondary => Some(Secondary),
        OnSecondaryContainer => Some(SecondaryContainer),
        OnTertiary => Some(Tertiary),
        OnTertiaryContainer => Some(TertiaryContainer),
        OnError => Some(Error),
        OnErrorContainer => Some(ErrorContainer),
        OnPrimaryFixed | OnPrimaryFixedVariant => Some(PrimaryFixedDim),
        OnSecondaryFixed | OnSecondaryFixedVariant => Some(SecondaryFixedDim),
        OnTertiaryFixed | OnTertiaryFixedVariant => Some(TertiaryFixedDim),
        _ => None,
    }
}

fn second_background_of(role: Role) -> Option<Role> {
    match role {
        OnPrimaryFixed | OnPrimaryFixedVariant => Some(PrimaryFixed),
        OnSecondaryFixed | OnSecondaryFixedVariant => Some(SecondaryFixed),
        OnTertiaryFixed | OnTertiaryFixedVariant => Some(TertiaryFixed),
        _ => None,
    }
}

fn curve_of(role: Role) -> Option<[f64; 4]> {
    match role {
        OnBackground => Some([3.0, 3.0, 4.5, 7.0]),
        OnSurface | InverseOnSurface | OnPrimary | OnSecondary | OnTertiary | OnError
        | OnPrimaryFixed | OnSecondaryFixed | OnTertiaryFixed => Some([4.5, 7.0, 11.0, 21.0]),
        OnSurfaceVariant | OnPrimaryContainer | OnSecondaryContainer | OnTertiaryContainer
        | OnErrorContainer | OnPrimaryFixedVariant | OnSecondaryFixedVariant
        | OnTertiaryFixedVariant => Some([3.0, 4.5, 7.0, 11.0]),
        Outline => Some([1.5, 3.0, 4.5, 7.0]),
        OutlineVariant | PrimaryContainer | SecondaryContainer | TertiaryContainer
        | ErrorContainer | PrimaryFixed | PrimaryFixedDim | SecondaryFixed
        | SecondaryFixedDim | TertiaryFixed | TertiaryFixedDim => Some([1.0, 1.0, 3.0, 4.5]),
        Primary | Secondary | Tertiary | Error | InversePrimary => Some([3.0, 4.5, 7.0, 7.0]),
        _ => None,
    }
}

fn pair_of(role: Role) -> Option<Pair> {
    match role {
        Primary | PrimaryContainer => Some(Pair { a: PrimaryContainer, b: Primary, delta: 10.0, polarity: Polarity::Nearer, stay_together: false }),
        Secondary | SecondaryContainer => Some(Pair { a: SecondaryContainer, b: Secondary, delta: 10.0, polarity: Polarity::Nearer, stay_together: false }),
        Tertiary | TertiaryContainer => Some(Pair { a: TertiaryContainer, b: Tertiary, delta: 10.0, polarity: Polarity::Nearer, stay_together: false }),
        Error | ErrorContainer => Some(Pair { a: ErrorContainer, b: Error, delta: 10.0, polarity: Polarity::Nearer, stay_together: false }),
        PrimaryFixed | PrimaryFixedDim => Some(Pair { a: PrimaryFixed, b: PrimaryFixedDim, delta: 10.0, polarity: Polarity::Lighter, stay_together: true }),
        SecondaryFixed | SecondaryFixedDim => Some(Pair { a: SecondaryFixed, b: SecondaryFixedDim, delta: 10.0, polarity: Polarity::Lighter, stay_together: true }),
        TertiaryFixed | TertiaryFixedDim => Some(Pair { a: TertiaryFixed, b: TertiaryFixedDim, delta: 10.0, polarity: Polarity::Lighter, stay_together: true }),
        _ => None,
    }
}

// ── the Python get_tone engine ──────────────────────────────────────────

pub fn get_tone(role: Role, s: &DynamicScheme) -> f64 {
    let decreasing_contrast = s.contrast_level < 0.0;
    let curve_get = |c: [f64; 4]| ContrastCurve { low: c[0], normal: c[1], medium: c[2], high: c[3] }.get(s.contrast_level);

    if let Some(pair) = pair_of(role) {
        let bg_tone = get_tone(background_of(role, s).expect("paired role has bg"), s);

        let a_is_nearer = pair.polarity == Polarity::Nearer
            || (pair.polarity == Polarity::Lighter && !s.is_dark);
        let (nearer, farther) = if a_is_nearer { (pair.a, pair.b) } else { (pair.b, pair.a) };
        let am_nearer = role == nearer;
        let expansion_dir = if s.is_dark { 1.0 } else { -1.0 };

        let n_contrast = curve_get(curve_of(nearer).expect("paired role has curve"));
        let f_contrast = curve_get(curve_of(farther).expect("paired role has curve"));

        let n_initial = raw_tone(nearer, s);
        let mut n_tone = if ratio_of_tones(bg_tone, n_initial) >= n_contrast {
            n_initial
        } else {
            foreground_tone(bg_tone, n_contrast)
        };
        let f_initial = raw_tone(farther, s);
        let mut f_tone = if ratio_of_tones(bg_tone, f_initial) >= f_contrast {
            f_initial
        } else {
            foreground_tone(bg_tone, f_contrast)
        };

        if decreasing_contrast {
            n_tone = foreground_tone(bg_tone, n_contrast);
            f_tone = foreground_tone(bg_tone, f_contrast);
        }

        if (f_tone - n_tone) * expansion_dir >= pair.delta {
            // fine as-is
        } else {
            f_tone = (f_tone - pair.delta * expansion_dir).clamp(0.0, 100.0);
        }

        if (50.0..60.0).contains(&n_tone) {
            if expansion_dir > 0.0 {
                f_tone = f_tone.max(n_tone + pair.delta * expansion_dir);
                n_tone = 60.0;
            } else {
                f_tone = f_tone.min(n_tone + pair.delta * expansion_dir);
                n_tone = 49.0;
            }
        } else if (50.0..60.0).contains(&f_tone) {
            if pair.stay_together {
                if expansion_dir > 0.0 {
                    f_tone = f_tone.max(n_tone + pair.delta * expansion_dir);
                    n_tone = 60.0;
                } else {
                    f_tone = f_tone.min(n_tone + pair.delta * expansion_dir);
                    n_tone = 49.0;
                }
            } else if expansion_dir > 0.0 {
                f_tone = 60.0;
            } else {
                f_tone = 49.0;
            }
        }

        return if am_nearer { n_tone } else { f_tone };
    }

    let mut answer = raw_tone(role, s);

    let Some(bg_role) = background_of(role, s) else {
        return answer;
    };
    let bg_tone = get_tone(bg_role, s);
    let desired_ratio = curve_get(curve_of(role).expect("role with bg has curve"));

    if ratio_of_tones(bg_tone, answer) < desired_ratio {
        answer = foreground_tone(bg_tone, desired_ratio);
    }
    if decreasing_contrast {
        answer = foreground_tone(bg_tone, desired_ratio);
    }

    // Python-bug parity: is_background is truthy for EVERY role, so this
    // avoidance rule is unconditional.
    if (50.0..60.0).contains(&answer) {
        answer = if ratio_of_tones(49.0, bg_tone) >= desired_ratio { 49.0 } else { 60.0 };
    }

    if let Some(bg2_role) = second_background_of(role) {
        let bg_tone1 = get_tone(bg_role, s);
        let bg_tone2 = get_tone(bg2_role, s);
        let upper = bg_tone1.max(bg_tone2);
        let lower = bg_tone1.min(bg_tone2);

        if ratio_of_tones(upper, answer) >= desired_ratio
            && ratio_of_tones(lower, answer) >= desired_ratio
        {
            return answer;
        }

        let light_option = lighter(upper, desired_ratio);
        let dark_option = darker(lower, desired_ratio);
        let prefers_light =
            tone_prefers_light_foreground(bg_tone1) || tone_prefers_light_foreground(bg_tone2);
        // Python-bug parity: dark_option wins unless light is preferred AND
        // one of the options is unavailable.
        return if prefers_light && (light_option == -1.0 || dark_option == -1.0) {
            light_option
        } else {
            dark_option
        };
    }

    answer
}

/// Resolve a role to its final color, exactly like Python's
/// `palette.get_hct(tone)` (float tone straight into the HCT solver).
pub fn get_argb(role: Role, s: &DynamicScheme) -> Argb {
    let p = palette_of(role, s);
    Hct::from(p.hue(), p.chroma(), get_tone(role, s)).into()
}
