# tinct

**Image/color → Material You theming engine, in Rust.** Extract a Material
You palette from any wallpaper or seed color, harmonize your terminal's ANSI
colors toward it, reshape the palette with composable transforms, and render
it into every app config you have — in about a millisecond, as a single
static binary that runs on any Linux.

```
$ tinct generate --path wallpaper.jpg --mode dark --scheme scheme-vibrant \
      --termscheme base16.json
$darkmode: True;
$primary: #9FCAFF;
$onPrimary: #003259;
…
$term0: #343434;
$term15: #FFFFFF;
```

## What it does

- **Extract** — Celebi quantization + Material scoring picks the dominant
  accent of an image (`--path`), or start from a hex seed (`--color`).
- **Schemes** — all nine Material You scheme variants (tonal-spot, vibrant,
  expressive, fruit-salad, monochrome, rainbow, neutral, fidelity, content),
  dark and light.
- **Terminal colors** — harmonizes a base-16 ANSI scheme toward the palette's
  key color in HCT space (`--harmony`, `--harmonize_threshold`,
  `--term_fg_boost`, `--blend_bg_fg`).
- **Transforms** — color-theory rotations (mono/analogous/complementary/
  triadic/split/tetradic), style filters (pastel/muted/bright/colorful),
  practical passes (high-contrast/duotone) and nearest-color remaps onto 58
  named palettes (catppuccin, gruvbox, nord, …), applied in any order
  (`--mix-order`).
- **Render** — a matugen-compatible template renderer: reads your existing
  matugen `config.toml` and renders `{{colors.<role>.<mode>.<format>}}`
  templates for GTK, Hyprland, nvim, wezterm, tmux, KDE, and anything else —
  no matugen binary required.
- **Auto scheme** — `tinct scheme img.jpg` picks a scheme from the image's
  Hasler–Süsstrunk colorfulness.

## Why another Material You tool?

tinct started as a byte-exact Rust port of the
[illogical-impulse](https://github.com/end-4/dots-hyprland) Python color
pipeline (`materialyoucolor` + matugen + transform scripts). The golden test
suite pins **50 fixtures byte-identical** to the reference implementation —
including its exact quirks — so it can drop into an existing rice as a
1:1 replacement: one native call instead of several Python interpreter
launches and matugen subprocesses (~400 ms → ~1 ms for template rendering).

The engine is also a plain Rust library (`tinct-rs` on crates.io) with a
bytes-in/strings-out API, no filesystem assumptions, suitable for embedding
in apps (Tauri commands, album-art theming, Android via NDK).

## Install

```sh
# prebuilt static binary (any Linux)
curl -fsSL https://github.com/SettDEF/tinct-rs/releases/latest/download/tinct-x86_64-unknown-linux-musl.tar.gz | tar xz
install -Dm755 tinct ~/.local/bin/tinct

# or from source
cargo install tinct-rs-cli
```

## Library

```rust
let seed = tinct::seed_from_bytes(&album_art, &Default::default())?;
let palette = tinct::generate_palette(seed.argb, SchemeVariant::TonalSpot, true, 0.0);
let accent = palette.get("primary");
```

```toml
[dependencies]
tinct = { package = "tinct-rs", version = "0.1" }
```

MSRV of the core crate: 1.77.2.

## Repository layout

| crate | crates.io | what |
|---|---|---|
| `tinct/` | `tinct-rs` | core library: extraction, schemes, terminal, transforms, outputs |
| `tinct-cli/` | `tinct-rs-cli` | the `tinct` binary: generate / transform / scheme / render |

## License

MIT
