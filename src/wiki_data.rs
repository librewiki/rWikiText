use crate::{ConversionMode, FileInfo};
use std::fmt;

/// Caller-owned wiki data. Lookups run synchronously on the conversion thread.
/// The title uses canonical namespace and underscore-separated spelling.
pub trait WikiData {
    fn template_text(&self, _title: &str) -> Result<Option<String>, LookupError> {
        Ok(None)
    }

    fn page_exists(&self, _title: &str) -> Result<Option<bool>, LookupError> {
        Ok(None)
    }

    fn file_info(&self, _title: &str) -> Result<Option<FileInfo>, LookupError> {
        Ok(None)
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LookupError(String);

impl LookupError {
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

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ExpansionLimits {
    pub max_depth: usize,
    pub max_expansions: usize,
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

#[derive(Clone, Copy, Default)]
pub struct ConversionOptions<'a> {
    pub mode: ConversionMode,
    pub wiki_data: Option<&'a dyn WikiData>,
    pub expansion_limits: ExpansionLimits,
}
