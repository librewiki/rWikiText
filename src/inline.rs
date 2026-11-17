use crate::ConversionError;

#[derive(Debug, Clone)]
pub(crate) enum Inline {
    Text(String),
    Emphasis {
        strong: bool,
        children: Vec<Inline>,
    },
    Html(String),
    Link {
        href: String,
        label: Vec<Inline>,
        internal: bool,
    },
}

pub(crate) fn text(out: &mut Vec<Inline>, value: &str) {
    if value.is_empty() {
        return;
    }
    if let Some(Inline::Text(last)) = out.last_mut() {
        last.push_str(value);
    } else {
        out.push(Inline::Text(value.to_owned()));
    }
}

pub(crate) fn encode_component(input: &str) -> String {
    let mut out = String::new();
    for c in input.bytes() {
        if c.is_ascii_alphanumeric()
            || matches!(
                c,
                b'-' | b'_'
                    | b'.'
                    | b'~'
                    | b'/'
                    | b':'
                    | b'('
                    | b')'
                    | b'!'
                    | b'$'
                    | b'\''
                    | b'*'
                    | b','
                    | b';'
                    | b'@'
            )
        {
            out.push(c as char);
        } else {
            out.push_str(&format!("%{c:02X}"));
        }
    }
    out
}
pub(crate) fn decode_component(input: &str) -> Result<String, ConversionError> {
    let mut bytes = Vec::new();
    let mut it = input.as_bytes().iter().copied();
    while let Some(c) = it.next() {
        if c == b'%' {
            let h = it.next().and_then(|v| (v as char).to_digit(16));
            let l = it.next().and_then(|v| (v as char).to_digit(16));
            bytes.push(
                (h.ok_or_else(|| ConversionError::new("invalid URL encoding"))? * 16
                    + l.ok_or_else(|| ConversionError::new("invalid URL encoding"))?)
                    as u8,
            );
        } else {
            bytes.push(c);
        }
    }
    String::from_utf8(bytes).map_err(|_| ConversionError::new("invalid URL encoding"))
}
pub(crate) fn namespace(prefix: &str) -> Option<&'static str> {
    Some(
        match prefix.replace(' ', "_").to_ascii_lowercase().as_str() {
            "media" => "Media",
            "special" => "Special",
            "talk" => "Talk",
            "user" => "User",
            "user_talk" => "User_talk",
            "project" => "Project",
            "project_talk" => "Project_talk",
            "mediawiki" => "MediaWiki",
            "mediawiki_talk" => "MediaWiki_talk",
            "template" => "Template",
            "template_talk" => "Template_talk",
            "help" => "Help",
            "help_talk" => "Help_talk",
            "file" | "image" => "File",
            "file_talk" | "image_talk" => "File_talk",
            "category" => "Category",
            "category_talk" => "Category_talk",
            _ => return None,
        },
    )
}

pub(crate) fn normalize_title(title: &str) -> String {
    fn uppercase_first(value: &str) -> String {
        let mut chars = value.chars();
        chars.next().map_or_else(String::new, |first| {
            first.to_uppercase().chain(chars).collect()
        })
    }

    let title = normalize_fragment(title);
    if let Some((prefix, name)) = title.split_once(':') {
        if let Some(namespace) = namespace(prefix) {
            let name = uppercase_first(name.trim_start_matches('_'));
            return format!("{namespace}:{name}");
        }
    }
    uppercase_first(&title)
}
pub(crate) fn normalize_fragment(fragment: &str) -> String {
    fragment
        .replace('_', " ")
        .split_whitespace()
        .collect::<Vec<_>>()
        .join("_")
}
pub(crate) fn internal_href(target: &str) -> Result<String, ConversionError> {
    if target
        .chars()
        .any(|c| c.is_control() || matches!(c, '[' | ']' | '{' | '}' | '|' | '<' | '>'))
    {
        return Err(ConversionError::new("invalid internal link target"));
    }
    let (title, fragment) = match target.split_once('#') {
        Some((a, b)) => (a, Some(b)),
        None => (target, None),
    };
    let title = normalize_title(title.strip_prefix(':').unwrap_or(title));
    if (title.is_empty() && fragment.is_none()) || title.ends_with(':') {
        return Err(ConversionError::new("empty internal link target"));
    }
    let mut href = if title.is_empty() {
        String::new()
    } else {
        format!("./{}", encode_component(&title))
    };
    if let Some(fragment) = fragment {
        href.push('#');
        href.push_str(&encode_component(&normalize_fragment(fragment)));
    }
    Ok(href)
}
fn invalid_url_character(c: char) -> bool {
    c.is_control() || c.is_whitespace() || matches!(c, '<' | '>' | '"' | '[' | ']' | '\\')
}

fn external_valid(url: &str) -> bool {
    crate::html_policy::external_url(url) && !url.chars().any(invalid_url_character)
}
pub(crate) fn link_target(href: &str) -> Result<(String, bool), ConversionError> {
    if external_valid(href) {
        return Ok((href.to_owned(), false));
    }
    let target = if let Some(s) = href.strip_prefix("./") {
        s
    } else if let Some(s) = href.strip_prefix("/wiki/") {
        s
    } else if href.starts_with('#') {
        href
    } else {
        return Err(ConversionError::new("unsupported or unsafe link target"));
    };
    let (page, fragment) = target
        .split_once('#')
        .map_or((target, None), |(page, fragment)| (page, Some(fragment)));
    let page = page.strip_suffix("?action=edit&redlink=1").unwrap_or(page);
    let mut decoded = decode_component(page)?;
    if let Some(fragment) = fragment {
        decoded.push('#');
        decoded.push_str(&decode_component(fragment)?);
    }
    Ok((internal_href(&decoded)?, true))
}

fn decoded(input: &str) -> String {
    html_escape::decode_html_entities(input).into_owned()
}

fn character_reference(input: &str) -> Option<(&str, String)> {
    let name = input.strip_prefix('&')?;
    let end = name
        .bytes()
        .take_while(|byte| byte.is_ascii_alphanumeric() || *byte == b'#')
        .count();
    if name.as_bytes().get(end) != Some(&b';') {
        return None;
    }
    let source = &input[..end + 2];
    let value = decoded(source);
    (value != source).then_some((source, value))
}

fn bare_external(input: &str, position: usize) -> Option<(&str, String)> {
    let suffix = &input[position..];
    if !crate::html_policy::PROTOCOLS.iter().any(|prefix| {
        *prefix != "//"
            && suffix
                .get(..prefix.len())
                .is_some_and(|start| start.eq_ignore_ascii_case(prefix))
    }) || (!input[..position].ends_with("<nowiki/>")
        && input[..position]
            .chars()
            .next_back()
            .is_some_and(|c| c.is_alphanumeric() || c == '_'))
    {
        return None;
    }

    let mut length = 0;
    let mut last_entity_end = 0;
    while let Some(c) = suffix[length..].chars().next() {
        if invalid_url_character(c) || c == '{' || suffix[length..].starts_with("''") {
            break;
        }
        if let Some((source, value)) = character_reference(&suffix[length..]) {
            if value.chars().any(invalid_url_character) {
                break;
            }
            length += source.len();
            last_entity_end = length;
        } else {
            length += c.len_utf8();
        }
    }

    let has_left_parenthesis = suffix[..length].contains('(');
    let tail = suffix[last_entity_end..length].trim_end_matches(|c| {
        matches!(c, '.' | ',' | ';' | ':' | '!' | '?') || (c == ')' && !has_left_parenthesis)
    });
    let source = &suffix[..last_entity_end + tail.len()];
    let url = decoded(source);
    external_valid(&url).then_some((source, url))
}

pub(crate) fn parse_wiki(input: &str, language: &str) -> Result<Vec<Inline>, ConversionError> {
    parse(input, language, true)
}

fn parse(input: &str, language: &str, links: bool) -> Result<Vec<Inline>, ConversionError> {
    let mut result = Vec::new();
    let (mut start, mut position) = (0, 0);
    while position < input.len() {
        let rest = &input[position..];
        // Links and embedded HTML are parsed as single tokens; don't split them
        // merely because their contents span multiple source lines.
        if links && rest.starts_with("[[") {
            if let Some(end) = rest.find("]]") {
                position += end + 2;
                continue;
            }
        }
        if let Some((_, end)) = crate::html_policy::wiki_fragment(rest) {
            position += end;
            continue;
        }
        if rest.starts_with('\n') {
            result.extend(parse_line(&input[start..position], language, links)?);
            text(&mut result, "\n");
            start = position + 1;
        }
        position += rest.chars().next().unwrap().len_utf8();
    }
    result.extend(parse_line(&input[start..], language, links)?);
    Ok(result)
}

#[derive(Debug)]
enum QuotePart {
    Content(Vec<Inline>),
    Quote {
        width: usize,
        previous: Option<char>,
        before_previous: Option<char>,
    },
}

fn parse_line(input: &str, language: &str, links: bool) -> Result<Vec<Inline>, ConversionError> {
    let mut parts = Vec::new();
    let mut out = Vec::new();
    let mut pos = 0;
    while pos < input.len() {
        let rest = &input[pos..];
        if let Some(body) = rest.strip_prefix("[[").filter(|_| links) {
            let end = body
                .find("]]")
                .ok_or_else(|| ConversionError::new("unclosed internal link"))?;
            let link = &body[..end];
            let (target, label) = match link.split_once('|') {
                Some((target, label)) if !label.contains('|') && !label.is_empty() => {
                    (target, label)
                }
                Some(_) => return Err(ConversionError::new("unsupported wikilink label")),
                None => (link, link.trim_start().strip_prefix(':').unwrap_or(link)),
            };
            let target = decoded(target);
            let href = match internal_href(&target) {
                Ok(href) => href,
                Err(_) => {
                    text(&mut out, "[[");
                    out.extend(parse(link, language, false)?);
                    text(&mut out, "]]");
                    pos += end + 4;
                    continue;
                }
            };
            let trail_len = body[end + 2..]
                .bytes()
                .take_while(|b| b.is_ascii_lowercase())
                .count();
            // Linktrail rules vary by site language; only the English rule is supported.
            if body[end + 2..]
                .chars()
                .next()
                .is_some_and(char::is_alphabetic)
                && !language
                    .split('-')
                    .next()
                    .unwrap_or("")
                    .eq_ignore_ascii_case("en")
            {
                return Err(ConversionError::new(
                    "linktrails for this site language are unsupported",
                ));
            }
            let mut label = parse(label, language, false)?;
            text(&mut label, &body[end + 2..end + 2 + trail_len]);
            out.push(Inline::Link {
                href,
                label,
                internal: true,
            });
            pos += end + 4 + trail_len;
        } else if links
            && rest.strip_prefix('[').is_some_and(|tail| {
                let candidate = tail
                    .split(|c: char| c.is_whitespace() || c == ']')
                    .next()
                    .unwrap_or("");
                external_valid(&decoded(candidate))
            })
        {
            let end = rest
                .find(']')
                .ok_or_else(|| ConversionError::new("unclosed external link"))?;
            let body = &rest[1..end];
            let (url, label) = body
                .split_once(char::is_whitespace)
                .ok_or_else(|| ConversionError::new("external links require a label"))?;
            let label = label.trim_start();
            let url = decoded(url);
            if !external_valid(&url) || label.is_empty() || label.contains(['[', ']']) {
                return Err(ConversionError::new("invalid external link"));
            }
            out.push(Inline::Link {
                href: url,
                label: parse(label, language, false)?,
                internal: false,
            });
            pos += end + 1;
        } else if rest.starts_with("''") {
            let count = rest.bytes().take_while(|&b| b == b'\'').count();
            let width = if count >= 5 {
                5
            } else if count >= 3 {
                3
            } else {
                2
            };
            let literal = count - width;
            if literal > 0 {
                text(&mut out, &"'".repeat(literal));
            }
            let mut previous = input[..pos + literal]
                .strip_suffix("<nowiki/>")
                .unwrap_or(&input[..pos + literal])
                .chars()
                .rev();
            parts.push(QuotePart::Content(std::mem::take(&mut out)));
            parts.push(QuotePart::Quote {
                width,
                previous: previous.next(),
                before_previous: previous.next(),
            });
            pos += count;
        } else if rest.starts_with("<nowiki/>") {
            pos += 9;
            continue;
        } else if let Some((_, end)) = crate::html_policy::wiki_fragment(rest) {
            out.push(Inline::Html(crate::embedded_html(&rest[..end], language)));
            pos += end;
        } else if let Some((source, url)) = links.then(|| bare_external(input, pos)).flatten() {
            out.push(Inline::Link {
                href: url.clone(),
                label: vec![Inline::Text(url)],
                internal: false,
            });
            pos += source.len();
        } else if let Some((source, value)) = character_reference(rest) {
            text(&mut out, &value);
            pos += source.len();
        } else {
            let ch = rest.chars().next().unwrap();
            if rest.starts_with("''") || ch == '<' {
                return Err(ConversionError::new("unsupported wikitext syntax"));
            }
            pos += ch.len_utf8();
            text(&mut out, &rest[..ch.len_utf8()]);
        }
    }
    parts.push(QuotePart::Content(out));
    Ok(resolve_quotes(parts))
}

#[derive(Clone, Copy)]
enum QuoteOp {
    Open(bool),
    Close,
}

// Parsoid resolves quote markers per line, balancing odd italic/bold counts
// before deciding the order of tags for a five-apostrophe marker.
fn resolve_quotes(mut parts: Vec<QuotePart>) -> Vec<Inline> {
    let (mut italics, mut bold) = (0, 0);
    for part in &parts {
        if let QuotePart::Quote { width, .. } = part {
            italics += usize::from(*width == 2 || *width == 5);
            bold += usize::from(*width == 3 || *width == 5);
        }
    }
    if italics % 2 == 1 && bold % 2 == 1 {
        let mut candidates = [None; 3];
        for (i, part) in parts.iter().enumerate() {
            if let QuotePart::Quote {
                width: 3,
                previous,
                before_previous,
            } = part
            {
                let whitespace = |c: Option<char>| c.is_none_or(char::is_whitespace);
                let category = if whitespace(*previous) {
                    2
                } else if whitespace(*before_previous) {
                    0
                } else {
                    1
                };
                if candidates[category].is_none() {
                    candidates[category] = Some(i);
                }
            }
        }
        if let Some(index) = candidates.into_iter().flatten().next() {
            if let QuotePart::Content(previous) = &mut parts[index - 1] {
                text(previous, "'");
            }
            if let QuotePart::Quote { width, .. } = &mut parts[index] {
                *width = 2;
            }
        }
    }
    use QuoteOp::{Close as C, Open as O};
    let mut actions = vec![Vec::<QuoteOp>::new(); parts.len()];
    let mut state = "";
    let mut both = 0;
    for index in 0..parts.len() {
        let QuotePart::Quote { width, .. } = parts[index] else {
            continue;
        };
        let (next, ops): (&str, Vec<QuoteOp>) = match (width, state) {
            (2, "i") => ("", vec![C]),
            (2, "bi") => ("b", vec![C]),
            (2, "ib") => ("b", vec![C, C, O(true)]),
            (2, "both") => {
                actions[both] = vec![O(true), O(false)];
                ("b", vec![C])
            }
            (2, "b") => ("bi", vec![O(false)]),
            (2, "") => ("i", vec![O(false)]),
            (3, "b") => ("", vec![C]),
            (3, "ib") => ("i", vec![C]),
            (3, "bi") => ("i", vec![C, C, O(false)]),
            (3, "both") => {
                actions[both] = vec![O(false), O(true)];
                ("i", vec![C])
            }
            (3, "i") => ("ib", vec![O(true)]),
            (3, "") => ("b", vec![O(true)]),
            (5, "b") => ("i", vec![C, O(false)]),
            (5, "i") => ("b", vec![C, O(true)]),
            (5, "bi") | (5, "ib") => ("", vec![C, C]),
            (5, "both") => {
                actions[both] = vec![O(false), O(true)];
                ("", vec![C, C])
            }
            (5, "") => {
                both = index;
                ("both", vec![])
            }
            _ => unreachable!(),
        };
        actions[index] = ops;
        state = next;
    }
    if state == "both" {
        actions[both] = vec![O(true), O(false)];
    }
    let mut result = Vec::new();
    let mut stack: Vec<(bool, Vec<Inline>)> = Vec::new();
    fn append(result: &mut Vec<Inline>, stack: &mut [(bool, Vec<Inline>)], item: Inline) {
        if let Some((_, children)) = stack.last_mut() {
            children.push(item);
        } else {
            result.push(item);
        }
    }
    fn close(result: &mut Vec<Inline>, stack: &mut Vec<(bool, Vec<Inline>)>) {
        if let Some((strong, children)) = stack.pop() {
            append(result, stack, Inline::Emphasis { strong, children });
        }
    }
    for (part, ops) in parts.into_iter().zip(actions) {
        if let QuotePart::Content(items) = part {
            for item in items {
                append(&mut result, &mut stack, item);
            }
        }
        for op in ops {
            match op {
                O(strong) => stack.push((strong, Vec::new())),
                C => close(&mut result, &mut stack),
            }
        }
    }
    while !stack.is_empty() {
        close(&mut result, &mut stack);
    }
    result
}

pub(crate) fn wiki_target(href: &str) -> Result<String, ConversionError> {
    let value = href.strip_prefix("./").unwrap_or(href);
    Ok(decode_component(value)?.replace('_', " "))
}
pub(crate) fn plain(inlines: &[Inline]) -> String {
    inlines
        .iter()
        .map(|i| match i {
            Inline::Text(s) => s.clone(),
            Inline::Emphasis { children, .. }
            | Inline::Link {
                label: children, ..
            } => plain(children),
            Inline::Html(html) => scraper::Html::parse_fragment(html)
                .root_element()
                .text()
                .collect(),
        })
        .collect()
}
