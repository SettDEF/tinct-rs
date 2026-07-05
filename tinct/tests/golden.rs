//! Golden parity tests: byte-identical SCSS vs the Python
//! generate_colors_material.py for --color seeds (no image decode involved,
//! so any diff is a real algorithm divergence).

use tinct::{derive_terminal, generate_palette, hex_to_argb, output, Base16, SchemeVariant, TerminalOptions};
#[allow(unused_imports)]
use tinct as _;

fn base16(dark: bool) -> Base16 {
    let raw = include_str!("fixtures/scheme-base.json");
    // minimal hand-parser to avoid a dev-dependency: the file is a flat
    // two-level object of "termN": "#HEX" pairs.
    let section = if dark { "\"dark\"" } else { "\"light\"" };
    let start = raw.find(section).expect("section") + section.len();
    let open = raw[start..].find('{').unwrap() + start;
    let close = raw[open..].find('}').unwrap() + open;
    let body = &raw[open + 1..close];
    body.split(',')
        .filter_map(|pair| {
            let mut it = pair.split(':');
            let k = it.next()?.trim().trim_matches('"').to_string();
            let v = it.next()?.trim().trim_matches('"').to_string();
            if k.is_empty() { None } else { Some((k, v)) }
        })
        .collect()
}

fn run_case(color: &str, dark: bool, scheme: &str, blend_bg_fg: bool, fixture: &str) {
    let seed = hex_to_argb(color).unwrap();
    let variant = SchemeVariant::from_cli_name(scheme);
    let palette = generate_palette(seed, variant, dark, 0.0);
    let opts = TerminalOptions { blend_bg_fg, ..Default::default() };
    let terms = derive_terminal(&palette, &base16(dark), &opts);
    let got = output::to_scss(&palette, false, &terms);
    let want = std::fs::read_to_string(format!("tests/fixtures/{fixture}")).unwrap();
    if got != want {
        for (g, w) in got.lines().zip(want.lines()) {
            if g != w {
                eprintln!("MISMATCH: got `{g}` want `{w}`");
            }
        }
    }
    assert_eq!(got, want, "fixture {fixture} diverged");
}

#[test]
fn color_b52755_dark_tonalspot_blend() {
    run_case("#B52755", true, "scheme-tonal-spot", true, "color-B52755-dark-tonalspot.scss");
}

const ALL_SCHEMES: [&str; 9] = [
    "tonal-spot", "vibrant", "expressive", "fruit-salad", "monochrome",
    "rainbow", "neutral", "fidelity", "content",
];

/// Full matrix: 9 schemes × 2 modes × 2 seed/option sets = 36 fixtures,
/// all byte-identical to generate_colors_material.py output.
#[test]
fn full_matrix_b52755_blend() {
    for scheme in ALL_SCHEMES {
        for mode in ["dark", "light"] {
            run_case(
                "#B52755", mode == "dark", &format!("scheme-{scheme}"), true,
                &format!("full-B52755-{mode}-{scheme}.scss"),
            );
        }
    }
}

#[test]
fn full_matrix_22aa77_custom_harmony() {
    for scheme in ALL_SCHEMES {
        for mode in ["dark", "light"] {
            let seed = hex_to_argb("#22AA77").unwrap();
            let variant = SchemeVariant::from_cli_name(&format!("scheme-{scheme}"));
            let palette = generate_palette(seed, variant, mode == "dark", 0.0);
            let opts = TerminalOptions {
                harmony: 0.5,
                harmonize_threshold: 60.0,
                term_fg_boost: 0.5,
                ..Default::default()
            };
            let terms = derive_terminal(&palette, &base16(mode == "dark"), &opts);
            let got = tinct::output::to_scss(&palette, false, &terms);
            let fixture = format!("full-22AA77-{mode}-{scheme}.scss");
            let want = std::fs::read_to_string(format!("tests/fixtures/{fixture}")).unwrap();
            if got != want {
                for (g, w) in got.lines().zip(want.lines()) {
                    if g != w {
                        eprintln!("MISMATCH [{fixture}]: got `{g}` want `{w}`");
                    }
                }
            }
            assert_eq!(got, want, "fixture {fixture} diverged");
        }
    }
}

#[test]
fn color_3a6ea5_matrix() {
    for mode in ["dark", "light"] {
        for scheme in ["vibrant", "content", "neutral", "expressive"] {
            run_case(
                "#3A6EA5",
                mode == "dark",
                &format!("scheme-{scheme}"),
                false,
                &format!("color-3A6EA5-{mode}-{scheme}.scss"),
            );
        }
    }
}
