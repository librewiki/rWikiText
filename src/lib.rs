use std::fmt;
use std::str::FromStr;

use pulldown_cmark::{Event, Options, Parser, Tag, TagEnd};
use scraper::{Html, Node};

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

pub struct ConversionRequest<'a> {
    pub source: Format,
    pub target: Format,
    pub input: &'a str,
    pub context: &'a WikiContext<'a>,
}

#[derive(Debug, Default, Clone, PartialEq, Eq)]
pub struct PageMetadata {
    pub categories: Vec<String>,
    pub redirect_target: Option<String>,
    pub display_title: Option<String>,
}
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Diagnostic {
    pub message: String,
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
}
impl ConversionError {
    fn new(message: impl Into<String>) -> Self {
        Self {
            message: message.into(),
        }
    }
}
impl fmt::Display for ConversionError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.message.fmt(f)
    }
}
impl std::error::Error for ConversionError {}

// A block representation keeps parsing and serialization independent of each other.
struct Document {
    paragraphs: Vec<String>,
}

pub fn convert(request: ConversionRequest<'_>) -> Result<ConversionResult, ConversionError> {
    if !matches!(
        (request.source, request.target),
        (Format::Wikitext, Format::Gfm | Format::Html)
            | (Format::Gfm | Format::Html, Format::Wikitext)
    ) {
        return Err(ConversionError::new("unsupported conversion direction"));
    }
    let document = match request.source {
        Format::Wikitext => parse_wikitext(request.input)?,
        Format::Gfm => parse_gfm(request.input)?,
        Format::Html => parse_html(request.input)?,
    };
    let content = match request.target {
        Format::Wikitext => document
            .paragraphs
            .iter()
            .map(|s| escape_wikitext(s))
            .collect::<Vec<_>>()
            .join("\n\n"),
        Format::Gfm => document
            .paragraphs
            .iter()
            .map(|s| escape_gfm(s))
            .collect::<Vec<_>>()
            .join("\n\n"),
        Format::Html => document
            .paragraphs
            .iter()
            .map(|s| format!("<p>{}</p>", html_escape::encode_text(s)))
            .collect::<Vec<_>>()
            .join("\n"),
    };
    Ok(ConversionResult {
        content,
        metadata: PageMetadata::default(),
        diagnostics: Vec::new(),
    })
}

fn parse_wikitext(input: &str) -> Result<Document, ConversionError> {
    let normalized = input.replace("\r\n", "\n").replace('\r', "\n");
    let mut paragraphs = Vec::new();
    let mut lines = Vec::new();
    let mut blank_lines = 0;
    for line in normalized.split('\n') {
        if line.trim().is_empty() {
            blank_lines += 1;
            if !lines.is_empty() {
                paragraphs.push(html_escape::decode_html_entities(&lines.join("\n")).into_owned());
                lines.clear();
            }
        } else {
            if !paragraphs.is_empty() && blank_lines > 1 {
                return Err(ConversionError::new(
                    "multiple blank lines between paragraphs are unsupported",
                ));
            }
            blank_lines = 0;
            // These forms are not plain paragraphs; reject rather than silently changing meaning.
            if line.starts_with([' ', '\t', '*', '#', ';', ':', '='])
                || line.starts_with("----")
                || line.contains("[[")
                || line.contains("{{")
                || line.contains("''")
                || line.contains('<')
            {
                return Err(ConversionError::new("unsupported wikitext syntax"));
            }
            lines.push(line);
        }
    }
    if !lines.is_empty() {
        paragraphs.push(html_escape::decode_html_entities(&lines.join("\n")).into_owned());
    }
    Ok(Document { paragraphs })
}

fn parse_gfm(input: &str) -> Result<Document, ConversionError> {
    let mut paragraphs = Vec::new();
    let mut current: Option<String> = None;
    let options = Options::ENABLE_GFM
        | Options::ENABLE_TABLES
        | Options::ENABLE_STRIKETHROUGH
        | Options::ENABLE_TASKLISTS;
    for event in Parser::new_ext(input, options) {
        match event {
            Event::Start(Tag::Paragraph) if current.is_none() => current = Some(String::new()),
            Event::End(TagEnd::Paragraph) => paragraphs.push(
                current
                    .take()
                    .ok_or_else(|| ConversionError::new("invalid paragraph"))?,
            ),
            Event::Text(text) if current.is_some() => current.as_mut().unwrap().push_str(&text),
            Event::SoftBreak if current.is_some() => current.as_mut().unwrap().push('\n'),
            _ => return Err(ConversionError::new("unsupported GFM syntax")),
        }
    }
    Ok(Document { paragraphs })
}

fn parse_html(input: &str) -> Result<Document, ConversionError> {
    let fragment = Html::parse_fragment(input);
    let root = fragment.root_element();
    let mut paragraphs = Vec::new();
    for child in root.children() {
        match child.value() {
            Node::Text(text) if text.trim().is_empty() => {}
            Node::Element(element) if element.name() == "p" => {
                let mut paragraph = String::new();
                for node in child.children() {
                    match node.value() {
                        Node::Text(text) => paragraph.push_str(text),
                        Node::Comment(_) => {}
                        _ => {
                            return Err(ConversionError::new(
                                "unsupported HTML content inside paragraph",
                            ))
                        }
                    }
                }
                // HTML collapses whitespace in normal paragraph text.
                let normalized = paragraph
                    .split([' ', '\t', '\n', '\r', '\u{000C}'])
                    .filter(|part| !part.is_empty())
                    .collect::<Vec<_>>()
                    .join(" ");
                if normalized.is_empty() {
                    return Err(ConversionError::new(
                        "empty HTML paragraphs are unsupported",
                    ));
                }
                paragraphs.push(normalized);
            }
            Node::Comment(_) => {}
            _ => return Err(ConversionError::new("unsupported HTML content")),
        }
    }
    Ok(Document { paragraphs })
}

fn escape_wikitext(text: &str) -> String {
    let mut output = String::new();
    let mut line_start = true;
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
