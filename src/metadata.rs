use crate::{inline, ConversionError, PageMetadata, WikiContext};
use scraper::{Html, Selector};

pub(crate) struct MetadataLocation {
    pub start: usize,
    pub end: usize,
    pub kind: &'static str,
}
pub(crate) struct Extraction {
    pub body: String,
    pub metadata: PageMetadata,
    pub locations: Vec<MetadataLocation>,
    pub ranges: Vec<std::ops::Range<usize>>,
}

fn valid_title(s: &str) -> bool {
    !s.trim().is_empty()
        && !s
            .chars()
            .any(|c| c.is_control() || matches!(c, '[' | ']' | '{' | '}' | '|' | '<' | '>' | '?'))
}
fn valid_sort_key(s: &str) -> bool {
    !s.chars()
        .any(|c| c.is_control() || matches!(c, '[' | ']' | '{' | '}' | '|' | '<' | '>'))
}
fn title(s: &str) -> String {
    let t = s
        .replace('_', " ")
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ");
    let mut chars = t.chars();
    chars
        .next()
        .map(|c| c.to_uppercase().chain(chars).collect())
        .unwrap_or_default()
}
fn display_valid(s: &str, ctx: &WikiContext<'_>) -> bool {
    if ctx.page_title.is_empty()
        || s.chars()
            .any(|c| c.is_control() || matches!(c, '[' | ']' | '{' | '}' | '|'))
    {
        return false;
    }
    if valid_title(s) {
        let plain = html_escape::decode_html_entities(s);
        return valid_title(&plain) && title(&plain) == title(ctx.page_title);
    }
    // Admit one literal span with a color-only style; this subset has no
    // layout effects, template syntax, executable CSS, or nested elements.
    let Some(open) = s.find('<') else {
        return false;
    };
    let Some((name, len, false)) = crate::html_policy::tag_info(&s[open..]) else {
        return false;
    };
    if name != "span" {
        return false;
    }
    let start = open + len;
    let Some(relative_close) = s[start..].find('<') else {
        return false;
    };
    let close = start + relative_close;
    let Some((name, end_len, true)) = crate::html_policy::tag_info(&s[close..]) else {
        return false;
    };
    if name != "span"
        || s[close + end_len..].contains(['<', '>'])
        || s[..open].contains('>')
        || s[start..close].contains('>')
    {
        return false;
    }
    if !s[close..close + end_len].eq_ignore_ascii_case("</span>") {
        return false;
    }
    let raw_attrs = s[open + 5..start - 1].trim();
    let Some((attr, quoted)) = raw_attrs.split_once('=') else {
        return false;
    };
    if !attr.trim().eq_ignore_ascii_case("style") {
        return false;
    }
    let quoted = quoted.trim();
    let Some(quote) = quoted.chars().next().filter(|c| matches!(c, '\'' | '"')) else {
        return false;
    };
    let Some(style) = quoted
        .strip_prefix(quote)
        .and_then(|v| v.strip_suffix(quote))
    else {
        return false;
    };
    if style.contains(quote) {
        return false;
    }
    let style = style.trim().trim_end_matches(';').trim();
    let Some((property, color)) = style.split_once(':') else {
        return false;
    };
    let color = color.trim();
    const COLORS: &[&str] = &[
        "black", "silver", "gray", "white", "maroon", "red", "purple", "fuchsia", "green", "lime",
        "olive", "yellow", "navy", "blue", "teal", "aqua", "orange",
    ];
    if !property.trim().eq_ignore_ascii_case("color")
        || !(COLORS.iter().any(|name| color.eq_ignore_ascii_case(name))
            || color.strip_prefix('#').is_some_and(|hex| {
                matches!(hex.len(), 3 | 6) && hex.chars().all(|c| c.is_ascii_hexdigit())
            }))
    {
        return false;
    }
    let plain = format!(
        "{}{}{}",
        &s[..open],
        &s[start..close],
        &s[close + end_len..]
    );
    let plain = html_escape::decode_html_entities(&plain);
    valid_title(&plain) && title(&plain) == title(ctx.page_title)
}
fn blank_or_remove(
    input: &str,
    ranges: &[(usize, usize)],
) -> (String, Vec<std::ops::Range<usize>>) {
    let mut out = String::new();
    let mut at = 0;
    let mut removed = Vec::new();
    for &(start, end) in ranges {
        let line_start = input[..start].rfind('\n').map_or(0, |n| n + 1);
        let line_end = input[end..].find('\n').map_or(input.len(), |n| end + n);
        if line_start >= at
            && input[line_start..start].trim().is_empty()
            && input[end..line_end].trim().is_empty()
        {
            out.push_str(&input[at..line_start]);
            at = if line_end < input.len() {
                line_end + 1
            } else {
                line_end
            };
            removed.push(line_start..at);
        } else {
            out.push_str(&input[at..start]);
            at = end;
            removed.push(start..end);
        }
    }
    out.push_str(&input[at..]);
    (out, removed)
}
fn empty() -> Extraction {
    Extraction {
        body: String::new(),
        metadata: PageMetadata::default(),
        locations: Vec::new(),
        ranges: Vec::new(),
    }
}
fn add(e: &mut Extraction, start: usize, end: usize, kind: &'static str) {
    e.locations.push(MetadataLocation { start, end, kind });
}

pub(crate) fn redirect(input: &str) -> Option<(&str, usize)> {
    let directive = input.trim_start_matches([' ', '\t', '\r', '\n']);
    if !directive
        .get(..9)
        .is_some_and(|prefix| prefix.eq_ignore_ascii_case("#redirect"))
    {
        return None;
    }
    let rest = directive[9..].trim_start_matches([' ', '\t', '\r', '\n']);
    let rest = rest
        .strip_prefix(':')
        .unwrap_or(rest)
        .trim_start_matches([' ', '\t', '\r', '\n']);
    let link = rest.strip_prefix("[[")?;
    let end = link.find("]]")?;
    let target = &link[..end];
    valid_title(target).then_some((target, input.len() - link.len() + end + 2))
}

pub(crate) fn extract_wikitext(
    input: &str,
    raw_input: &str,
    ctx: &WikiContext<'_>,
) -> Result<Extraction, ConversionError> {
    let mut e = empty();
    let mut ranges = Vec::new();
    let mut pos = 0;
    // Redirect is a page directive only at the beginning of the raw document.
    if !raw_input
        .trim_start_matches([' ', '\t', '\r', '\n'])
        .starts_with("{{")
    {
        if let Some((target, stop)) = redirect(input) {
            e.metadata.redirect_target = Some(title(target));
            ranges.push((0, stop));
            add(&mut e, 0, stop, "redirect");
            pos = stop;
        }
    }
    while pos < input.len() {
        let rest = &input[pos..];
        if rest.starts_with("<!--") {
            if let Some(n) = rest.find("-->") {
                pos += n + 3;
                continue;
            }
            break;
        }
        if rest.starts_with('<') {
            if let Some(n) = tag_end(rest) {
                let tag = rest[..n].to_ascii_lowercase();
                if ["script", "style", "nowiki", "pre"].iter().any(|name| {
                    tag.strip_prefix(&format!("<{name}"))
                        .is_some_and(|tail| {
                            tail.starts_with([' ', '\t', '\r', '\n', '>']) || tail == "/>"
                        })
                }) {
                    if tag.ends_with("/>") {
                        pos += n;
                        continue;
                    }
                    let name = tag[1..]
                        .split(|c: char| c.is_ascii_whitespace() || c == '>')
                        .next()
                        .unwrap();
                    if let Some((_, close_end)) = crate::html_policy::raw_close(&rest[n..], name) {
                        pos += n + close_end;
                        continue;
                    }
                    break;
                }
            }
        }
        // Attribute values are HTML syntax, not wikitext page properties.
        if let Some((_, len, _)) = crate::html_policy::tag_info(rest) {
            pos += len;
            continue;
        }
        if rest.starts_with("[[") {
            if let Some(n) = rest.find("]]") {
                let inside = &rest[2..n];
                if let Some((prefix, value)) = inside.split_once(':') {
                    if prefix.eq_ignore_ascii_case("category") {
                        let (name, sort) = match value.split_once('|') {
                            Some((a, b)) => (a, Some(b)),
                            None => (value, None),
                        };
                        if valid_title(name)
                            && !name.contains('#')
                            && sort.is_none_or(valid_sort_key)
                        {
                            let end = pos + n + 2;
                            e.metadata.categories.push(title(name));
                            e.metadata.category_sort_keys.push(sort.map(str::to_owned));
                            ranges.push((pos, end));
                            add(&mut e, pos, end, "category");
                            pos = end;
                            continue;
                        }
                    }
                }
                pos += n + 2;
                continue;
            }
        }
        if rest.starts_with("{{") {
            if let Some(n) = rest.find("}}") {
                let inside = &rest[2..n];
                if let Some((name, value)) = inside.split_once(':') {
                    if name.eq_ignore_ascii_case("displaytitle") && display_valid(value, ctx) {
                        let end = pos + n + 2;
                        e.metadata.display_title = Some(value.to_owned());
                        ranges.push((pos, end));
                        add(&mut e, pos, end, "display title");
                        pos = end;
                        continue;
                    }
                }
                pos += n + 2;
                continue;
            }
        }
        pos += rest.chars().next().unwrap().len_utf8();
    }
    (e.body, e.ranges) = blank_or_remove(input, &ranges);
    Ok(e)
}
fn encode(s: &str) -> String {
    let mut out = String::new();
    for b in s.bytes() {
        if b.is_ascii_alphanumeric()
            || matches!(
                b,
                b'-' | b'_' | b'.' | b'~' | b':' | b'/' | b'\'' | b'(' | b')'
            )
        {
            out.push(b as char)
        } else {
            out.push_str(&format!("%{b:02X}"))
        }
    }
    out
}
fn href_title(s: &str) -> Option<String> {
    let raw = s.strip_prefix("./")?;
    let decoded = inline::decode_component(raw).ok()?;
    if valid_title(&decoded) {
        Some(title(&decoded))
    } else {
        None
    }
}
fn tag_end(s: &str) -> Option<usize> {
    let mut quote = None;
    for (i, c) in s.char_indices() {
        match (quote, c) {
            (Some(q), x) if q == x => quote = None,
            (None, '\'' | '"') => quote = Some(c),
            (None, '>') => return Some(i + 1),
            _ => {}
        }
    }
    None
}

// HTML raw text ends at its matching end tag; template contents are inert and
// can contain nested templates and raw-text elements. Fail closed on an
// unterminated container so its content can never become page metadata.
fn skip_inert(input: &str, mut pos: usize, name: &str) -> usize {
    if name == "plaintext" {
        return input.len();
    }
    let container = matches!(name, "template" | "svg" | "math" | "select" | "noscript");
    let mut depth = 1;
    while let Some(offset) = input[pos..].find('<') {
        pos += offset;
        if container && input[pos..].starts_with("<!--") {
            match input[pos..].find("-->") {
                Some(end) => {
                    pos += end + 3;
                    continue;
                }
                None => return input.len(),
            }
        }
        let Some((tag, len, closing)) = crate::html_policy::tag_info(&input[pos..]) else {
            pos += 1;
            continue;
        };
        pos += len;
        if tag == name {
            if closing {
                depth -= 1;
                if depth == 0 {
                    return pos;
                }
            } else if container {
                depth += 1;
            }
        } else if container
            && !closing
            && (crate::html_policy::is_inert_html(&tag)
                || matches!(tag.as_str(), "svg" | "math" | "select" | "noscript"))
        {
            pos = skip_inert(input, pos, &tag);
            if pos == input.len() {
                return pos;
            }
        }
    }
    input.len()
}

pub(crate) fn extract_html(
    input: &str,
    ctx: &WikiContext<'_>,
) -> Result<Extraction, ConversionError> {
    let mut e = empty();
    let mut ranges = Vec::new();
    let mut pos = 0;
    while pos < input.len() {
        let Some(offset) = input[pos..].find('<') else {
            break;
        };
        pos += offset;
        if input[pos..].starts_with("<!--") {
            if let Some(n) = input[pos..].find("-->") {
                pos += n + 3;
                continue;
            }
            break;
        }
        let Some(n) = tag_end(&input[pos..]) else {
            break;
        };
        let tag = &input[pos..pos + n];
        if let Some((name, _, false)) = crate::html_policy::tag_info(tag) {
            if crate::html_policy::is_inert_html(&name)
                || matches!(name.as_str(), "svg" | "math" | "select" | "noscript")
            {
                pos = skip_inert(input, pos + n, &name);
                continue;
            }
        }
        let is_link = tag
            .get(..5)
            .is_some_and(|s| s.eq_ignore_ascii_case("<link"));
        let is_span = tag
            .get(..5)
            .is_some_and(|s| s.eq_ignore_ascii_case("<span"));
        if !is_link && !is_span {
            pos += n;
            continue;
        }
        let mut end = pos + n;
        if is_span {
            if !input[end..].starts_with("</span>") {
                pos = end;
                continue;
            }
            end += 7;
        }
        let fragment = Html::parse_fragment(&input[pos..end]);
        let selector = Selector::parse(if is_link { "link" } else { "span" }).unwrap();
        let Some(node) = fragment.select(&selector).next() else {
            pos = end;
            continue;
        };
        let element = node.value();
        if element.attrs().any(|(name, _)| {
            !if is_link {
                matches!(name, "rel" | "href" | "data-parsoid")
            } else {
                matches!(name, "typeof" | "about" | "data-mw" | "data-parsoid")
            }
        }) {
            pos = end;
            continue;
        }
        if is_link {
            let Some(rel) = element.attr("rel") else {
                pos = end;
                continue;
            };
            let Some(href) = element.attr("href") else {
                pos = end;
                continue;
            };
            if element.attr("typeof").is_some()
                || element.attr("about").is_some()
                || element.attr("data-mw").is_some()
            {
                pos = end;
                continue;
            }
            if rel == "mw:PageProp/Category" {
                let (raw, sort) = href
                    .split_once('#')
                    .map_or((href, None), |(a, b)| (a, Some(b)));
                if let Some(target) = href_title(raw) {
                    if let Some(name) = target.strip_prefix("Category:") {
                        if valid_title(name) && !name.contains('#') {
                            let sort = match sort {
                                Some(s) => Some(inline::decode_component(s)?),
                                None => None,
                            };
                            if sort.as_deref().is_some_and(|s| !valid_sort_key(s)) {
                                pos = end;
                                continue;
                            }
                            e.metadata.categories.push(title(name));
                            e.metadata.category_sort_keys.push(sort);
                            ranges.push((pos, end));
                            add(&mut e, pos, end, "category");
                        }
                    }
                }
            } else if rel == "mw:PageProp/redirect" && e.metadata.redirect_target.is_none() {
                if let Some(target) = href_title(href) {
                    e.metadata.redirect_target = Some(target);
                    ranges.push((pos, end));
                    add(&mut e, pos, end, "redirect");
                }
            }
        } else if element.attr("typeof") == Some("mw:Transclusion") {
            if let Some(json) = element.attr("data-mw") {
                if let Ok(value) = serde_json::from_str::<serde_json::Value>(json) {
                    let template = value
                        .as_object()
                        .filter(|o| o.len() == 1)
                        .and_then(|_| value.get("parts"))
                        .and_then(|p| p.as_array())
                        .filter(|p| p.len() == 1)
                        .and_then(|p| {
                            p[0].as_object()
                                .filter(|o| o.len() == 1)
                                .and_then(|o| o.get("template"))
                        });
                    if let Some(t) = template {
                        let target = t.get("target");
                        let params = t.get("params").and_then(|p| p.as_object());
                        if t.as_object().is_some_and(|o| {
                            o.keys()
                                .all(|k| matches!(k.as_str(), "target" | "params" | "i"))
                        }) && target.and_then(|v| v.as_object()).is_some_and(|o| {
                            o.len() == 2
                                && o.keys().all(|k| matches!(k.as_str(), "wt" | "function"))
                        }) && t.get("i").and_then(|i| i.as_u64()) == Some(0)
                            && target
                                .and_then(|t| t.get("function"))
                                .and_then(|f| f.as_str())
                                == Some("displaytitle")
                            && params.is_some_and(|p| p.is_empty())
                        {
                            if let Some(wt) =
                                target.and_then(|t| t.get("wt")).and_then(|w| w.as_str())
                            {
                                if let Some((name, value)) = wt.split_once(':') {
                                    if name.eq_ignore_ascii_case("displaytitle")
                                        && display_valid(value, ctx)
                                    {
                                        e.metadata.display_title = Some(value.to_owned());
                                        ranges.push((pos, end));
                                        add(&mut e, pos, end, "display title");
                                    }
                                }
                            }
                        }
                    }
                }
            }
        }
        pos = end;
    }
    (e.body, e.ranges) = blank_or_remove(input, &ranges);
    Ok(e)
}
pub(crate) fn render_wikitext(m: &PageMetadata) -> String {
    let mut parts = Vec::new();
    if let Some(target) = &m.redirect_target {
        parts.push(format!("#REDIRECT [[{target}]]"));
    }
    if let Some(t) = &m.display_title {
        parts.push(format!("{{{{DISPLAYTITLE:{t}}}}}"));
    }
    for (i, c) in m.categories.iter().enumerate() {
        let sort = m
            .category_sort_keys
            .get(i)
            .and_then(|s| s.as_ref())
            .map(|s| format!("|{s}"))
            .unwrap_or_default();
        parts.push(format!("[[Category:{c}{sort}]]"));
    }
    parts.join("\n")
}
pub(crate) fn render_html(m: &PageMetadata) -> String {
    let mut parts = Vec::new();
    if let Some(t) = &m.redirect_target {
        parts.push(format!(
            "<link rel=\"mw:PageProp/redirect\" href=\"./{}\">",
            t.split_once('#').map_or_else(
                || encode(&t.replace(' ', "_")),
                |(page, fragment)| format!(
                    "{}#{}",
                    encode(&page.replace(' ', "_")),
                    encode(&fragment.replace(' ', "_"))
                )
            )
        ));
    }
    if let Some(t) = &m.display_title {
        let json=serde_json::json!({"parts":[{"template":{"target":{"wt":format!("DISPLAYTITLE:{t}"),"function":"displaytitle"},"params":{},"i":0}}]}).to_string();
        parts.push(format!(
            "<span typeof=\"mw:Transclusion\" data-mw=\"{}\"></span>",
            html_escape::encode_double_quoted_attribute(&json)
        ));
    }
    for (i, c) in m.categories.iter().enumerate() {
        let sort = m
            .category_sort_keys
            .get(i)
            .and_then(|s| s.as_ref())
            .map(|s| format!("#{}", encode(s)))
            .unwrap_or_default();
        parts.push(format!(
            "<link rel=\"mw:PageProp/Category\" href=\"./Category:{}{sort}\">",
            encode(&c.replace(' ', "_"))
        ));
    }
    parts.join("\n")
}
