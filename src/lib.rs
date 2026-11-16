mod inline;

use inline::Inline;
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

// Blocks retain their order independently of the input and output formats.
struct Document {
    blocks: Vec<Block>,
}

enum Block {
    Paragraph(Vec<Inline>),
    Heading { level: u8, text: Vec<Inline> },
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
        Format::Wikitext => parse_wikitext(request.input, request.context.site_language)?,
        Format::Gfm => parse_gfm(request.input)?,
        Format::Html => parse_html(request.input)?,
    };
    let mut ids = std::collections::HashSet::<String>::new();
    let content = document
        .blocks
        .iter()
        .map(|block| match (request.target, block) {
            (Format::Wikitext, Block::Paragraph(text)) => render_inline(text, Format::Wikitext),
            (Format::Wikitext, Block::Heading { level, text }) => {
                let delimiter = "=".repeat(*level as usize);
                format!(
                    "{delimiter} {} {delimiter}",
                    render_inline(text, Format::Wikitext)
                )
            }
            (Format::Gfm, Block::Paragraph(text)) => render_inline(text, Format::Gfm),
            (Format::Gfm, Block::Heading { level, text }) => {
                format!(
                    "{} {}",
                    "#".repeat(*level as usize),
                    render_inline(text, Format::Gfm)
                )
            }
            (Format::Html, Block::Paragraph(text)) => {
                format!("<p>{}</p>", render_inline(text, Format::Html))
            }
            (Format::Html, Block::Heading { level, text }) => {
                let plain = inline::plain(text);
                let base = plain.split_whitespace().collect::<Vec<_>>().join("_");
                let mut id = base.clone();
                let mut suffix = 2;
                while !ids.insert(id.to_lowercase()) {
                    id = format!("{base}_{suffix}");
                    suffix += 1;
                }
                format!(
                    "<h{level} id=\"{}\">{}</h{level}>",
                    html_escape::encode_double_quoted_attribute(&id),
                    render_inline(text, Format::Html)
                )
            }
        })
        .collect::<Vec<_>>()
        .join(if request.target == Format::Html {
            "\n"
        } else {
            "\n\n"
        });
    Ok(ConversionResult {
        content,
        metadata: PageMetadata::default(),
        diagnostics: Vec::new(),
    })
}

fn flush_paragraph(
    blocks: &mut Vec<Block>,
    lines: &mut Vec<&str>,
    language: &str,
) -> Result<(), ConversionError> {
    if !lines.is_empty() {
        blocks.push(Block::Paragraph(inline::parse_wiki(
            &lines.join("\n"),
            language,
        )?));
        lines.clear();
    }
    Ok(())
}

fn parse_wikitext(input: &str, language: &str) -> Result<Document, ConversionError> {
    let normalized = input.replace("\r\n", "\n").replace('\r', "\n");
    let mut blocks = Vec::new();
    let mut lines = Vec::new();
    let mut blank_lines = 0;
    for line in normalized.split('\n') {
        if line.trim().is_empty() {
            blank_lines += 1;
            flush_paragraph(&mut blocks, &mut lines, language)?;
            continue;
        }
        if !blocks.is_empty() && blank_lines > 1 {
            return Err(ConversionError::new(
                "multiple blank lines between blocks are unsupported",
            ));
        }
        blank_lines = 0;
        let heading_line = line.trim_end();
        let start = heading_line.bytes().take_while(|&c| c == b'=').count();
        let end = heading_line
            .bytes()
            .rev()
            .take_while(|&c| c == b'=')
            .count();
        let level = if start == heading_line.len() {
            start.saturating_sub(1).checked_div(2).unwrap_or(0).min(6)
        } else {
            start.min(end).min(6)
        };
        if level > 0 {
            flush_paragraph(&mut blocks, &mut lines, language)?;
            let text = &heading_line[level..heading_line.len() - level];
            blocks.push(Block::Heading {
                level: level as u8,
                text: inline::parse_wiki(text.trim(), language)?,
            });
            continue;
        }
        if line.starts_with([' ', '\t', '*', '#', ';', ':']) || line.starts_with("----") {
            return Err(ConversionError::new("unsupported wikitext syntax"));
        }
        lines.push(line);
    }
    flush_paragraph(&mut blocks, &mut lines, language)?;
    Ok(Document { blocks })
}

fn block_inlines(block: &mut Block) -> &mut Vec<Inline> {
    match block {
        Block::Paragraph(text) | Block::Heading { text, .. } => text,
    }
}

fn parse_gfm(input: &str) -> Result<Document, ConversionError> {
    let mut blocks = Vec::new();
    let mut current: Option<Block> = None;
    let mut link: Option<(String, String)> = None;
    let options = Options::ENABLE_GFM
        | Options::ENABLE_TABLES
        | Options::ENABLE_STRIKETHROUGH
        | Options::ENABLE_TASKLISTS;
    for event in Parser::new_ext(input, options) {
        match event {
            Event::Start(Tag::Paragraph) if current.is_none() => {
                current = Some(Block::Paragraph(Vec::new()))
            }
            Event::Start(Tag::Heading { level, .. }) if current.is_none() => {
                current = Some(Block::Heading {
                    level: level as u8,
                    text: Vec::new(),
                })
            }
            Event::End(TagEnd::Paragraph)
                if matches!(current, Some(Block::Paragraph(_))) && link.is_none() =>
            {
                blocks.push(current.take().unwrap())
            }
            Event::End(TagEnd::Heading(_))
                if matches!(current, Some(Block::Heading { .. })) && link.is_none() =>
            {
                blocks.push(current.take().unwrap())
            }
            Event::Start(Tag::Link {
                dest_url, title, ..
            }) if current.is_some() && link.is_none() && title.is_empty() => {
                link = Some((dest_url.into_string(), String::new()))
            }
            Event::End(TagEnd::Link) if link.is_some() => {
                let (href, label) = link.take().unwrap();
                block_inlines(current.as_mut().unwrap()).push(inline::from_href(&href, label)?);
            }
            Event::Text(value) => {
                if let Some((_, label)) = &mut link {
                    label.push_str(&value);
                } else if let Some(block) = &mut current {
                    inline::text(block_inlines(block), &value);
                } else {
                    return Err(ConversionError::new("unsupported GFM text"));
                }
            }
            Event::SoftBreak if link.is_none() && current.is_some() => {
                inline::text(block_inlines(current.as_mut().unwrap()), "\n")
            }
            _ => return Err(ConversionError::new("unsupported GFM syntax")),
        }
    }
    Ok(Document { blocks })
}

fn parse_html(input: &str) -> Result<Document, ConversionError> {
    fn normalized(text: &str) -> String {
        let mut output = String::new();
        let mut space = false;
        for c in text.chars() {
            if matches!(c, ' ' | '\t' | '\n' | '\r' | '\u{000C}') {
                if !space {
                    output.push(' ');
                }
                space = true;
            } else {
                output.push(c);
                space = false;
            }
        }
        output
    }
    fn children(
        parent: scraper::ElementRef<'_>,
        blocks: &mut Vec<Block>,
    ) -> Result<(), ConversionError> {
        for child in parent.children() {
            match child.value() {
                Node::Text(text) if text.trim().is_empty() => {}
                Node::Comment(_) => {}
                Node::Element(element) if element.name() == "section" => {
                    children(scraper::ElementRef::wrap(child).unwrap(), blocks)?
                }
                Node::Element(element)
                    if element.name() == "p"
                        || matches!(element.name(), "h1" | "h2" | "h3" | "h4" | "h5" | "h6") =>
                {
                    let mut content = Vec::new();
                    for node in child.children() {
                        match node.value() {
                            Node::Text(value) => inline::text(&mut content, &normalized(value)),
                            Node::Comment(_) => {}
                            Node::Element(anchor) if anchor.name() == "a" => {
                                let href = anchor
                                    .attr("href")
                                    .ok_or_else(|| ConversionError::new("link has no href"))?;
                                let mut label = String::new();
                                for item in node.children() {
                                    match item.value() {
                                        Node::Text(value) => label.push_str(value),
                                        Node::Comment(_) => {}
                                        _ => {
                                            return Err(ConversionError::new(
                                                "unsupported link label",
                                            ))
                                        }
                                    }
                                }
                                content.push(inline::from_href(href, normalized(&label))?);
                            }
                            _ => {
                                return Err(ConversionError::new(
                                    "unsupported HTML content inside block",
                                ))
                            }
                        }
                    }
                    if let Some(Inline::Text(first)) = content.first_mut() {
                        *first = first.trim_start_matches(' ').to_string();
                    }
                    if let Some(Inline::Text(last)) = content.last_mut() {
                        *last = last.trim_end_matches(' ').to_string();
                    }
                    if element.name() == "p" {
                        if inline::plain(&content).is_empty() {
                            return Err(ConversionError::new(
                                "empty HTML paragraphs are unsupported",
                            ));
                        }
                        blocks.push(Block::Paragraph(content));
                    } else {
                        blocks.push(Block::Heading {
                            level: element.name().as_bytes()[1] - b'0',
                            text: content,
                        });
                    }
                }
                _ => return Err(ConversionError::new("unsupported HTML content")),
            }
        }
        Ok(())
    }
    let fragment = Html::parse_fragment(input);
    let mut blocks = Vec::new();
    children(fragment.root_element(), &mut blocks)?;
    Ok(Document { blocks })
}

fn render_inline(content: &[Inline], format: Format) -> String {
    let mut output = String::new();
    for part in content {
        match part {
            Inline::Text(text) => match format {
                Format::Wikitext => output.push_str(&escape_wikitext(text)),
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
                        escape_wikitext(label)
                    ));
                }
                Format::Wikitext => output.push_str(&format!(
                    "[{} {}]",
                    href.replace('&', "&amp;"),
                    escape_wikitext(label)
                )),
                Format::Gfm => output.push_str(&format!(
                    "[{}](<{}>)",
                    escape_gfm(label),
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
                    html_escape::encode_text(label)
                )),
            },
        }
    }
    output
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
