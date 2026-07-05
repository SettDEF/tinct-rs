//! Hex/ARGB conversions matching the Python pipeline's exact formatting.

use material_colors::color::Argb;

/// `#RRGGBB`, uppercase — the format every consumer of the pipeline expects
/// (python: `"#{:02X}{:02X}{:02X}".format(...)`).
pub fn argb_to_hex(c: Argb) -> String {
    format!("#{:02X}{:02X}{:02X}", c.red, c.green, c.blue)
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HexParseError(pub String);

impl std::fmt::Display for HexParseError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "invalid hex color: {}", self.0)
    }
}
impl std::error::Error for HexParseError {}

/// Parse `#RRGGBB` (leading `#` optional) to an opaque Argb.
pub fn hex_to_argb(hex: &str) -> Result<Argb, HexParseError> {
    let s = hex.strip_prefix('#').unwrap_or(hex);
    if s.len() != 6 || !s.bytes().all(|b| b.is_ascii_hexdigit()) {
        return Err(HexParseError(hex.to_string()));
    }
    let r = u8::from_str_radix(&s[0..2], 16).unwrap();
    let g = u8::from_str_radix(&s[2..4], 16).unwrap();
    let b = u8::from_str_radix(&s[4..6], 16).unwrap();
    Ok(Argb::new(255, r, g, b))
}

/// Nearest xterm-256 index for a hex color (exact-distance scan over the 16
/// standard + 6×6×6 cube + grayscale ramp). Port of
/// `rebuild_templates.py::hex_to_ansi256_exact` — used for cmus theming.
pub fn hex_to_ansi256_exact(hex: &str) -> Result<u8, HexParseError> {
    let c = hex_to_argb(hex)?;
    let (r, g, b) = (c.red as i32, c.green as i32, c.blue as i32);

    const BASE16: [(i32, i32, i32); 16] = [
        (0x00, 0x00, 0x00), (0x80, 0x00, 0x00), (0x00, 0x80, 0x00), (0x80, 0x80, 0x00),
        (0x00, 0x00, 0x80), (0x80, 0x00, 0x80), (0x00, 0x80, 0x80), (0xc0, 0xc0, 0xc0),
        (0x80, 0x80, 0x80), (0xff, 0x00, 0x00), (0x00, 0xff, 0x00), (0xff, 0xff, 0x00),
        (0x00, 0x00, 0xff), (0xff, 0x00, 0xff), (0x00, 0xff, 0xff), (0xff, 0xff, 0xff),
    ];
    const CUBE: [i32; 6] = [0, 95, 135, 175, 215, 255];

    let mut best = (i64::MAX, 0u8);
    let consider = |idx: u8, cr: i32, cg: i32, cb: i32, best: &mut (i64, u8)| {
        let d = ((r - cr) as i64).pow(2) + ((g - cg) as i64).pow(2) + ((b - cb) as i64).pow(2);
        if d < best.0 {
            *best = (d, idx);
        }
    };

    for (i, &(cr, cg, cb)) in BASE16.iter().enumerate() {
        consider(i as u8, cr, cg, cb, &mut best);
    }
    for (i, &cr) in CUBE.iter().enumerate() {
        for (j, &cg) in CUBE.iter().enumerate() {
            for (k, &cb) in CUBE.iter().enumerate() {
                consider((16 + 36 * i + 6 * j + k) as u8, cr, cg, cb, &mut best);
            }
        }
    }
    for i in 0..24 {
        let v = 8 + i * 10;
        consider((232 + i) as u8, v, v, v, &mut best);
    }
    Ok(best.1)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn hex_roundtrip() {
        let c = hex_to_argb("#B52755").unwrap();
        assert_eq!(argb_to_hex(c), "#B52755");
        assert_eq!(argb_to_hex(hex_to_argb("b52755").unwrap()), "#B52755");
        assert!(hex_to_argb("#xyz").is_err());
    }

    #[test]
    fn ansi256_corners() {
        assert_eq!(hex_to_ansi256_exact("#000000").unwrap(), 0);
        assert_eq!(hex_to_ansi256_exact("#FFFFFF").unwrap(), 15);
    }
}
