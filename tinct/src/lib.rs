//! tinct — image/color → Material You palette engine.
//!
//! Rust port of the illogical-impulse Python color pipeline
//! (`generate_colors_material.py`, `palette_transform.py`, `scheme_for_image.py`),
//! byte-compatible at the SCSS/JSON output level so it can replace the Python
//! scripts behind existing shell pipelines, and embeddable as a crate
//! (Tauri/Android-safe: bytes in, strings out).

pub mod color;
pub mod compat;
pub mod extract;
pub mod metrics;
pub mod output;
pub mod palette;
pub mod scheme;
pub mod terminal;
pub mod transform;

pub use color::{argb_to_hex, hex_to_argb};
pub use material_colors::color::Argb;
pub use palette::Palette;
pub use scheme::{generate_palette, SchemeVariant};
pub use terminal::{derive_terminal, Base16, TerminalOptions};

#[cfg(feature = "image-decode")]
pub use extract::{seed_from_bytes, seed_from_path};
pub use extract::{seed_from_argb_pixels, ExtractOptions, Seed};
