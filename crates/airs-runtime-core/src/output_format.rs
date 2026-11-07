//! The output format names (DESIGN §2.4); tenant files validate `defaultOutput` with them.

/// The output formats (`markdown`, `csv` and `yaml` are dropped for good, D50).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Format {
    Pretty,
    Table,
    Json,
}

impl Format {
    pub const ALL: [Format; 3] = [Format::Pretty, Format::Table, Format::Json];

    pub fn name(self) -> &'static str {
        match self {
            Self::Pretty => "pretty",
            Self::Table => "table",
            Self::Json => "json",
        }
    }

    /// The format with exactly this (case-sensitive) name.
    pub fn parse(name: &str) -> Option<Self> {
        Self::ALL.into_iter().find(|format| format.name() == name)
    }

    /// Every valid name, comma+space joined (`pretty, table, json`).
    pub fn valid_names() -> String {
        Self::ALL.map(Self::name).join(", ")
    }
}
