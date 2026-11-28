use crate::{ConversionMode, FileInfo};
use std::fmt;

/// Caller-owned wiki data provider for MediaWiki expansion and link rendering.
///
/// Lookups run synchronously on the conversion thread. Every `title` argument uses
/// canonical namespace casing and underscore-separated spelling (for example,
/// `"Template:Greeting"`, `"Main_Page"`, or `"File:Example.png"`).
///
/// Returning `Ok(None)` reports missing data and falls back to the converter's default
/// offline behavior. Returning `Err(LookupError)` aborts the conversion with a located
/// [`DiagnosticKind::WikiLookup`](crate::DiagnosticKind::WikiLookup) diagnostic.
///
/// NamuMark input does not expand MediaWiki templates or invoke `WikiData` callbacks.
pub trait WikiData {
    /// Returns the raw wikitext source of a transcluded template, or `Ok(None)` if missing.
    fn template_text(&self, _title: &str) -> Result<Option<String>, LookupError> {
        Ok(None)
    }

    /// Reports whether the given page exists on the wiki.
    ///
    /// Return `Ok(Some(true))` if the page exists, `Ok(Some(false))` if it is known to be
    /// absent (producing red links in HTML output), or `Ok(None)` if existence is unknown.
    fn page_exists(&self, _title: &str) -> Result<Option<bool>, LookupError> {
        Ok(None)
    }

    /// Returns rendering metadata for a wiki file, or `Ok(None)` if unknown.
    fn file_info(&self, _title: &str) -> Result<Option<FileInfo>, LookupError> {
        Ok(None)
    }
}

/// Error returned by a [`WikiData`] callback when a lookup fails.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LookupError(String);

impl LookupError {
    /// Creates a new lookup error carrying a human-readable diagnostic message.
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

/// Bounds that guard MediaWiki template and parser-function expansion.
///
/// Exceeding any bound aborts the conversion with a
/// [`DiagnosticKind::ExpansionLimit`](crate::DiagnosticKind::ExpansionLimit) diagnostic.
///
/// # Defaults
///
/// - `max_depth`: `40`
/// - `max_expansions`: `10_000`
/// - `max_output_bytes`: `2_097_152` (2 MiB)
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ExpansionLimits {
    /// Maximum nesting depth of recursive template expansions.
    pub max_depth: usize,
    /// Maximum total number of template and parser-function expansions.
    pub max_expansions: usize,
    /// Maximum size in bytes of the expanded wikitext buffer.
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

/// Configuration passed to [`convert_with_options`](crate::convert_with_options).
#[derive(Clone, Copy, Default)]
pub struct ConversionOptions<'a> {
    /// Controls whether conversion loss is reported as diagnostics or fails the call.
    pub mode: ConversionMode,
    /// Optional synchronous wiki data provider; `None` reports missing for all lookups.
    pub wiki_data: Option<&'a dyn WikiData>,
    /// Resource bounds for MediaWiki template expansion.
    pub expansion_limits: ExpansionLimits,
}
