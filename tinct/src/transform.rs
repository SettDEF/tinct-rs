//! Palette post-transforms: color-theory rotation, style HSL filters,
//! practical passes (high-contrast / duotone) and nearest-color remapping,
//! applied in a caller-chosen order (WallTune's draggable "Active mix").
//! Byte-parity port of `palette_transform.py`, including Python's `colorsys`
//! HLS round-trip and banker's rounding on the final u8 channels.

/// Python `round()` (half-to-even) — the hex output depends on it.
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

// ── colorsys ports (python stdlib semantics, byte-exact) ───────────────

fn rgb_to_hls(r: f64, g: f64, b: f64) -> (f64, f64, f64) {
    let maxc = r.max(g).max(b);
    let minc = r.min(g).min(b);
    let sumc = maxc + minc;
    let rangec = maxc - minc;
    let l = sumc / 2.0;
    if minc == maxc {
        return (0.0, l, 0.0);
    }
    let s = if l <= 0.5 { rangec / sumc } else { rangec / (2.0 - sumc) };
    let rc = (maxc - r) / rangec;
    let gc = (maxc - g) / rangec;
    let bc = (maxc - b) / rangec;
    let h = if r == maxc {
        bc - gc
    } else if g == maxc {
        2.0 + rc - bc
    } else {
        4.0 + gc - rc
    };
    ((h / 6.0).rem_euclid(1.0), l, s)
}

fn hls_v(m1: f64, m2: f64, hue: f64) -> f64 {
    let hue = hue.rem_euclid(1.0);
    if hue < 1.0 / 6.0 {
        m1 + (m2 - m1) * hue * 6.0
    } else if hue < 0.5 {
        m2
    } else if hue < 2.0 / 3.0 {
        m1 + (m2 - m1) * (2.0 / 3.0 - hue) * 6.0
    } else {
        m1
    }
}

fn hls_to_rgb(h: f64, l: f64, s: f64) -> (f64, f64, f64) {
    if s == 0.0 {
        return (l, l, l);
    }
    let m2 = if l <= 0.5 { l * (1.0 + s) } else { l + s - l * s };
    let m1 = 2.0 * l - m2;
    (
        hls_v(m1, m2, h + 1.0 / 3.0),
        hls_v(m1, m2, h),
        hls_v(m1, m2, h - 1.0 / 3.0),
    )
}

fn hex_to_rgb(hex: &str) -> Option<(f64, f64, f64)> {
    let h = hex.trim_start_matches('#');
    let h = if h.len() == 8 { &h[..6] } else { h };
    if h.len() != 6 || !h.bytes().all(|b| b.is_ascii_hexdigit()) {
        return None;
    }
    let c = |i: usize| u8::from_str_radix(&h[i..i + 2], 16).unwrap() as f64 / 255.0;
    Some((c(0), c(2), c(4)))
}

/// Lowercase hex, python `rgb_to_hex` semantics (banker's rounding + clamp).
fn rgb_to_hex(r: f64, g: f64, b: f64) -> String {
    let clamp = |x: f64| py_round(x * 255.0).clamp(0.0, 255.0) as u8;
    format!("#{:02x}{:02x}{:02x}", clamp(r), clamp(g), clamp(b))
}

fn shift_hue(hex: &str, deg: f64) -> String {
    let Some((r, g, b)) = hex_to_rgb(hex) else { return hex.to_string() };
    let (h, l, s) = rgb_to_hls(r, g, b);
    let h = (h + deg / 360.0).rem_euclid(1.0);
    let (r, g, b) = hls_to_rgb(h, l, s);
    rgb_to_hex(r, g, b)
}

fn set_hue(hex: &str, h_target: f64) -> String {
    let Some((r, g, b)) = hex_to_rgb(hex) else { return hex.to_string() };
    let (_, l, s) = rgb_to_hls(r, g, b);
    let (r, g, b) = hls_to_rgb(h_target, l, s);
    rgb_to_hex(r, g, b)
}

// ── steps ───────────────────────────────────────────────────────────────

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Theory {
    Mono,
    Analogous,
    Complementary,
    Triadic,
    Split,
    Tetradic,
}

impl Theory {
    pub fn from_name(name: &str) -> Option<Self> {
        match name {
            "mono" => Some(Self::Mono),
            "analogous" => Some(Self::Analogous),
            "complementary" => Some(Self::Complementary),
            "triadic" => Some(Self::Triadic),
            "split" => Some(Self::Split),
            "tetradic" => Some(Self::Tetradic),
            _ => None, // "", "material", unknown → no-op
        }
    }

    /// (primary, secondary, tertiary) hue deltas in degrees.
    fn deltas(self) -> (f64, f64, f64) {
        match self {
            Self::Mono => (0.0, 0.0, 0.0),
            Self::Analogous => (0.0, 30.0, -30.0),
            Self::Complementary => (0.0, 180.0, 180.0),
            Self::Triadic => (0.0, 120.0, 240.0),
            Self::Split => (0.0, 150.0, 210.0),
            Self::Tetradic => (0.0, 90.0, 180.0),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Style {
    Pastel,
    Muted,
    Bright,
    Colorful,
}

impl Style {
    pub fn from_name(name: &str) -> Option<Self> {
        match name {
            "pastel" => Some(Self::Pastel),
            "muted" => Some(Self::Muted),
            "bright" => Some(Self::Bright),
            "colorful" => Some(Self::Colorful),
            _ => None,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Practical {
    HighContrast,
    Duotone,
}

impl Practical {
    pub fn from_name(name: &str) -> Option<Self> {
        match name {
            "high_contrast" => Some(Self::HighContrast),
            "duotone" => Some(Self::Duotone),
            _ => None,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StepKind {
    Theory,
    Style,
    Practical,
    Remap,
}

pub const DEFAULT_ORDER: [StepKind; 4] =
    [StepKind::Theory, StepKind::Style, StepKind::Practical, StepKind::Remap];

/// Parse a `--mix-order` string ("remap,theory") into a full step order:
/// recognized names first (in given order), then the canonical remainder.
pub fn resolve_order(mix_order: &str) -> Vec<StepKind> {
    let mut order: Vec<StepKind> = mix_order
        .split(',')
        .filter_map(|s| match s.trim() {
            "theory" => Some(StepKind::Theory),
            "style" => Some(StepKind::Style),
            "practical" => Some(StepKind::Practical),
            "remap" => Some(StepKind::Remap),
            _ => None,
        })
        .collect();
    for step in DEFAULT_ORDER {
        if !order.contains(&step) {
            order.push(step);
        }
    }
    order
}

/// Remap palette: list of (hex, (r, g, b)) target colors.
pub type RemapPalette = Vec<(String, (f64, f64, f64))>;

/// The 58 named palettes shipped with the pipeline (embedded palettes.json).
pub fn builtin_palette(name: &str) -> Option<RemapPalette> {
    let raw = include_str!("../data/palettes.json");
    // Flat {"name": ["#hex", ...], ...} — parsed by hand to avoid a
    // serde_json dependency in the core crate.
    let key = format!("\"{}\"", name);
    let start = raw.find(&key)?;
    let open = raw[start..].find('[')? + start;
    let close = raw[open..].find(']')? + open;
    let colors: Vec<(String, (f64, f64, f64))> = raw[open + 1..close]
        .split(',')
        .filter_map(|item| {
            let hex = item.trim().trim_matches('"').to_string();
            hex_to_rgb(&hex).map(|rgb| (hex, rgb))
        })
        .collect();
    if colors.is_empty() {
        None
    } else {
        Some(colors)
    }
}

#[derive(Debug, Clone, Default)]
pub struct TransformConfig {
    pub theory: Option<Theory>,
    pub style: Option<Style>,
    pub practical: Option<Practical>,
    pub remap: Option<RemapPalette>,
    /// Empty → canonical order.
    pub order: Vec<StepKind>,
}

impl TransformConfig {
    pub fn is_noop(&self) -> bool {
        self.theory.is_none()
            && self.style.is_none()
            && self.practical.is_none()
            && self.remap.is_none()
    }
}

fn camel_to_snake(s: &str) -> String {
    let mut out = String::with_capacity(s.len() + 4);
    for (i, ch) in s.chars().enumerate() {
        if ch.is_ascii_uppercase() && i != 0 {
            out.push('_');
        }
        out.push(ch.to_ascii_lowercase());
    }
    out
}

#[derive(Clone, Copy, PartialEq)]
enum Group {
    Primary,
    Secondary,
    Tertiary,
}

fn classify(key_snake: &str) -> Option<Group> {
    if key_snake.starts_with("primary") || key_snake.starts_with("inverse_primary") {
        Some(Group::Primary)
    } else if key_snake.starts_with("secondary") {
        Some(Group::Secondary)
    } else if key_snake.starts_with("tertiary") {
        Some(Group::Tertiary)
    } else {
        None
    }
}

fn style_filter(hex: &str, style: Style) -> String {
    let Some((r, g, b)) = hex_to_rgb(hex) else { return hex.to_string() };
    let (h, mut l, mut s) = rgb_to_hls(r, g, b);
    match style {
        Style::Pastel => {
            s = s.min(0.45);
            if l > 0.25 && l < 0.75 {
                l = l * 0.5 + 0.5 * 0.78;
            }
        }
        Style::Muted => s = s.min(0.35),
        Style::Bright => s = s.max((s * 1.5 + 0.2).min(1.0)),
        Style::Colorful => s = (s * 1.25 + 0.15).min(1.0),
    }
    let (r, g, b) = hls_to_rgb(h, l, s);
    rgb_to_hex(r, g, b)
}

fn practical_filter(
    hex: &str,
    group: Option<Group>,
    key_snake: &str,
    mode: Practical,
    primary_hue: f64,
) -> String {
    let Some((r, g, b)) = hex_to_rgb(hex) else { return hex.to_string() };
    let (mut h, mut l, mut s) = rgb_to_hls(r, g, b);
    match mode {
        Practical::HighContrast => {
            s = (s * 1.2 + 0.05).min(1.0);
            if key_snake.starts_with("on_") {
                l = if l < 0.5 { 0.05 } else { 0.96 };
            } else {
                l = if l < 0.5 { (l - 0.05).max(0.0) } else { (l + 0.05).min(1.0) };
            }
        }
        Practical::Duotone => {
            if group == Some(Group::Tertiary) {
                h = (primary_hue + 0.5).rem_euclid(1.0);
            }
        }
    }
    let (r, g, b) = hls_to_rgb(h, l, s);
    rgb_to_hex(r, g, b)
}

fn find_nearest_color(hex: &str, palette: &RemapPalette) -> String {
    let Some((r, g, b)) = hex_to_rgb(hex) else { return hex.to_string() };
    let mut best = (f64::INFINITY, hex.to_string());
    for (p_hex, (pr, pg, pb)) in palette {
        let rmean = (r + pr) / 2.0;
        let (dr, dg, db) = (r - pr, g - pg, b - pb);
        let dist = (2.0 + rmean) * dr * dr + 4.0 * dg * dg + (3.0 - rmean) * db * db;
        if dist < best.0 {
            best = (dist, p_hex.clone());
        }
    }
    best.1
}

/// Transform one key/hex. `primary_hue` is the HLS hue (0..1) of the
/// palette's PRE-transform `primary` value.
pub fn transform_one(key: &str, hex: &str, primary_hue: f64, cfg: &TransformConfig) -> String {
    let key_snake = camel_to_snake(key);
    let group = classify(&key_snake);
    let order: &[StepKind] = if cfg.order.is_empty() { &DEFAULT_ORDER } else { &cfg.order };

    let mut out = hex.to_string();
    for step in order {
        match step {
            StepKind::Theory => {
                if let (Some(theory), Some(g)) = (cfg.theory, group) {
                    let (p, sec, ter) = theory.deltas();
                    if p == 0.0 && sec == 0.0 && ter == 0.0 {
                        // mono: collapse all accent hues onto primary's hue
                        out = set_hue(&out, primary_hue);
                    } else {
                        let delta = match g {
                            Group::Primary => p,
                            Group::Secondary => sec,
                            Group::Tertiary => ter,
                        };
                        out = shift_hue(&out, delta);
                    }
                }
            }
            StepKind::Style => {
                if let (Some(style), Some(_)) = (cfg.style, group) {
                    out = style_filter(&out, style);
                }
            }
            StepKind::Practical => {
                if let Some(mode) = cfg.practical {
                    if group.is_some() || key_snake.starts_with("on_") {
                        out = practical_filter(&out, group, &key_snake, mode, primary_hue);
                    }
                }
            }
            StepKind::Remap => {
                if let Some(palette) = &cfg.remap {
                    out = find_nearest_color(&out, palette);
                }
            }
        }
    }
    out
}

/// HLS hue (0..1) of a hex color — for computing `primary_hue` from the
/// pre-transform primary entry.
pub fn hls_hue(hex: &str) -> Option<f64> {
    hex_to_rgb(hex).map(|(r, g, b)| rgb_to_hls(r, g, b).0)
}

/// Transform ordered (key, value) entries in place. Non-color values (not
/// starting with '#') pass through untouched. No-op without a `primary` key.
pub fn transform_entries(entries: &mut [(String, String)], cfg: &TransformConfig) -> bool {
    let Some(primary_hue) = entries
        .iter()
        .find(|(k, _)| camel_to_snake(k) == "primary")
        .and_then(|(_, v)| hls_hue(v))
    else {
        return false;
    };
    for (k, v) in entries.iter_mut() {
        if v.starts_with('#') {
            *v = transform_one(k, v, primary_hue, cfg);
        }
    }
    true
}
