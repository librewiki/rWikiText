mod core;
mod html;
mod html_policy;
mod inline;
mod metadata;
mod table;

use inline::Inline;
use std::fmt;
use std::ops::Range;
use std::str::FromStr;

pub(crate) use html::parse_html_inlines;
use html::{embedded_html, html_to_wiki, parse_gfm, parse_html};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Format {
    Wikitext,
    Gfm,
    Html,
}

impl FromStr for Format {
    type Err = ConversionError;
    fn from_str(value: &str) -> Result<Self, Self::Err> {
        match value {
            "wikitext" => Ok(Self::Wikitext),
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
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Diagnostic {
    pub kind: DiagnosticKind,
    /// Half-open byte range in the original input.
    pub span: Range<usize>,
    pub message: String,
}

impl Diagnostic {
    fn new(kind: DiagnosticKind, span: Range<usize>, message: impl Into<String>) -> Self {
        Self {
            kind,
            span,
            message: message.into(),
        }
    }

    fn loss(span: Range<usize>, message: impl Into<String>) -> Self {
        Self::new(DiagnosticKind::ConversionLoss, span, message)
    }
}

impl fmt::Display for Diagnostic {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let kind = match self.kind {
            DiagnosticKind::ConversionLoss => "conversion loss",
            DiagnosticKind::HtmlRemoval => "HTML removal",
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
    fn new(message: impl Into<String>) -> Self {
        Self {
            message: message.into(),
            diagnostics: Vec::new(),
        }
    }

    pub fn diagnostics(&self) -> &[Diagnostic] {
        &self.diagnostics
    }
}
impl fmt::Display for ConversionError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.message.fmt(f)
    }
}
impl std::error::Error for ConversionError {}

// Blocks retain their order independently of the input and output formats.
struct Document {
    blocks: Vec<Block>,
}

enum Block {
    Paragraph(Vec<Inline>),
    Heading { level: u8, text: Vec<Inline> },
    Core(core::Block),
    Table(table::Table),
    Html(String),
}

fn heading_id(text: &[Inline], ids: &mut std::collections::HashSet<String>) -> String {
    let base = inline::normalize_fragment(&inline::plain(text));
    let mut id = base.clone();
    let mut suffix = 2;
    while !ids.insert(id.to_lowercase()) {
        id = format!("{base}_{suffix}");
        suffix += 1;
    }
    id
}

pub fn convert(request: ConversionRequest<'_>) -> Result<ConversionResult, ConversionError> {
    convert_with_mode(request, ConversionMode::BestEffort)
}

pub fn convert_with_mode(
    request: ConversionRequest<'_>,
    mode: ConversionMode,
) -> Result<ConversionResult, ConversionError> {
    if !matches!(
        (request.source, request.target),
        (Format::Wikitext, Format::Gfm | Format::Html)
            | (Format::Gfm | Format::Html, Format::Wikitext)
    ) {
        return Err(ConversionError::new("unsupported conversion direction"));
    }

    let mut diagnostics = Vec::new();
    let (document, page_metadata) = match request.source {
        Format::Wikitext => {
            let metadata = metadata::extract_wikitext(request.input, request.context)?;
            if request.target == Format::Gfm {
                diagnostics.extend(metadata.locations.into_iter().map(|location| {
                    Diagnostic::loss(
                        location.start..location.end,
                        format!("saving the GFM body alone loses the {}", location.kind),
                    )
                }));
            }
            let (without_templates, template_losses) = strip_transclusions(&metadata.body);
            let template_ranges: Vec<_> = template_losses
                .iter()
                .map(|loss| loss.span.clone())
                .collect();
            let sanitized = html_policy::sanitize(&without_templates, true);
            let map_template_span =
                |span| original_range(original_range(span, &template_ranges), &metadata.ranges);
            let document = parse_wikitext(
                &sanitized.text,
                request.context.site_language,
                &|span| map_template_span(sanitized.original_span(span)),
                &mut diagnostics,
            );
            diagnostics.extend(template_losses.into_iter().map(|mut diagnostic| {
                diagnostic.span = original_range(diagnostic.span, &metadata.ranges);
                diagnostic
            }));
            diagnostics.extend(sanitized.diagnostics.into_iter().map(|mut diagnostic| {
                diagnostic.span = map_template_span(diagnostic.span);
                diagnostic
            }));
            (document, metadata.metadata)
        }
        Format::Html => {
            let metadata = metadata::extract_html(request.input, request.context)?;
            let document = parse_html(
                &metadata.body,
                request.input,
                &metadata.ranges,
                &mut diagnostics,
            )?;
            (document, metadata.metadata)
        }
        Format::Gfm => (
            parse_gfm(request.input, &mut diagnostics)?,
            PageMetadata::default(),
        ),
    };

    let separator = if request.target == Format::Html {
        "\n"
    } else {
        "\n\n"
    };
    let mut headings = std::collections::HashSet::new();
    let mut previous_list: Option<(bool, bool)> = None;
    let mut blocks = Vec::new();
    for block in document.blocks {
        let alternate = if let Block::Core(block) = &block {
            core::list_type(block).map(|ordered| {
                let alternate = previous_list.is_some_and(|(last, alt)| last == ordered && !alt);
                (ordered, alternate)
            })
        } else {
            None
        };
        previous_list = alternate;
        let rendered = match &block {
            Block::Paragraph(text) => match request.target {
                Format::Html => format!("<p>{}</p>", render_inline(text, Format::Html)),
                format => render_inline(text, format),
            },
            Block::Heading { level, text } => match request.target {
                Format::Wikitext => {
                    let delimiter = "=".repeat(*level as usize);
                    format!(
                        "{delimiter} {} {delimiter}",
                        render_inline(text, Format::Wikitext)
                    )
                }
                Format::Gfm => format!(
                    "{} {}",
                    "#".repeat(*level as usize),
                    render_inline(text, Format::Gfm)
                ),
                Format::Html => {
                    let id = heading_id(text, &mut headings);
                    format!(
                        "<h{level} id=\"{}\">{}</h{level}>",
                        html_escape::encode_double_quoted_attribute(&id),
                        render_inline(text, Format::Html)
                    )
                }
            },
            Block::Core(block) => {
                core::render(block, request.target, alternate.is_some_and(|(_, alt)| alt))
            }
            Block::Table(table) => table.render(request.target)?,
            Block::Html(html) if request.target == Format::Wikitext => html_to_wiki(html),
            Block::Html(html) => embedded_html(html, request.context.site_language),
        };
        if !rendered.is_empty() {
            blocks.push(rendered);
        }
    }
    let encoded_metadata = match request.target {
        Format::Wikitext => metadata::render_wikitext(&page_metadata),
        Format::Html => metadata::render_html(&page_metadata),
        Format::Gfm => String::new(),
    };
    if !encoded_metadata.is_empty() {
        if page_metadata.redirect_target.is_some() && request.target == Format::Wikitext {
            blocks.insert(0, encoded_metadata);
        } else {
            blocks.push(encoded_metadata);
        }
    }
    diagnostics.sort_by_key(|diagnostic| diagnostic.span.start);
    if mode == ConversionMode::Strict
        && diagnostics
            .iter()
            .any(|diagnostic| diagnostic.kind == DiagnosticKind::ConversionLoss)
    {
        return Err(ConversionError {
            message: "strict conversion would lose document meaning".into(),
            diagnostics,
        });
    }
    Ok(ConversionResult {
        content: blocks.join(separator),
        metadata: page_metadata,
        diagnostics,
    })
}

fn original_range(span: Range<usize>, removed: &[Range<usize>]) -> Range<usize> {
    let mut start = span.start;
    let mut end = span.end;
    for removal in removed {
        if removal.start <= start {
            start += removal.len();
        }
        if removal.start < end {
            end += removal.len();
        }
    }
    start..end.max(start)
}

fn strip_transclusions(input: &str) -> (String, Vec<Diagnostic>) {
    let (mut pos, mut output, mut diagnostics) = (0, String::new(), Vec::new());
    while pos < input.len() {
        let tail = &input[pos..];
        let literal = if let Some(comment) = tail.strip_prefix("<!--") {
            Some(comment.find("-->").map_or(tail.len(), |end| end + 7))
        } else if tail
            .get(..8)
            .is_some_and(|s| s.eq_ignore_ascii_case("<nowiki>"))
        {
            Some(
                tail.to_ascii_lowercase()
                    .find("</nowiki>")
                    .map_or(tail.len(), |end| end + 9),
            )
        } else if let Some((name, open_end, false)) = html_policy::tag_info(tail) {
            if name == "pre" {
                html_policy::raw_close(&tail[open_end..], &name).map(|(_, end)| open_end + end)
            } else {
                None
            }
        } else {
            None
        };
        if let Some(length) = literal {
            output.push_str(&tail[..length]);
            pos += length;
            continue;
        }
        if tail.starts_with("{{") {
            let length = transclusion_end(tail);
            if let Some(length) = length {
                diagnostics.push(Diagnostic::loss(
                    pos..pos + length,
                    "unsupported wikitext transclusion omitted",
                ));
                pos += length;
                continue;
            }
            output.push_str(tail);
            break;
        }
        let character = tail.chars().next().unwrap();
        output.push(character);
        pos += character.len_utf8();
    }
    (output, diagnostics)
}

fn transclusion_end(input: &str) -> Option<usize> {
    let mut levels = Vec::new();
    let mut pos = 0;
    while let Some(next) = input[pos..].find(['{', '}']) {
        pos += next;
        let tail = &input[pos..];
        if tail.starts_with("{{") {
            let width = if tail.starts_with("{{{") && !tail.starts_with("{{{{") {
                3
            } else {
                2
            };
            levels.push(width);
            pos += width;
        } else if let Some(&width) = levels.last() {
            if tail
                .as_bytes()
                .get(..width)
                .is_some_and(|part| part.iter().all(|&b| b == b'}'))
            {
                levels.pop();
                pos += width;
                if levels.is_empty() {
                    return Some(pos);
                }
            } else {
                pos += 1;
            }
        }
    }
    None
}

fn parse_wikitext(
    input: &str,
    language: &str,
    source_span: &impl Fn(Range<usize>) -> Range<usize>,
    diagnostics: &mut Vec<Diagnostic>,
) -> Document {
    let removed_crs: Vec<_> = input
        .match_indices("\r\n")
        .map(|(pos, _)| pos..pos + 1)
        .collect();
    let normalized = input.replace("\r\n", "\n").replace('\r', "\n");
    let mut lines: Vec<_> = normalized.split('\n').collect();
    let location = |part: &str, length| {
        let start = part.as_ptr() as usize - normalized.as_ptr() as usize;
        source_span(original_range(start..start + length, &removed_crs))
    };
    let mut blocks = Vec::new();
    let mut pos = 0;
    while pos < lines.len() {
        let line = lines[pos];
        let remainder = &normalized[line.as_ptr() as usize - normalized.as_ptr() as usize..];
        if let Some((name, end)) = html_policy::wiki_fragment(remainder) {
            if html_policy::is_block(&name) {
                let html = &remainder[..end];
                blocks.push(Block::Html(html.to_owned()));
                pos += html.bytes().filter(|&b| b == b'\n').count();
                let rest = remainder[end..]
                    .split('\n')
                    .next()
                    .unwrap_or("")
                    .trim_start_matches([' ', '\t']);
                if rest.is_empty() {
                    pos += 1;
                } else {
                    lines[pos] = rest;
                }
                continue;
            }
        }
        if line.starts_with("{|") {
            let mut depth = 0;
            let length = lines[pos..]
                .iter()
                .position(|line| {
                    let line = line.trim_start();
                    if line.starts_with("{|") {
                        depth += 1;
                    } else if line.starts_with("|}") {
                        depth -= 1;
                        return depth == 0;
                    }
                    false
                })
                .map_or(lines.len() - pos, |end| end + 1);
            let table_source = lines[pos..pos + length].join("\n");
            let span = location(line, table_source.len());
            match table::Table::parse_wiki(&table_source, language) {
                Ok(table) => {
                    diagnostics.extend(table.wiki_import_losses().iter().map(|diagnostic| {
                        Diagnostic::new(diagnostic.kind, span.clone(), &diagnostic.message)
                    }));
                    blocks.push(Block::Table(table));
                }
                Err(error) => {
                    diagnostics.push(Diagnostic::loss(span, format!("table omitted: {error}")))
                }
            }
            pos += length;
            continue;
        }
        match core::parse_wiki(&lines[pos..], language) {
            Ok(Some((block, length))) => {
                blocks.push(Block::Core(block));
                pos += length;
                continue;
            }
            Ok(None) => {}
            Err(error) => {
                diagnostics.push(Diagnostic::loss(
                    location(line, line.len()),
                    error.to_string(),
                ));
                blocks.push(Block::Paragraph(vec![Inline::Text(line.to_owned())]));
                pos += 1;
                continue;
            }
        }
        if line.trim_ascii().is_empty() {
            pos += 1;
            continue;
        }
        if let Some((level, text)) = heading(line) {
            blocks.push(Block::Heading {
                level,
                text: parse_wiki_inline(text, language, location(text, text.len()), diagnostics),
            });
            pos += 1;
            continue;
        }
        let end = lines[pos + 1..]
            .iter()
            .position(|line| {
                line.trim_ascii().is_empty()
                    || heading(line).is_some()
                    || line.starts_with(['*', '#', ':', ';', ' '])
                    || line.starts_with("----")
                    || line.starts_with("{|")
                    || html_policy::wiki_fragment(line)
                        .is_some_and(|(name, _)| html_policy::is_block(&name))
            })
            .map(|offset| pos + 1 + offset)
            .unwrap_or(lines.len());
        let paragraph = lines[pos..end].join("\n");
        blocks.push(Block::Paragraph(parse_wiki_inline(
            &paragraph,
            language,
            location(line, paragraph.len()),
            diagnostics,
        )));
        pos = end;
    }
    Document { blocks }
}

fn parse_wiki_inline(
    input: &str,
    language: &str,
    location: Range<usize>,
    diagnostics: &mut Vec<Diagnostic>,
) -> Vec<Inline> {
    match inline::parse_wiki(input, language) {
        Ok(inlines) => inlines,
        Err(error) => {
            diagnostics.push(Diagnostic::loss(location, error.to_string()));
            vec![Inline::Text(input.to_owned())]
        }
    }
}

fn locate(input: &str, fragment: &str) -> Range<usize> {
    input
        .find(fragment)
        .map_or(0..input.len(), |start| start..start + fragment.len())
}

fn heading(line: &str) -> Option<(u8, &str)> {
    let line = line.trim_end();
    let start = line.bytes().take_while(|&c| c == b'=').count();
    let end = line.bytes().rev().take_while(|&c| c == b'=').count();
    let level = if start == line.len() {
        start.saturating_sub(1) / 2
    } else {
        start.min(end)
    }
    .min(6);
    (level > 0).then(|| (level as u8, line[level..line.len() - level].trim()))
}

fn render_inline(content: &[Inline], format: Format) -> String {
    let mut output = String::new();
    let mut previous_wikilink = false;
    for part in content {
        match part {
            Inline::Html(html) => match format {
                Format::Wikitext => output.push_str(&html_to_wiki(html)),
                _ => output.push_str(html),
            },
            Inline::Emphasis { strong, children } => {
                let (open, close) = match (format, strong) {
                    (Format::Html, true) => ("<b>", "</b>"),
                    (Format::Html, false) => ("<i>", "</i>"),
                    (Format::Wikitext, true) => ("'''", "'''"),
                    (Format::Wikitext, false) => ("''", "''"),
                    (Format::Gfm, true) => ("**", "**"),
                    (Format::Gfm, false) => ("*", "*"),
                };
                output.push_str(open);
                output.push_str(&render_inline(children, format));
                output.push_str(close);
            }
            Inline::Text(text) => match format {
                Format::Wikitext => {
                    let mut remainder = text.as_str();
                    if previous_wikilink {
                        if let Some(first) = remainder.chars().next().filter(|c| c.is_alphabetic())
                        {
                            output.push_str(&format!("&#{};", first as u32));
                            remainder = &remainder[first.len_utf8()..];
                        }
                    }
                    output.push_str(&escape_wikitext_at(
                        remainder,
                        output.is_empty() || output.ends_with('\n'),
                    ));
                }
                Format::Gfm => output.push_str(&escape_gfm(text)),
                Format::Html => output.push_str(&html_escape::encode_text(text)),
            },
            Inline::Link {
                href,
                label,
                internal,
            } => match format {
                Format::Wikitext if *internal => {
                    let target = inline::wiki_target(href).expect("validated wiki href");
                    output.push_str(&format!(
                        "[[{}|{}]]",
                        target.replace('&', "&amp;"),
                        render_inline(label, format)
                    ));
                }
                Format::Wikitext => output.push_str(&format!(
                    "[{} {}]",
                    html_policy::escape_wiki_attribute_value(href),
                    render_inline(label, format)
                )),
                Format::Gfm => output.push_str(&format!(
                    "[{}](<{}>)",
                    render_inline(label, format),
                    href.replace('&', "&amp;")
                )),
                Format::Html => output.push_str(&format!(
                    "<a rel=\"{}\" href=\"{}\">{}</a>",
                    if *internal {
                        "mw:WikiLink"
                    } else {
                        "mw:ExtLink"
                    },
                    html_escape::encode_double_quoted_attribute(href),
                    render_inline(label, format)
                )),
            },
        }
        match part {
            Inline::Link { internal, .. } => previous_wikilink = *internal,
            Inline::Text(text) if !text.is_empty() => previous_wikilink = false,
            _ => {}
        }
    }
    output
}

fn escape_wikitext(text: &str) -> String {
    escape_wikitext_at(text, true)
}

fn escape_wikitext_at(text: &str, mut line_start: bool) -> String {
    let mut output = String::new();
    for ch in text.chars() {
        match ch {
            ' ' | '\t' if line_start => output.push_str(&format!("&#{};", ch as u32)),
            '&' => output.push_str("&amp;"),
            '<' => output.push_str("&lt;"),
            '>' => output.push_str("&gt;"),
            '[' | ']' | '{' | '}' | '\'' | '|' | '=' | '*' | '#' | ':' | ';' | '-' | '_' | '~' => {
                output.push_str(&format!("&#{};", ch as u32))
            }
            _ => output.push(ch),
        }
        line_start = ch == '\n';
    }
    output
}

fn escape_gfm(text: &str) -> String {
    let mut output = String::new();
    for ch in text.chars() {
        if ch.is_ascii_punctuation() {
            output.push('\\');
        }
        output.push(ch);
    }
    output
}
