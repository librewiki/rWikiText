use std::ops::Range;

use crate::{Diagnostic, DiagnosticKind};

const WIKI_TAGS: &[&str] = &[
    "abbr",
    "b",
    "bdi",
    "bdo",
    "big",
    "blockquote",
    "br",
    "caption",
    "center",
    "cite",
    "code",
    "data",
    "dd",
    "del",
    "dfn",
    "div",
    "dl",
    "dt",
    "em",
    "font",
    "h1",
    "h2",
    "h3",
    "h4",
    "h5",
    "h6",
    "hr",
    "i",
    "ins",
    "kbd",
    "li",
    "link",
    "mark",
    "meta",
    "ol",
    "p",
    "pre",
    "q",
    "rb",
    "rp",
    "rt",
    "rtc",
    "ruby",
    "s",
    "samp",
    "small",
    "span",
    "strike",
    "strong",
    "sub",
    "sup",
    "table",
    "td",
    "th",
    "time",
    "tr",
    "tt",
    "u",
    "ul",
    "var",
    "wbr",
];
const IMPORT_TAGS: &[&str] = &["a", "tbody", "thead", "tfoot", "section"];
const VOID: &[&str] = &["br", "hr", "link", "meta", "wbr"];
const RAW: &[&str] = &[
    "script",
    "style",
    "iframe",
    "textarea",
    "title",
    "xmp",
    "noembed",
    "noframes",
    "plaintext",
];

pub(crate) fn is_inert_html(name: &str) -> bool {
    RAW.contains(&name) || name == "template"
}
const GLOBAL: &[&str] = &[
    "id",
    "class",
    "style",
    "lang",
    "dir",
    "title",
    "tabindex",
    "aria-describedby",
    "aria-flowto",
    "aria-hidden",
    "aria-label",
    "aria-labelledby",
    "aria-level",
    "aria-owns",
    "role",
    "about",
    "property",
    "resource",
    "datatype",
    "typeof",
    "itemid",
    "itemprop",
    "itemref",
    "itemscope",
    "itemtype",
];

pub(crate) const PROTOCOLS: &[&str] = &[
    "bitcoin:",
    "ftp://",
    "ftps://",
    "geo:",
    "git://",
    "gopher://",
    "http://",
    "https://",
    "irc://",
    "ircs://",
    "magnet:",
    "mailto:",
    "matrix:",
    "mms://",
    "news:",
    "nntp://",
    "redis://",
    "sftp://",
    "sip:",
    "sips:",
    "sms:",
    "ssh://",
    "svn://",
    "tel:",
    "telnet://",
    "urn:",
    "wikipedia://",
    "worldwind://",
    "xmpp:",
    "//",
];

pub(crate) fn external_url(url: &str) -> bool {
    let lower = url.to_ascii_lowercase();
    PROTOCOLS.iter().any(|prefix| {
        lower.strip_prefix(prefix).is_some_and(|tail| {
            !tail.is_empty() && (!prefix.ends_with("//") || !tail.starts_with(['/', '?', '#']))
        })
    }) && !url
        .chars()
        .any(|c| c.is_whitespace() || c.is_control() || matches!(c, '<' | '>' | '"' | '\\'))
}

fn allowed_attr(tag: &str, name: &str) -> bool {
    match tag {
        "meta" => return matches!(name, "itemprop" | "content"),
        "link" => return matches!(name, "itemprop" | "href" | "title"),
        _ => {}
    }
    if GLOBAL.contains(&name)
        || name.strip_prefix("data-").is_some_and(|tail| {
            !tail.starts_with("mw")
                && !tail.starts_with("parsoid")
                && !tail.starts_with("ooui")
                && !tail.contains(':')
        })
        || name.strip_prefix("xmlns:").is_some_and(|tail| {
            !tail.is_empty()
                && tail
                    .bytes()
                    .all(|b| b.is_ascii_alphanumeric() || b":_.-".contains(&b))
        })
    {
        return true;
    }
    let specific: &[&str] = match tag {
        "a" => &["href", "rel", "rev"],
        "blockquote" | "q" => &["cite"],
        "del" | "ins" => &["cite", "datetime"],
        "div" | "p" | "h1" | "h2" | "h3" | "h4" | "h5" | "h6" | "caption" => &["align"],
        "br" => &["clear"],
        "data" => &["value"],
        "time" => &["datetime"],
        "font" => &["size", "color", "face"],
        "hr" | "pre" => &["width"],
        "ol" => &["type", "start", "reversed"],
        "ul" => &["type"],
        "li" => &["type", "value"],
        "table" => &[
            "summary",
            "width",
            "border",
            "frame",
            "rules",
            "cellspacing",
            "cellpadding",
            "align",
            "bgcolor",
        ],
        "tr" | "tbody" | "thead" | "tfoot" => &["align", "valign", "bgcolor"],
        "td" | "th" => &[
            "abbr", "axis", "headers", "scope", "rowspan", "colspan", "nowrap", "width", "height",
            "bgcolor", "align", "valign",
        ],
        _ => &[],
    };
    specific.contains(&name)
}

fn safe_attr(tag: &str, name: &str, value: &str) -> bool {
    let compact: String = value
        .chars()
        .filter(|c| !c.is_whitespace() && !c.is_control())
        .flat_map(char::to_lowercase)
        .collect();
    match name {
        "href" | "cite" => {
            (external_url(value)
                || (tag != "link"
                    && value.starts_with(['/', '#', '.'])
                    && !value.starts_with("//")
                    && !value
                        .chars()
                        .any(|c| c.is_whitespace() || c.is_control() || c == '\\')))
                && (tag != "a" || name != "href" || crate::inline::link_target(value).is_ok())
        }
        "style" => {
            // Reject CSS escape sequences so a browser cannot retokenize
            // a declaration as one of MediaWiki's prohibited CSS functions.
            !compact.contains(['\\', '@', '&', '<', '>'])
                && ![
                    "expression",
                    "accelerator:",
                    "-o-link",
                    "-o-replace:",
                    "url(",
                    "src(",
                    "image(",
                    "image-set(",
                    "attr(",
                    "behavior:",
                    "-moz-binding:",
                ]
                .iter()
                .any(|pattern| compact.contains(pattern))
        }
        "tabindex" => value == "0",
        "id" => !value.trim().is_empty(),
        "about" | "resource" | "property" | "datatype" | "typeof" | "rel" | "rev" | "itemid"
        | "itemprop" | "itemref" | "itemtype" | "itemscope" => {
            !compact.contains("javascript:") && !compact.contains("vbscript:")
        }
        _ if name.starts_with("xmlns:") => {
            !compact.contains("javascript:") && !compact.contains("vbscript:")
        }
        _ => true,
    }
}

fn css_without_comments(mut value: &str) -> String {
    let mut css = String::new();
    while let Some(start) = value.find("/*") {
        css.push_str(&value[..start]);
        css.push(' ');
        let comment = &value[start + 2..];
        value = comment.find("*/").map_or("", |end| &comment[end + 2..]);
    }
    css.push_str(value);
    css
}

pub(crate) fn escape_wiki_attribute_markup(markup: &str) -> String {
    let mut escaped = String::new();
    for character in markup.chars() {
        if matches!(character, '{' | '}' | '[' | ']' | '|' | '\'' | '\n' | '\r') {
            escaped.push_str(&format!("&#{};", character as u32));
        } else {
            escaped.push(character);
        }
    }
    escaped
}

pub(crate) fn escape_wiki_attribute_value(value: &str) -> String {
    escape_wiki_attribute_markup(&html_escape::encode_double_quoted_attribute(value))
}

struct Attribute<'a> {
    name: String,
    value: &'a str,
    span: Range<usize>,
}

struct Tag<'a> {
    name: String,
    end: usize,
    closing: bool,
    self_closing: bool,
    attrs: Vec<Attribute<'a>>,
}

fn tag(input: &str) -> Option<Tag<'_>> {
    let input_bytes = input.as_bytes();
    if input_bytes.first() != Some(&b'<') {
        return None;
    }
    let closing = input_bytes.get(1) == Some(&b'/');
    let start = 1 + usize::from(closing);
    if !input_bytes.get(start)?.is_ascii_alphabetic() {
        return None;
    }
    let mut i = start;
    while input_bytes
        .get(i)
        .is_some_and(|b| !b.is_ascii_whitespace() && !b"/>".contains(b))
    {
        i += 1;
    }
    let name = input[start..i].to_ascii_lowercase();
    let mut attrs = Vec::new();
    loop {
        i += input_bytes[i..]
            .iter()
            .take_while(|b| b.is_ascii_whitespace())
            .count();
        let self_closing = input_bytes.get(i..i + 2) == Some(b"/>");
        if self_closing || input_bytes.get(i) == Some(&b'>') {
            return Some(Tag {
                name,
                end: i + 1 + usize::from(self_closing),
                closing,
                self_closing,
                attrs,
            });
        }
        let attr_start = i;
        i += 1;
        input_bytes.get(attr_start)?;
        while input_bytes
            .get(i)
            .is_some_and(|b| !b.is_ascii_whitespace() && !b"=/><".contains(b))
        {
            i += 1;
        }
        let attr_name = input[attr_start..i].to_ascii_lowercase();
        i += input_bytes[i..]
            .iter()
            .take_while(|b| b.is_ascii_whitespace())
            .count();
        let mut value = "";
        if input_bytes.get(i) == Some(&b'=') {
            i += 1;
            i += input_bytes[i..]
                .iter()
                .take_while(|b| b.is_ascii_whitespace())
                .count();
            if let Some(quote @ (b'\'' | b'"')) = input_bytes.get(i).copied() {
                i += 1;
                let end = i + input_bytes[i..].iter().position(|&b| b == quote)?;
                value = &input[i..end];
                i = end + 1;
            } else {
                let start = i;
                while input_bytes
                    .get(i)
                    .is_some_and(|b| !b.is_ascii_whitespace() && *b != b'>')
                {
                    i += 1;
                }
                value = &input[start..i];
            }
        }
        attrs.push(Attribute {
            name: attr_name,
            value,
            span: attr_start..i,
        });
    }
}

pub(crate) fn is_block(name: &str) -> bool {
    matches!(
        name,
        "div"
            | "center"
            | "blockquote"
            | "dl"
            | "h1"
            | "h2"
            | "h3"
            | "h4"
            | "h5"
            | "h6"
            | "hr"
            | "ol"
            | "p"
            | "pre"
            | "table"
            | "ul"
            | "section"
    )
}

pub(crate) fn tag_info(input: &str) -> Option<(String, usize, bool)> {
    tag(input).map(|tag| (tag.name, tag.end, tag.closing))
}

pub(crate) fn sanitize_tag_attributes(name: &str, attrs: &str) -> (String, Vec<Diagnostic>) {
    let open = format!("<{name}");
    let source = format!("{open} {}>", attrs.trim());
    if !tag(&source).is_some_and(|tag| tag.end == source.len() && !tag.self_closing) {
        return (
            String::new(),
            vec![Diagnostic::new(
                DiagnosticKind::HtmlRemoval,
                0..attrs.len(),
                "malformed table attributes removed",
            )],
        );
    }
    let sanitized = sanitize(&source, true);
    let attributes = sanitized
        .text
        .strip_prefix(&open)
        .and_then(|s| s.strip_suffix('>'));
    let Some(attributes) = attributes else {
        return (String::new(), sanitized.diagnostics);
    };
    (attributes.to_owned(), sanitized.diagnostics)
}

// The wiki parser consumes HTML after the policy pass. Unclosed literal tags
// remain available to the HTML5 parser for normal error recovery.
pub(crate) fn wiki_fragment(input: &str) -> Option<(String, usize)> {
    let open = tag(input)?;
    if !WIKI_TAGS.contains(&open.name.as_str()) {
        return None;
    }
    if open.closing || open.self_closing || VOID.contains(&open.name.as_str()) {
        return Some((open.name, open.end));
    }
    let end = matching_close(input, &open).unwrap_or(input.len());
    Some((open.name, end))
}

fn matching_close(input: &str, open: &Tag<'_>) -> Option<usize> {
    let mut depth = 1;
    let mut pos = open.end;
    while let Some(next) = input[pos..].find('<') {
        pos += next;
        if let Some(next) = tag(&input[pos..]) {
            pos += next.end;
            if next.name == open.name {
                if next.closing {
                    depth -= 1;
                } else if !next.self_closing {
                    depth += 1;
                }
                if depth == 0 {
                    return Some(pos);
                }
            }
        } else {
            pos += 1;
        }
    }
    None
}

pub(crate) struct Sanitized {
    pub text: String,
    pub diagnostics: Vec<Diagnostic>,
    replacements: Vec<(Range<usize>, Range<usize>)>,
}

impl Sanitized {
    pub fn original_span(&self, span: Range<usize>) -> Range<usize> {
        let boundary = |offset: usize, right: bool| {
            let mut original_offset = offset;
            for (input, output) in &self.replacements {
                if offset < output.start || (offset == output.start && !right) {
                    break;
                }
                if offset < output.end {
                    return if right { input.start } else { input.end };
                }
                original_offset = input.end + (offset - output.end);
            }
            original_offset
        };
        let start = boundary(span.start, true);
        start..boundary(span.end, false).max(start)
    }
}

pub(crate) fn sanitize(input: &str, wikitext: bool) -> Sanitized {
    let mut output = String::new();
    let mut diagnostics = Vec::new();
    let mut replacements = Vec::new();
    let mut pos = 0;
    while let Some(next) = input[pos..].find('<') {
        output.push_str(&input[pos..pos + next]);
        pos += next;
        let start = output.len();
        let length = sanitize_token(&input[pos..], pos, wikitext, &mut output, &mut diagnostics);
        if output.len() - start != length {
            replacements.push((pos..pos + length, start..output.len()));
        }
        pos += length;
    }
    output.push_str(&input[pos..]);
    Sanitized {
        text: output,
        diagnostics,
        replacements,
    }
}

pub(crate) fn raw_close(input: &str, name: &str) -> Option<(usize, usize)> {
    let close = format!("</{name}");
    input
        .to_ascii_lowercase()
        .match_indices(&close)
        .find_map(|(start, _)| {
            tag(&input[start..])
                .filter(|candidate| candidate.name == name && candidate.closing)
                .map(|candidate| (start, start + candidate.end))
        })
}

fn sanitize_token(
    rest: &str,
    pos: usize,
    wikitext: bool,
    output: &mut String,
    diagnostics: &mut Vec<Diagnostic>,
) -> usize {
    if let Some(comment) = rest.strip_prefix("<!--") {
        return comment.find("-->").map_or(rest.len(), |n| n + 7);
    }
    if wikitext
        && rest
            .get(..8)
            .is_some_and(|s| s.eq_ignore_ascii_case("<nowiki>"))
    {
        let tail = &rest[8..];
        let end = tail
            .to_ascii_lowercase()
            .find("</nowiki>")
            .unwrap_or(tail.len());
        let literal = &tail[..end];
        for character in literal.chars() {
            output.push_str(&format!("&#{};", character as u32));
        }
        return (8 + end + 9).min(rest.len());
    }
    let Some(token) = tag(rest) else {
        output.push_str(if wikitext { "&lt;" } else { "<" });
        return 1;
    };
    let name = token.name.as_str();
    let allowed = WIKI_TAGS.contains(&name) || (!wikitext && IMPORT_TAGS.contains(&name));
    let microdata = !matches!(name, "link" | "meta")
        || token.closing
        || (token.attrs.iter().any(|a| a.name == "itemprop")
            && token
                .attrs
                .iter()
                .any(|a| a.name == if name == "link" { "href" } else { "content" }));
    if !allowed || !microdata {
        let mut end = token.end;
        if !wikitext && !token.closing && is_inert_html(name) {
            end = if name == "template" {
                matching_close(rest, &token).unwrap_or(rest.len())
            } else if name == "plaintext" {
                rest.len()
            } else {
                raw_close(&rest[token.end..], name)
                    .map_or(rest.len(), |(_, close)| token.end + close)
            };
        }
        if wikitext {
            output.push_str(&html_escape::encode_text(&rest[..end]));
        }
        if wikitext || !token.closing && !matches!(name, "html" | "head" | "body") {
            let kind = if !wikitext && !matches!(name, "script" | "style" | "title" | "base") {
                DiagnosticKind::ConversionLoss
            } else {
                DiagnosticKind::HtmlRemoval
            };
            diagnostics.push(Diagnostic::new(
                kind,
                pos..pos + end,
                format!("prohibited HTML element <{name}> removed"),
            ));
        }
        return end;
    }

    if token.closing {
        if !VOID.contains(&name) {
            output.push_str(&format!("</{name}>"));
        }
        return token.end;
    }
    output.push('<');
    output.push_str(name);
    let itemscope = token.attrs.iter().any(|a| a.name == "itemscope");
    let mut seen = std::collections::HashSet::new();
    for attr in &token.attrs {
        let key = attr.name.as_str();
        let mut value = html_escape::decode_html_entities(attr.value);
        if key == "style" && value != "/* insecure input */" && value.contains("/*") {
            value = css_without_comments(&value).into();
        }
        let scoped = itemscope || !matches!(key, "itemid" | "itemref" | "itemtype");
        if !wikitext && name == "a" && matches!(key, "rel" | "rev") {
            if !matches!(value.as_ref(), "mw:WikiLink" | "mw:ExtLink") {
                diagnostics.push(Diagnostic::loss(
                    pos + attr.span.start..pos + attr.span.end,
                    format!("link attribute {key} cannot be represented in wikitext"),
                ));
            }
        } else if allowed_attr(name, key)
            && safe_attr(name, key, &value)
            && scoped
            && seen.insert(key)
            && !key.contains(['\'', '"', '\0', '<'])
        {
            let value = if wikitext && key == "id" {
                value.trim().replace(' ', "_")
            } else {
                value.into_owned()
            };
            output.push_str(&format!(
                " {key}=\"{}\"",
                html_escape::encode_double_quoted_attribute(&value)
            ));
        } else if !wikitext
            && (key == "rel" && value.starts_with("mw:")
                || key == "data-parsoid"
                || key == "data-mw")
        {
            // Parsoid bookkeeping is consumed separately from rendered HTML.
        } else {
            let kind = if matches!(key, "href" | "cite" | "src" | "style" | "id")
                || key.starts_with("aria-")
            {
                DiagnosticKind::ConversionLoss
            } else {
                DiagnosticKind::HtmlRemoval
            };
            diagnostics.push(Diagnostic::new(
                kind,
                pos + attr.span.start..pos + attr.span.end,
                format!("prohibited HTML attribute {key} on <{name}> removed"),
            ));
        }
    }
    output.push('>');
    if wikitext && name == "pre" && !token.self_closing {
        let body = &rest[token.end..];
        if let Some((close, end)) = raw_close(body, name) {
            output.push_str(&html_escape::encode_text(
                &html_escape::decode_html_entities(&body[..close]),
            ));
            output.push_str("</pre>");
            return token.end + end;
        }
    }
    if wikitext && token.self_closing && !VOID.contains(&name) {
        output.push_str(&format!("</{name}>"));
    }
    token.end
}
