//! Pure string builders for the pipeline's output formats. No I/O here.

use crate::palette::Palette;

/// SCSS in the exact format `generate_colors_material.py` prints, including
/// Python's capitalized booleans.
pub fn to_scss(palette: &Palette, transparent: bool, terms: &[(String, String)]) -> String {
    let py_bool = |b: bool| if b { "True" } else { "False" };
    let mut out = String::with_capacity(4096);
    out.push_str(&format!("$darkmode: {};\n", py_bool(palette.dark)));
    out.push_str(&format!("$transparent: {};\n", py_bool(transparent)));
    for (k, v) in &palette.entries {
        out.push_str(&format!("${}: {};\n", k, v));
    }
    for (k, v) in terms {
        out.push_str(&format!("${}: {};\n", k, v));
    }
    out
}

/// matugen-compatible flat colors.json: the 50 scheme keys in snake_case,
/// alphabetically ordered, lowercase hex, 2-space indent — the exact format
/// the m3colors template produced, so downstream consumers (quickshell,
/// palette transforms, template rendering) see no difference.
pub fn to_matugen_colors_json(palette: &Palette) -> String {
    fn to_snake(s: &str) -> String {
        let mut out = String::new();
        for (i, ch) in s.chars().enumerate() {
            if ch.is_ascii_uppercase() && i != 0 {
                out.push('_');
            }
            out.push(ch.to_ascii_lowercase());
        }
        out
    }
    // Keys NOT in matugen's colors.json: palette key colors, the extended
    // success roles, and background/on_background ARE included.
    let mut entries: Vec<(String, String)> = palette
        .entries
        .iter()
        .filter(|(k, _)| !k.contains("paletteKeyColor") && !k.to_lowercase().contains("success"))
        .map(|(k, v)| (to_snake(k), v.to_lowercase()))
        .collect();
    entries.sort_by(|a, b| a.0.cmp(&b.0));

    let mut out = String::from("{\n");
    for (i, (k, v)) in entries.iter().enumerate() {
        out.push_str(&format!("  \"{}\": \"{}\"", k, v));
        if i + 1 < entries.len() {
            out.push(',');
        }
        out.push('\n');
    }
    out.push('}');
    out
}

/// Flat `{ "role": "#HEX", ... }` JSON preserving palette order.
pub fn to_flat_json(palette: &Palette, terms: &[(String, String)]) -> String {
    let mut out = String::from("{");
    let mut first = true;
    for (k, v) in palette.entries.iter().chain(terms.iter()) {
        if !first {
            out.push(',');
        }
        first = false;
        out.push_str(&format!("\"{}\":\"{}\"", k, v));
    }
    out.push('}');
    out
}
