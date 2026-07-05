//! `tinct render` — replacement for rebuild_templates.py + the matugen
//! binary: reads the matugen config.toml unchanged, synthesizes the nested
//! {colors: {key: {default/dark/light: {hex, hex_stripped, rgb, rgba}}}}
//! data and renders every template with a mini {{...}} engine (the installed
//! templates use only the colors/image subset — verified by enumeration).
//!
//! Opposite-mode colors come from material-colors' ThemeBuilder (TonalSpot),
//! mirroring rebuild_templates.py's `matugen color hex <source>` call.

use anyhow::{Context, Result};
use material_colors::theme::ThemeBuilder;
use std::collections::HashMap;
use std::path::{Path, PathBuf};

pub struct RenderOptions {
    pub config: PathBuf,
    pub colors: PathBuf,
    /// "dark" | "light" | None (= detect via gsettings, python parity)
    pub mode: Option<String>,
    /// {{image}} value; None → read state wallpaper path.txt like the python
    pub image: Option<String>,
    /// Restrict to these template names
    pub only: Option<Vec<String>>,
    pub run_post_hooks: bool,
    pub cmus: bool,
}

fn expand_home(p: &str) -> PathBuf {
    if let Some(rest) = p.strip_prefix("~/") {
        if let Some(home) = std::env::var_os("HOME") {
            return PathBuf::from(home).join(rest);
        }
    }
    PathBuf::from(p)
}

fn detect_dark() -> bool {
    // python: `"light" not in gsettings output` → dark
    std::process::Command::new("gsettings")
        .args(["get", "org.gnome.desktop.interface", "color-scheme"])
        .output()
        .map(|o| !String::from_utf8_lossy(&o.stdout).to_lowercase().contains("light"))
        .unwrap_or(true)
}

struct ColorFields {
    hex: String,
    hex_stripped: String,
    rgb: String,
    rgba: String,
}

fn fields(hex: &str) -> ColorFields {
    let stripped = hex.trim_start_matches('#');
    let stripped = if stripped.len() == 8 { &stripped[..6] } else { stripped };
    let c = |i: usize| u8::from_str_radix(&stripped[i..i + 2], 16).unwrap_or(0);
    let (r, g, b) = (c(0), c(2), c(4));
    ColorFields {
        hex: hex.to_string(),
        hex_stripped: stripped.to_string(),
        rgb: format!("{r}, {g}, {b}"),
        rgba: format!("{r}, {g}, {b}, 255"),
    }
}

/// key → (default, dark, light) hex
type NestedColors = HashMap<String, (String, String, String)>;

fn build_nested(colors: &serde_json::Map<String, serde_json::Value>, dark: bool) -> NestedColors {
    // Opposite palette from the source color, like `matugen color hex`.
    let source = colors
        .get("source_color")
        .or_else(|| colors.get("primary"))
        .and_then(|v| v.as_str())
        .unwrap_or("#bac2de")
        .to_string();
    let opposite: HashMap<String, String> = tinct::hex_to_argb(&source)
        .ok()
        .map(|argb| {
            let theme = ThemeBuilder::with_source(argb).build();
            let scheme = if dark { theme.schemes.light } else { theme.schemes.dark };
            scheme
                .into_iter()
                .map(|(k, v)| (k, v.to_hex_with_pound()))
                .collect()
        })
        .unwrap_or_default();

    let mut nested = NestedColors::new();
    for (key, value) in colors {
        let Some(hex) = value.as_str() else { continue };
        if !hex.starts_with('#') {
            continue;
        }
        let opp = opposite.get(key).cloned().unwrap_or_else(|| hex.to_string());
        let (dark_hex, light_hex) = if dark {
            (hex.to_string(), opp)
        } else {
            (opp, hex.to_string())
        };
        nested.insert(key.clone(), (hex.to_string(), dark_hex, light_hex));
    }
    // rebuild_templates.py synthesizes a source_color entry (fallback:
    // primary, else first color value) so {{colors.source_color…}} resolves.
    if !nested.contains_key("source_color") {
        let fallback = colors
            .get("primary")
            .and_then(|v| v.as_str())
            .or_else(|| {
                colors.values().find_map(|v| v.as_str().filter(|s| s.starts_with('#')))
            })
            .map(str::to_string);
        if let Some(hex) = fallback {
            nested.insert("source_color".into(), (hex.clone(), hex.clone(), hex));
        }
    }
    nested
}

/// Render {{ colors.<key>.<mode>.<fmt> }} and {{ image }} expressions.
fn render_template(input: &str, nested: &NestedColors, image: &str) -> (String, Vec<String>) {
    let mut out = String::with_capacity(input.len());
    let mut missing = Vec::new();
    let mut rest = input;

    while let Some(start) = rest.find("{{") {
        out.push_str(&rest[..start]);
        let after = &rest[start + 2..];
        let Some(end) = after.find("}}") else {
            out.push_str(&rest[start..]);
            rest = "";
            break;
        };
        let expr = after[..end].trim();
        let replacement = if expr == "image" {
            Some(image.to_string())
        } else if let Some(path) = expr.strip_prefix("colors.") {
            let parts: Vec<&str> = path.split('.').collect();
            if parts.len() == 3 {
                nested.get(parts[0]).and_then(|(default, dark, light)| {
                    let hex = match parts[1] {
                        "default" => default,
                        "dark" => dark,
                        "light" => light,
                        _ => return None,
                    };
                    let f = fields(hex);
                    match parts[2] {
                        "hex" => Some(f.hex),
                        "hex_stripped" => Some(f.hex_stripped),
                        "rgb" => Some(f.rgb),
                        "rgba" => Some(f.rgba),
                        _ => None,
                    }
                })
            } else {
                None
            }
        } else {
            None
        };
        match replacement {
            Some(r) => out.push_str(&r),
            None => {
                // Leave unknown expressions verbatim, but report them.
                missing.push(expr.to_string());
                out.push_str(&rest[start..start + 2 + end + 2]);
            }
        }
        rest = &after[end + 2..];
    }
    out.push_str(rest);
    (out, missing)
}

pub fn run(opts: &RenderOptions) -> Result<()> {
    let config_text = std::fs::read_to_string(&opts.config)
        .with_context(|| format!("reading {}", opts.config.display()))?;
    let config: toml::Value = toml::from_str(&config_text).context("parsing matugen config")?;

    let colors_text = std::fs::read_to_string(&opts.colors)
        .with_context(|| format!("reading {}", opts.colors.display()))?;
    let colors: serde_json::Map<String, serde_json::Value> =
        serde_json::from_str(&colors_text).context("parsing colors json")?;

    let dark = match opts.mode.as_deref() {
        Some("dark") => true,
        Some("light") => false,
        _ => detect_dark(),
    };

    let image = match &opts.image {
        Some(i) => i.clone(),
        None => {
            let p = expand_home("~/.local/state/quickshell/user/generated/wallpaper/path.txt");
            std::fs::read_to_string(p).map(|s| s.trim().to_string()).unwrap_or_default()
        }
    };

    let nested = build_nested(&colors, dark);

    let templates = config
        .get("templates")
        .and_then(|t| t.as_table())
        .context("no [templates] in config")?;

    let mut rendered = 0usize;
    for (name, entry) in templates {
        if let Some(only) = &opts.only {
            if !only.contains(name) {
                continue;
            }
        }
        let Some(input_path) = entry.get("input_path").and_then(|v| v.as_str()) else {
            continue;
        };
        let Some(output_path) = entry.get("output_path").and_then(|v| v.as_str()) else {
            continue;
        };
        let input_file = expand_home(input_path);
        let output_file = expand_home(output_path);

        let template = match std::fs::read_to_string(&input_file) {
            Ok(t) => t,
            Err(e) => {
                eprintln!("tinct render: skipping {name}: {e}");
                continue;
            }
        };
        let (output, missing) = render_template(&template, &nested, &image);
        if !missing.is_empty() {
            eprintln!("tinct render: {name}: unresolved {{{{...}}}}: {}", missing.join(", "));
        }
        if let Some(parent) = output_file.parent() {
            std::fs::create_dir_all(parent).ok();
        }
        std::fs::write(&output_file, output)
            .with_context(|| format!("writing {}", output_file.display()))?;
        rendered += 1;

        if opts.run_post_hooks {
            if let Some(hook) = entry.get("post_hook").and_then(|v| v.as_str()) {
                let status = std::process::Command::new("sh").arg("-c").arg(hook).status();
                if let Err(e) = status {
                    eprintln!("tinct render: post_hook for {name} failed to spawn: {e}");
                }
            }
        }
    }

    if opts.cmus {
        rebuild_cmus_theme(&colors);
    }

    println!("tinct render: {rendered} templates rendered");
    Ok(())
}

/// Port of rebuild_templates.py::rebuild_cmus_theme (256-color theme).
fn rebuild_cmus_theme(colors: &serde_json::Map<String, serde_json::Value>) {
    let get = |key: &str, fallback: &str| -> u8 {
        let hex = colors.get(key).and_then(|v| v.as_str()).unwrap_or(fallback);
        tinct::color::hex_to_ansi256_exact(hex).unwrap_or(7)
    };
    let on_surface = get("on_surface", "#e6e1e1");
    let tertiary = get("tertiary", "#8D76AD");
    let primary = get("primary", "#B52755");
    let on_primary = get("on_primary", "#ffffff");
    let sch = get("surface_container_highest", "#4d4b4d");
    let secondary = get("secondary", "#A97363");
    let surface_container = get("surface_container", "#201f20");
    let surface = get("surface", "#141313");
    let error = get("error", "#ffb4ab");
    let surface_variant = get("surface_variant", "#49464a");

    let theme = format!(
        r#"# Matugen-generated theme for cmus
# Matches the active system wallpaper palette (256-color compatible)

# Background and text colors
set color_win_bg=default
set color_win_fg={on_surface}

# Directory and Category headers (views 1 and 2)
set color_win_dir={tertiary}

# Selected row (focused window)
set color_win_sel_bg={primary}
set color_win_sel_fg={on_primary}

# Selected row (unfocused window)
set color_win_inactive_sel_bg={sch}
set color_win_inactive_sel_fg={on_surface}

# Currently playing track (unselected)
set color_win_cur={secondary}

# Currently playing track (selected)
set color_win_cur_sel_bg={primary}
set color_win_cur_sel_fg={on_primary}

# Inactive currently playing track (selected)
set color_win_inactive_cur_sel_bg={sch}
set color_win_inactive_cur_sel_fg={secondary}

# Title line at the top
set color_titleline_bg={surface_container}
set color_titleline_fg={primary}

# Status line at the bottom
set color_statusline_bg={surface_container}
set color_statusline_fg={secondary}

# Command line / search input at the very bottom
set color_cmdline_bg={surface}
set color_cmdline_fg={on_surface}

# Warnings and errors
set color_error={error}
set color_info={secondary}

# Separator lines
set color_separator={surface_variant}
"#
    );
    let path = expand_home("~/.config/cmus/rose-terracotta.theme");
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent).ok();
    }
    if std::fs::write(&path, theme).is_ok() {
        let _ = std::process::Command::new("sh")
            .arg("-c")
            .arg("timeout 1s cmus-remote -C 'colorscheme rose-terracotta' 2>/dev/null || true")
            .status();
    }
}
