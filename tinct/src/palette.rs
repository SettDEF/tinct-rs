//! Ordered Material role palette.
//!
//! Key names and iteration order replicate `vars(MaterialDynamicColors)` from
//! the Python materialyoucolor library exactly — downstream SCSS/JSON diffs
//! depend on both the mixed-case names (`surfaceContainerLow`,
//! `primary_paletteKeyColor`) and the order.

/// A generated palette: ordered `(role, "#RRGGBB")` pairs, including the
/// extended success roles. Terminal colors live separately (see
/// [`crate::terminal`]) because they're derived from a base-16 seed scheme.
#[derive(Debug, Clone, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct Palette {
    pub dark: bool,
    /// Ordered role → uppercase hex ("#RRGGBB").
    pub entries: Vec<(String, String)>,
}

impl Palette {
    pub fn get(&self, role: &str) -> Option<&str> {
        self.entries
            .iter()
            .find(|(k, _)| k == role)
            .map(|(_, v)| v.as_str())
    }

    pub fn set(&mut self, role: &str, hex: String) {
        match self.entries.iter_mut().find(|(k, _)| k == role) {
            Some((_, v)) => *v = hex,
            None => self.entries.push((role.to_string(), hex)),
        }
    }
}
