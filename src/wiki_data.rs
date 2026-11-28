use crate::{ConversionMode, FileInfo};
use std::fmt;

/// Caller-owned wiki data for MediaWiki expansion and link rendering.
///
/// Lookups run synchronously on the conversion thread. Titles use canonical
/// namespace casing and underscores (e.g. `"Template:Greeting"`, `"Main_Page"`,
/// `"File:Example.png"`).
///
/// `Ok(None)` reports missing data and keeps the converter's offline fallback.
/// `Err(LookupError)` aborts conversion with a
/// [`DiagnosticKind::WikiLookup`](crate::DiagnosticKind::WikiLookup) diagnostic.
/// NamuMark input never calls `WikiData`.
pub trait WikiData {
    /// Raw wikitext for a transcluded template, or `Ok(None)` if missing.
    fn template_text(&self, _title: &str) -> Result<Option<String>, LookupError> {
        Ok(None)
    }

    /// Whether a page exists. `Ok(Some(false))` explicitly marks it absent (red link in HTML);
    /// `Ok(None)` leaves existence unknown.
    fn page_exists(&self, _title: &str) -> Result<Option<bool>, LookupError> {
        Ok(None)
    }

    /// URL and optional dimensions for a wiki file, or `Ok(None)` if unknown.
    fn file_info(&self, _title: &str) -> Result<Option<FileInfo>, LookupError> {
        Ok(None)
    }
}

/// Error returned by a [`WikiData`] lookup.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LookupError(String);

impl LookupError {
    /// Creates a lookup error with the given message.
    pub fn new(message: impl Into<String>) -> Self {
        Self(message.into())
    }
}

impl fmt::Display for LookupError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

impl std::error::Error for LookupError {}

/// Limits for MediaWiki template and parser-function expansion.
///
/// Exceeding any limit aborts conversion with a
/// [`DiagnosticKind::ExpansionLimit`](crate::DiagnosticKind::ExpansionLimit) diagnostic.
///
/// Defaults: `max_depth = 40`, `max_expansions = 10_000`, `max_output_bytes = 2_097_152` (2 MiB).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ExpansionLimits {
    /// Maximum template nesting depth.
    pub max_depth: usize,
    /// Maximum number of template and parser-function expansions.
    pub max_expansions: usize,
    /// Maximum expanded wikitext size in bytes.
    pub max_output_bytes: usize,
}

impl Default for ExpansionLimits {
    fn default() -> Self {
        Self {
            max_depth: 40,
            max_expansions: 10_000,
            max_output_bytes: 2 * 1024 * 1024,
        }
    }
}

/// Options for [`convert_with_options`](crate::convert_with_options).
#[derive(Clone, Copy, Default)]
pub struct ConversionOptions<'a> {
    /// How conversion loss is handled.
    pub mode: ConversionMode,
    /// Optional wiki data provider; `None` reports missing for every lookup.
    pub wiki_data: Option<&'a dyn WikiData>,
    /// Limits for MediaWiki template expansion.
    pub expansion_limits: ExpansionLimits,
}
