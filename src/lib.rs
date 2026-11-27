mod core;
mod expansion;
mod ffi;
mod functions;
mod html;
mod html_policy;
mod inline;
mod metadata;
mod namumark;
mod namumark_output;
mod pipeline;
mod render;
mod spans;
mod table;
mod transclusion;
mod wiki_data;
mod wiki_links;
mod wiki_markup;
mod wikitext;

pub use wiki_data::{ConversionOptions, ExpansionLimits, LookupError, WikiData};
pub use wiki_links::FileInfo;

use inline::Inline;
use std::fmt;
use std::ops::Range;
use std::str::FromStr;

pub(crate) use html::{embedded_html, html_to_wiki, parse_html_inlines};
pub(crate) use render::{
    escape_ascii_punctuation, escape_wikitext, heading_id, render_inline, HtmlStyle, RenderTarget,
};
pub(crate) use spans::{locate, original_range};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Format {
    Wikitext,
    Gfm,
    Html,
    NamuMark,
}

impl FromStr for Format {
    type Err = ConversionError;
    fn from_str(value: &str) -> Result<Self, Self::Err> {
        match value {
            "wikitext" => Ok(Self::Wikitext),
            "namumark" => Ok(Self::NamuMark),
            "gfm" => Ok(Self::Gfm),
            "html" => Ok(Self::Html),
            _ => Err(ConversionError::new(format!("unknown format: {value}"))),
        }
    }
}

#[derive(Debug, Clone, Copy)]
pub struct WikiContext<'a> {
    pub page_title: &'a str,
    pub site_language: &'a str,
    pub reference_time: &'a str,
}

impl Default for WikiContext<'_> {
    fn default() -> Self {
        Self {
            page_title: "",
            site_language: "en",
            reference_time: "1970-01-01T00:00:00Z",
        }
    }
}

#[derive(Clone, Copy)]
pub struct ConversionRequest<'a> {
    pub source: Format,
    pub target: Format,
    pub input: &'a str,
    pub context: &'a WikiContext<'a>,
}

#[derive(Debug, Default, Clone, PartialEq, Eq)]
pub struct PageMetadata {
    pub categories: Vec<String>,
    pub category_sort_keys: Vec<Option<String>>,
    pub redirect_target: Option<String>,
    pub display_title: Option<String>,
}

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub enum ConversionMode {
    #[default]
    BestEffort,
    Strict,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DiagnosticKind {
    ConversionLoss,
    HtmlRemoval,
    WikiLookup,
    ExpansionLimit,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Diagnostic {
    pub kind: DiagnosticKind,
    /// Half-open byte range in the original input.
    pub span: Range<usize>,
    pub message: String,
}

impl Diagnostic {
    pub(crate) fn new(kind: DiagnosticKind, span: Range<usize>, message: impl Into<String>) -> Self {
        Self {
            kind,
            span,
            message: message.into(),
        }
    }

    pub(crate) fn loss(span: Range<usize>, message: impl Into<String>) -> Self {
        Self::new(DiagnosticKind::ConversionLoss, span, message)
    }
}

impl fmt::Display for Diagnostic {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let kind = match self.kind {
            DiagnosticKind::ConversionLoss => "conversion loss",
            DiagnosticKind::HtmlRemoval => "HTML removal",
            DiagnosticKind::WikiLookup => "wiki lookup error",
            DiagnosticKind::ExpansionLimit => "expansion limit",
        };
        write!(
            f,
            "{kind} at bytes {}..{}: {}",
            self.span.start, self.span.end, self.message
        )
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ConversionResult {
    pub content: String,
    pub metadata: PageMetadata,
    pub diagnostics: Vec<Diagnostic>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ConversionError {
    message: String,
    diagnostics: Vec<Diagnostic>,
}

impl ConversionError {
    pub(crate) fn new(message: impl Into<String>) -> Self {
        Self {
            message: message.into(),
            diagnostics: Vec::new(),
        }
    }

    pub fn diagnostics(&self) -> &[Diagnostic] {
        &self.diagnostics
    }

    pub(crate) fn lookup(
        span: Range<usize>,
        operation: &str,
        title: &str,
        error: LookupError,
    ) -> Self {
        Self::diagnosed(
            DiagnosticKind::WikiLookup,
            span,
            format!("{operation} for {title}: {error}"),
        )
    }

    pub(crate) fn expansion_limit(span: Range<usize>, message: impl Into<String>) -> Self {
        Self::diagnosed(DiagnosticKind::ExpansionLimit, span, message.into())
    }

    pub(crate) fn strict_loss(diagnostics: Vec<Diagnostic>) -> Self {
        Self {
            message: "strict conversion would lose document meaning".into(),
            diagnostics,
        }
    }

    fn diagnosed(kind: DiagnosticKind, span: Range<usize>, message: String) -> Self {
        Self {
            diagnostics: vec![Diagnostic::new(kind, span, &message)],
            message,
        }
    }
}

impl fmt::Display for ConversionError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.message.fmt(f)
    }
}

impl std::error::Error for ConversionError {}

// Blocks retain their order independently of the input and output formats.
pub(crate) struct Document {
    pub blocks: Vec<Block>,
}

pub(crate) enum Block {
    Paragraph(Vec<Inline>),
    Heading { level: u8, text: Vec<Inline> },
    Core(core::Block),
    Table(table::Table),
    Html(String),
}

pub fn convert(request: ConversionRequest<'_>) -> Result<ConversionResult, ConversionError> {
    convert_with_mode(request, ConversionMode::BestEffort)
}

pub fn convert_with_mode(
    request: ConversionRequest<'_>,
    mode: ConversionMode,
) -> Result<ConversionResult, ConversionError> {
    convert_with_options(
        request,
        ConversionOptions {
            mode,
            ..ConversionOptions::default()
        },
    )
}

pub fn convert_with_options(
    request: ConversionRequest<'_>,
    options: ConversionOptions<'_>,
) -> Result<ConversionResult, ConversionError> {
    pipeline::convert(request, options)
}
