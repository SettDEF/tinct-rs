//! Transform parity: rust transform_entries vs palette_transform.py output
//! on the same source scss, byte-identical.

use tinct::transform::{
    builtin_palette, resolve_order, transform_entries, Practical, Style, Theory, TransformConfig,
};

fn parse_scss(text: &str) -> Vec<(String, String)> {
    text.lines()
        .filter_map(|l| {
            let l = l.trim_end();
            let rest = l.strip_prefix('$')?;
            let (k, v) = rest.split_once(':')?;
            let v = v.trim().strip_suffix(';')?;
            Some((k.trim().to_string(), v.trim().to_string()))
        })
        .collect()
}

fn serialize_scss(entries: &[(String, String)]) -> String {
    let mut out = String::new();
    for (k, v) in entries {
        out.push_str(&format!("${}: {};\n", k, v));
    }
    out
}

fn run(cfg: TransformConfig, fixture: &str) {
    let src = include_str!("fixtures/full-B52755-dark-tonal-spot.scss");
    let mut entries = parse_scss(src);
    transform_entries(&mut entries, &cfg);
    let got = serialize_scss(&entries);
    let want = std::fs::read_to_string(format!("tests/fixtures/{fixture}")).unwrap();
    if got != want {
        for (g, w) in got.lines().zip(want.lines()) {
            if g != w {
                eprintln!("MISMATCH [{fixture}]: got `{g}` want `{w}`");
            }
        }
    }
    assert_eq!(got, want, "{fixture} diverged");
}

#[test]
fn triadic_pastel() {
    run(
        TransformConfig {
            theory: Theory::from_name("triadic"),
            style: Style::from_name("pastel"),
            ..Default::default()
        },
        "tf-1.scss",
    );
}

#[test]
fn mono() {
    run(TransformConfig { theory: Theory::from_name("mono"), ..Default::default() }, "tf-2.scss");
}

#[test]
fn high_contrast() {
    run(
        TransformConfig { practical: Practical::from_name("high_contrast"), ..Default::default() },
        "tf-3.scss",
    );
}

#[test]
fn remap_then_theory() {
    run(
        TransformConfig {
            theory: Theory::from_name("analogous"),
            remap: builtin_palette("catppuccin_mocha"),
            order: resolve_order("remap,theory"),
            ..Default::default()
        },
        "tf-4.scss",
    );
}

#[test]
fn practical_before_style() {
    run(
        TransformConfig {
            style: Style::from_name("bright"),
            practical: Practical::from_name("duotone"),
            order: resolve_order("practical,style"),
            ..Default::default()
        },
        "tf-5.scss",
    );
}
