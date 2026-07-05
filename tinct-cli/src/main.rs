//! tinct — CLI for the tinct color engine.
//!
//! Drop-in replacement for the illogical-impulse Python color scripts:
//!   tinct generate  ≙ generate_colors_material.py  (same flags, same stdout)
//!   tinct transform ≙ palette_transform.py         (same flags, in-place)
//!   tinct scheme    ≙ scheme_for_image.py
//!
//! Flag names accept both --snake_case (python argparse legacy) and
//! --kebab-case; outputs are byte-identical so existing shell pipelines
//! (switchwall.sh / applycolor.sh) can swap the python calls 1:1.

use anyhow::{bail, Context, Result};
use clap::{Args, Parser, Subcommand};
use std::path::PathBuf;

mod render;

#[derive(Parser)]
#[command(name = "tinct", version, about = "Image/color → Material You theming engine")]
struct Cli {
    #[command(subcommand)]
    cmd: Cmd,
}

#[derive(Subcommand)]
enum Cmd {
    /// Generate a Material palette (+ terminal colors) from an image or color
    Generate(GenerateArgs),
    /// Transform a palette file in place (theory/style/practical/remap)
    Transform(TransformArgs),
    /// Pick a scheme for an image from its colorfulness
    Scheme(SchemeArgs),
    /// Render matugen-config templates from a colors.json
    Render(RenderArgs),
}

#[derive(Args)]
struct GenerateArgs {
    /// Generate from an image
    #[arg(long)]
    path: Option<PathBuf>,
    /// Generate from a color ("#RRGGBB")
    #[arg(long)]
    color: Option<String>,
    /// Bitmap size for quantization
    #[arg(long, default_value_t = 128)]
    size: u32,
    #[arg(long, default_value = "dark", value_parser = ["dark", "light"])]
    mode: String,
    /// Material scheme (scheme-tonal-spot, scheme-vibrant, …)
    #[arg(long, default_value = "vibrant")]
    scheme: String,
    /// Downgrade to scheme-neutral when the seed has low chroma
    #[arg(long, default_value_t = false)]
    smart: bool,
    #[arg(long, default_value = "opaque", value_parser = ["opaque", "transparent"])]
    transparency: String,
    /// JSON file with the base-16 terminal scheme
    #[arg(long)]
    termscheme: Option<PathBuf>,
    /// (0-1) hue shift towards accent
    #[arg(long, default_value_t = 0.8)]
    harmony: f64,
    /// (0-180) max hue-shift angle
    #[arg(long = "harmonize_threshold", alias = "harmonize-threshold", default_value_t = 100.0)]
    harmonize_threshold: f64,
    /// Extra terminal fg/bg separation
    #[arg(long = "term_fg_boost", alias = "term-fg-boost", default_value_t = 0.35)]
    term_fg_boost: f64,
    /// Blend term0/term15 towards surface colors
    #[arg(long = "blend_bg_fg", alias = "blend-bg-fg", default_value_t = false)]
    blend_bg_fg: bool,
    /// Write the seed hex to this file
    #[arg(long)]
    cache: Option<PathBuf>,
    /// Contrast level (python pipeline always used 0.0)
    #[arg(long, default_value_t = 0.0)]
    contrast: f64,
    /// Write the SCSS to a file instead of stdout
    #[arg(long = "out-scss")]
    out_scss: Option<PathBuf>,
    /// Additionally write a flat {role: hex} JSON file
    #[arg(long = "out-json")]
    out_json: Option<PathBuf>,
    /// Print flat JSON to stdout instead of SCSS
    #[arg(long, default_value_t = false)]
    json: bool,
    /// Debug output (seed + HCT info)
    #[arg(long, default_value_t = false)]
    debug: bool,
}

#[derive(Args)]
struct TransformArgs {
    #[arg(long, default_value = "")]
    theory: String,
    #[arg(long, default_value = "")]
    style: String,
    #[arg(long, default_value = "")]
    practical: String,
    #[arg(long, default_value = "")]
    remap: String,
    /// matugen-style colors.json to transform in place
    #[arg(long, default_value = "")]
    json: String,
    /// material_colors.scss to transform in place
    #[arg(long, default_value = "")]
    scss: String,
    /// Step order, e.g. "remap,theory,style"
    #[arg(long = "mix-order", alias = "mix_order", default_value = "")]
    mix_order: String,
}

#[derive(Args)]
struct RenderArgs {
    /// matugen config.toml (templates list)
    #[arg(long, default_value = "~/.config/matugen/config.toml")]
    config: String,
    /// Flat colors.json to render from
    #[arg(long, default_value = "~/.local/state/quickshell/user/generated/colors.json")]
    colors: String,
    /// dark | light (default: detect via gsettings)
    #[arg(long)]
    mode: Option<String>,
    /// Value for {{image}} (default: state wallpaper path.txt)
    #[arg(long)]
    image: Option<String>,
    /// Comma-separated template names to render
    #[arg(long)]
    only: Option<String>,
    /// Skip per-template post_hook commands
    #[arg(long = "no-post-hooks", default_value_t = false)]
    no_post_hooks: bool,
    /// Skip the cmus 256-color theme rebuild
    #[arg(long = "no-cmus", default_value_t = false)]
    no_cmus: bool,
}

#[derive(Args)]
struct SchemeArgs {
    image: PathBuf,
    /// Print the raw colorfulness score instead of a scheme name
    #[arg(long, default_value_t = false)]
    colorfulness: bool,
}

fn main() -> Result<()> {
    match Cli::parse().cmd {
        Cmd::Generate(a) => generate(a),
        Cmd::Transform(a) => transform(a),
        Cmd::Scheme(a) => scheme(a),
        Cmd::Render(a) => render::run(&render::RenderOptions {
            config: expand_home_str(&a.config),
            colors: expand_home_str(&a.colors),
            mode: a.mode,
            image: a.image,
            only: a.only.map(|s| s.split(',').map(|x| x.trim().to_string()).collect()),
            run_post_hooks: !a.no_post_hooks,
            cmus: !a.no_cmus,
        }),
    }
}

fn expand_home_str(p: &str) -> PathBuf {
    if let Some(rest) = p.strip_prefix("~/") {
        if let Some(home) = std::env::var_os("HOME") {
            return PathBuf::from(home).join(rest);
        }
    }
    PathBuf::from(p)
}

fn read_termscheme(path: &PathBuf, dark: bool) -> Result<tinct::Base16> {
    let raw = std::fs::read_to_string(path)
        .with_context(|| format!("reading termscheme {}", path.display()))?;
    let v: serde_json::Value = serde_json::from_str(&raw)?;
    let section = v
        .get(if dark { "dark" } else { "light" })
        .context("termscheme missing dark/light section")?;
    let obj = section.as_object().context("termscheme section not an object")?;
    Ok(obj
        .iter()
        .map(|(k, v)| (k.clone(), v.as_str().unwrap_or_default().to_string()))
        .collect())
}

fn generate(a: GenerateArgs) -> Result<()> {
    let dark = a.mode == "dark";
    let transparent = a.transparency == "transparent";

    let (seed, mut scheme_name) = if let Some(path) = &a.path {
        let opts = tinct::ExtractOptions { bitmap_size: a.size, max_colors: 128 };
        let seed = tinct::seed_from_path(path, &opts)
            .map_err(|e| anyhow::anyhow!("extracting seed from {}: {e}", path.display()))?;
        (seed, a.scheme.clone())
    } else if let Some(color) = &a.color {
        let argb = tinct::hex_to_argb(color).map_err(|e| anyhow::anyhow!("{e}"))?;
        (tinct::extract::Seed::from_argb(argb), a.scheme.clone())
    } else {
        bail!("either --path or --color is required");
    };

    if let Some(cache) = &a.cache {
        std::fs::write(cache, tinct::argb_to_hex(seed.argb))?;
    }
    // --smart only applies to image seeds, like the python script.
    if a.path.is_some() && a.smart && seed.chroma < 20.0 {
        scheme_name = "neutral".into();
    }

    let variant = tinct::SchemeVariant::from_cli_name(&scheme_name);
    let palette = tinct::generate_palette(seed.argb, variant, dark, a.contrast);

    let terms = match &a.termscheme {
        Some(path) => {
            let base = read_termscheme(path, dark)?;
            let opts = tinct::TerminalOptions {
                harmony: a.harmony,
                harmonize_threshold: a.harmonize_threshold,
                term_fg_boost: a.term_fg_boost,
                blend_bg_fg: a.blend_bg_fg,
                // Python-parity: its passthrough check compared against the
                // literal "monochrome", which callers never pass.
                passthrough: scheme_name == "monochrome",
            };
            tinct::derive_terminal(&palette, &base, &opts)
        }
        None => Vec::new(),
    };

    if a.debug {
        println!("seed: {}", tinct::argb_to_hex(seed.argb));
        println!("HCT: {:.2} {:.2} {:.2}", seed.hue, seed.chroma, seed.tone);
        println!("scheme: {}", variant.cli_name());
        return Ok(());
    }

    let scss = tinct::output::to_scss(&palette, transparent, &terms);
    if let Some(out) = &a.out_scss {
        std::fs::write(out, &scss)?;
    }
    if let Some(out) = &a.out_json {
        std::fs::write(out, tinct::output::to_matugen_colors_json(&palette))?;
    }
    if a.json {
        println!("{}", tinct::output::to_flat_json(&palette, &terms));
    } else if a.out_scss.is_none() {
        print!("{}", scss);
    }
    Ok(())
}

fn build_transform_config(a: &TransformArgs) -> tinct::transform::TransformConfig {
    use tinct::transform::*;
    let remap = if !a.remap.is_empty() && a.remap != "matugen" {
        builtin_palette(&a.remap)
    } else {
        None
    };
    TransformConfig {
        theory: Theory::from_name(&a.theory),
        style: Style::from_name(&a.style),
        practical: Practical::from_name(&a.practical),
        remap,
        order: resolve_order(&a.mix_order),
    }
}

fn transform(a: TransformArgs) -> Result<()> {
    // Same short-circuit as palette_transform.py.
    if (a.theory.is_empty() || a.theory == "material")
        && (a.style.is_empty() || a.style == "material")
        && (a.practical.is_empty() || a.practical == "none")
        && a.remap.is_empty()
    {
        return Ok(());
    }
    let cfg = build_transform_config(&a);

    if !a.json.is_empty() {
        transform_json_file(&a.json, &cfg)?;
    }
    if !a.scss.is_empty() {
        transform_scss_file(&a.scss, &cfg)?;
    }
    Ok(())
}

/// In-place transform of matugen's colors.json — python parity: 2-space
/// indent, key order preserved, only "#..." string values touched.
fn transform_json_file(path: &str, cfg: &tinct::transform::TransformConfig) -> Result<()> {
    let p = std::path::Path::new(path);
    if !p.exists() {
        return Ok(());
    }
    let data: serde_json::Value = serde_json::from_str(&std::fs::read_to_string(p)?)?;
    let Some(obj) = data.as_object() else { return Ok(()) };
    let Some(primary) = obj.get("primary").and_then(|v| v.as_str()) else {
        return Ok(());
    };
    let Some(primary_hue) = tinct::transform::hls_hue(primary) else {
        return Ok(());
    };
    let mut out = serde_json::Map::new();
    for (k, v) in obj {
        match v.as_str() {
            Some(s) if s.starts_with('#') => {
                let t = tinct::transform::transform_one(k, s, primary_hue, cfg);
                out.insert(k.clone(), serde_json::Value::String(t));
            }
            _ => {
                out.insert(k.clone(), v.clone());
            }
        }
    }
    let mut text = serde_json::to_string_pretty(&serde_json::Value::Object(out))?;
    text.push('\n');
    std::fs::write(p, text)?;
    Ok(())
}

/// In-place transform of a "$key: #hex;" scss file — python parity: key case
/// kept, other lines untouched, output hex lowercase.
fn transform_scss_file(path: &str, cfg: &tinct::transform::TransformConfig) -> Result<()> {
    let p = std::path::Path::new(path);
    if !p.exists() {
        return Ok(());
    }
    let text = std::fs::read_to_string(p)?;

    let parse = |line: &str| -> Option<(String, String)> {
        let rest = line.strip_prefix('$')?;
        let (k, v) = rest.split_once(':')?;
        let v = v.trim().strip_suffix(';')?.trim();
        if !v.starts_with('#') || v.len() < 4 || v.len() > 9 {
            return None;
        }
        if !v[1..].bytes().all(|b| b.is_ascii_hexdigit()) {
            return None;
        }
        Some((k.trim().to_string(), v.to_string()))
    };

    // Pass 1: find primary (pre-transform), like the python script.
    let mut primary_hue = None;
    for line in text.lines() {
        if let Some((k, v)) = parse(line) {
            if to_snake(&k) == "primary" {
                primary_hue = tinct::transform::hls_hue(&v);
                break;
            }
        }
    }
    let Some(primary_hue) = primary_hue else { return Ok(()) };

    let mut out_lines = Vec::new();
    for line in text.lines() {
        match parse(line) {
            Some((k, v)) => {
                let t = tinct::transform::transform_one(&k, &v, primary_hue, cfg);
                out_lines.push(format!("${}: {};", k, t));
            }
            None => out_lines.push(line.to_string()),
        }
    }
    std::fs::write(p, out_lines.join("\n") + "\n")?;
    Ok(())
}

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

fn scheme(a: SchemeArgs) -> Result<()> {
    let bytes = std::fs::read(&a.image)?;
    let img = image::load_from_memory(&bytes)?;
    // Match scheme_for_image.py: downscale to max dim 128 before scoring.
    let img = img.resize(128, 128, image::imageops::FilterType::Triangle);
    let rgb = img.to_rgb8();
    let score = tinct::metrics::colorfulness_rgb(
        rgb.pixels().map(|p| (p.0[0] as f64, p.0[1] as f64, p.0[2] as f64)),
    );
    if a.colorfulness {
        println!("{score}");
    } else {
        println!("{}", tinct::metrics::pick_scheme(score).cli_name());
    }
    Ok(())
}
