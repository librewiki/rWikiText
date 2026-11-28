//! Converts between MediaWiki wikitext, NamuMark, GitHub Flavored Markdown (GFM), and HTML.
//!
//! Page metadata (categories, sort keys, redirects, display title) is returned separately
//! from the converted body. Lossy constructs, stripped HTML, lookup errors, and
//! expansion-limit hits are reported as UTF-8 byte-span diagnostics.
//!
//! # Supported Format Pairs
//!
//! | Source | Targets |
//! | --- | --- |
//! | [`Format::Wikitext`] | [`Format::Gfm`], [`Format::Html`], [`Format::NamuMark`] |
//! | [`Format::NamuMark`] | [`Format::Wikitext`], [`Format::Gfm`], [`Format::Html`] |
//! | [`Format::Gfm`] | [`Format::Wikitext`], [`Format::NamuMark`] |
//! | [`Format::Html`] | [`Format::Wikitext`], [`Format::NamuMark`] |
//!
//! Identity conversions (`source == target`) and direct [`Format::Gfm`] ↔ [`Format::Html`]
//! conversions return [`Err`] with a [`ConversionError`].
//!
//! # Quick Start
//!
//! ```rust
//! use rwikitext::{convert, ConversionRequest, Format, WikiContext};
//!
//! let context = WikiContext {
//!     page_title: "Guide",
//!     ..WikiContext::default()
//! };
//! let result = convert(ConversionRequest {
//!     source: Format::Wikitext,
//!     target: Format::Gfm,
//!     input: "== Overview ==\n'''Bold''' text on {{PAGENAME}}\n[[Category:Docs|Guide]]",
//!     context: &context,
//! })?;
//!
//! assert!(result.content.contains("## Overview"));
//! assert!(result.content.contains("**Bold** text on Guide"));
//! assert_eq!(result.metadata.categories, vec!["Docs"]);
//! assert_eq!(result.metadata.category_sort_keys, vec![Some("Guide".to_string())]);
//! # Ok::<(), rwikitext::ConversionError>(())
//! ```
//!
//! # Wiki Data Lookups and Strict Mode
//!
//! Pass a [`WikiData`] implementation via [`ConversionOptions`] to expand MediaWiki
//! templates, check link targets, and resolve file info. Set [`ConversionMode::Strict`]
//! to fail whenever a construct would lose meaning.
//!
//! ```rust
//! use rwikitext::{
//!     convert_with_options, ConversionMode, ConversionOptions, ConversionRequest, FileInfo,
//!     Format, LookupError, WikiContext, WikiData,
//! };
//!
//! struct MemoryWiki;
//!
//! impl WikiData for MemoryWiki {
//!     fn template_text(&self, title: &str) -> Result<Option<String>, LookupError> {
//!         Ok((title == "Template:Greeting").then(|| "Hello, {{{1}}}!".to_string()))
//!     }
//!
//!     fn page_exists(&self, title: &str) -> Result<Option<bool>, LookupError> {
//!         Ok(Some(title == "Main_Page"))
//!     }
//!
//!     fn file_info(&self, title: &str) -> Result<Option<FileInfo>, LookupError> {
//!         Ok((title == "File:Logo.png").then(|| FileInfo {
//!             url: "https://example.org/logo.png".to_string(),
//!             width: Some(200),
//!             height: Some(100),
//!         }))
//!     }
//! }
//!
//! let context = WikiContext {
//!     page_title: "Home",
//!     ..WikiContext::default()
//! };
//! let wiki = MemoryWiki;
//! let result = convert_with_options(
//!     ConversionRequest {
//!         source: Format::Wikitext,
//!         target: Format::Html,
//!         input: "{{Greeting|{{PAGENAME}}}} [[File:Logo.png]]",
//!         context: &context,
//!     },
//!     ConversionOptions {
//!         mode: ConversionMode::BestEffort,
//!         wiki_data: Some(&wiki),
//!         ..ConversionOptions::default()
//!     },
//! )?;
//!
//! assert!(result.content.contains("Hello, Home!"));
//! assert!(result.content.contains("https://example.org/logo.png"));
//! # Ok::<(), rwikitext::ConversionError>(())
//! ```

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

/// Document format for conversion input or output.
///
/// [`FromStr`] accepts `"wikitext"`, `"namumark"`, `"gfm"`, and `"html"`.
///
/// # Examples
///
/// ```rust
/// use rwikitext::Format;
///
/// assert_eq!("wikitext".parse::<Format>()?, Format::Wikitext);
/// assert_eq!("namumark".parse::<Format>()?, Format::NamuMark);
/// assert_eq!("gfm".parse::<Format>()?, Format::Gfm);
/// assert_eq!("html".parse::<Format>()?, Format::Html);
/// # Ok::<(), rwikitext::ConversionError>(())
/// ```
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Format {
    /// MediaWiki wikitext.
    Wikitext,
    /// GitHub Flavored Markdown (GFM).
    Gfm,
    /// HTML (Parsoid-compatible when converting from MediaWiki wikitext).
    Html,
    /// NamuMark wiki markup.
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

/// Page and site settings used during conversion.
///
/// MediaWiki magic words such as `{{PAGENAME}}`, `{{CONTENTLANGUAGE}}`, and
/// `{{CURRENTTIMESTAMP}}` read from these fields.
///
/// Defaults: `page_title` is `""`, `site_language` is `"en"`, and
/// `reference_time` is `"1970-01-01T00:00:00Z"`.
#[derive(Debug, Clone, Copy)]
pub struct WikiContext<'a> {
    /// Current page title (e.g. `"Guide"` or `"Help:Contents"`).
    pub page_title: &'a str,
    /// Site language code (e.g. `"en"`, `"ko"`).
    pub site_language: &'a str,
    /// Reference UTC timestamp in RFC 3339 format (e.g. `"2025-03-04T05:06:07Z"`).
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

/// Document text, formats, and context for a conversion.
#[derive(Clone, Copy)]
pub struct ConversionRequest<'a> {
    /// Format of `input`.
    pub source: Format,
    /// Target output format.
    pub target: Format,
    /// UTF-8 document text to convert.
    pub input: &'a str,
    /// Page and site context.
    pub context: &'a WikiContext<'a>,
}

/// Page metadata extracted outside the rendered document body.
#[derive(Debug, Default, Clone, PartialEq, Eq)]
pub struct PageMetadata {
    /// Category names in document order.
    pub categories: Vec<String>,
    /// Sort keys parallel to `categories` (`None` when omitted).
    pub category_sort_keys: Vec<Option<String>>,
    /// Redirect destination page title, if the document is a redirect.
    pub redirect_target: Option<String>,
    /// Custom display title set by `{{DISPLAYTITLE:...}}`.
    pub display_title: Option<String>,
}

/// How conversion loss is handled.
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub enum ConversionMode {
    /// Finish converting and record lossy constructs in [`ConversionResult::diagnostics`].
    #[default]
    BestEffort,
    /// Fail with [`ConversionError`] if any construct would lose meaning.
    Strict,
}

/// Category of a [`Diagnostic`].
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DiagnosticKind {
    /// A source construct could not be represented losslessly in the target format.
    ConversionLoss,
    /// Disallowed raw HTML was stripped.
    HtmlRemoval,
    /// A [`WikiData`] lookup failed or returned an invalid reply.
    WikiLookup,
    /// MediaWiki expansion exceeded an [`ExpansionLimits`] bound.
    ExpansionLimit,
}

/// Warning or error tied to a byte span in the original input.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Diagnostic {
    /// Diagnostic category.
    pub kind: DiagnosticKind,
    /// Half-open UTF-8 byte range (`start..end`) in the original input.
    pub span: Range<usize>,
    /// Description of the issue.
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

/// Output of a successful conversion.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ConversionResult {
    /// Converted text in the target format.
    pub content: String,
    /// Page metadata extracted from the source.
    pub metadata: PageMetadata,
    /// Non-fatal diagnostics recorded during conversion.
    pub diagnostics: Vec<Diagnostic>,
}

/// Error returned when conversion fails.
///
/// Returned for unsupported format pairs, strict-mode conversion loss,
/// [`WikiData`] lookup errors, and exceeded [`ExpansionLimits`].
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

    /// Diagnostics associated with this error. Empty for invalid requests or unsupported format pairs.
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

/// Converts a document using [`ConversionMode::BestEffort`] and default [`ConversionOptions`].
///
/// # Errors
///
/// Returns [`ConversionError`] if the `(source, target)` format pair is unsupported
/// or if expansion exceeds the default [`ExpansionLimits`].
///
/// # Examples
///
/// ```rust
/// use rwikitext::{convert, ConversionRequest, Format, WikiContext};
///
/// let context = WikiContext::default();
/// let result = convert(ConversionRequest {
///     source: Format::NamuMark,
///     target: Format::Gfm,
///     input: "== Hello ==\n'''World'''",
///     context: &context,
/// })?;
/// assert!(result.content.contains("## Hello"));
/// assert!(result.content.contains("**World**"));
/// # Ok::<(), rwikitext::ConversionError>(())
/// ```
pub fn convert(request: ConversionRequest<'_>) -> Result<ConversionResult, ConversionError> {
    convert_with_mode(request, ConversionMode::BestEffort)
}

/// Converts a document with an explicit [`ConversionMode`] and default wiki data and limits.
///
/// # Errors
///
/// Returns [`ConversionError`] if the format pair is unsupported, if `mode` is
/// [`ConversionMode::Strict`] and any conversion loss occurs, or if expansion exceeds
/// the default [`ExpansionLimits`].
///
/// # Examples
///
/// ```rust
/// use rwikitext::{
///     convert_with_mode, ConversionMode, ConversionRequest, DiagnosticKind, Format, WikiContext,
/// };
///
/// let context = WikiContext::default();
/// let err = convert_with_mode(
///     ConversionRequest {
///         source: Format::Wikitext,
///         target: Format::Gfm,
///         input: "[[Category:Demo]]",
///         context: &context,
///     },
///     ConversionMode::Strict,
/// )
/// .unwrap_err();
/// assert_eq!(err.diagnostics()[0].kind, DiagnosticKind::ConversionLoss);
/// ```
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

/// Converts a document with custom mode, wiki data, and expansion limits.
///
/// # Errors
///
/// Fails with [`ConversionError`] when the format pair is unsupported, `mode` is
/// [`ConversionMode::Strict`] and conversion loss occurs, a [`WikiData`] lookup
/// returns [`Err`], or expansion exceeds `options.expansion_limits`.
///
/// # Examples
///
/// ```rust
/// use rwikitext::{
///     convert_with_options, ConversionOptions, ConversionRequest, ExpansionLimits, Format,
///     WikiContext,
/// };
///
/// let context = WikiContext::default();
/// let result = convert_with_options(
///     ConversionRequest {
///         source: Format::Gfm,
///         target: Format::Wikitext,
///         input: "**Bold**",
///         context: &context,
///     },
///     ConversionOptions {
///         expansion_limits: ExpansionLimits {
///             max_depth: 10,
///             ..ExpansionLimits::default()
///         },
///         ..ConversionOptions::default()
///     },
/// )?;
/// assert!(result.content.contains("'''Bold'''"));
/// # Ok::<(), rwikitext::ConversionError>(())
/// ```
pub fn convert_with_options(
    request: ConversionRequest<'_>,
    options: ConversionOptions<'_>,
) -> Result<ConversionResult, ConversionError> {
    pipeline::convert(request, options)
}
