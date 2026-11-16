use crate::ConversionError;

#[derive(Debug, Clone)]
pub(crate) enum Inline {
    Text(String),
    Link {
        href: String,
        label: String,
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

fn encode_component(input: &str) -> String {
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
fn normalize_title(title: &str) -> String {
    fn uppercase_first(value: &str) -> String {
        let mut chars = value.chars();
        chars.next().map_or_else(String::new, |first| {
            first.to_uppercase().chain(chars).collect()
        })
    }

    let title = normalize_fragment(title);
    if let Some((prefix, name)) = title.split_once(':') {
        let namespace = match prefix.to_ascii_lowercase().as_str() {
            "talk" => "Talk",
            "user" => "User",
            "user_talk" => "User_talk",
            "mediawiki" => "MediaWiki",
            "mediawiki_talk" => "MediaWiki_talk",
            "template" => "Template",
            "template_talk" => "Template_talk",
            "help" => "Help",
            "help_talk" => "Help_talk",
            "file_talk" | "image_talk" => "File_talk",
            "category_talk" => "Category_talk",
            _ => return uppercase_first(&title),
        };
        let name = uppercase_first(name.trim_start_matches('_'));
        return format!("{namespace}:{name}");
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
fn internal_href(target: &str) -> Result<String, ConversionError> {
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
    let title = normalize_title(title);
    if title.is_empty() && fragment.is_none() {
        return Err(ConversionError::new("empty internal link target"));
    }
    let lower = title.to_lowercase();
    if lower.starts_with("category:") || lower.starts_with("file:") || lower.starts_with("image:") {
        return Err(ConversionError::new("unsupported special wiki link"));
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
    let lower = url.to_ascii_lowercase();
    let host = if lower.starts_with("https://") {
        &url[8..]
    } else if lower.starts_with("http://") {
        &url[7..]
    } else {
        return false;
    };
    !host.is_empty()
        && !host.starts_with(['/', '?', '#'])
        && !url.chars().any(invalid_url_character)
}
pub(crate) fn from_href(href: &str, label: String) -> Result<Inline, ConversionError> {
    if label.is_empty() {
        return Err(ConversionError::new("empty link labels are unsupported"));
    }
    if external_valid(href) {
        return Ok(Inline::Link {
            href: href.to_owned(),
            label,
            internal: false,
        });
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
    let decoded = decode_component(target)?;
    let normalized = internal_href(&decoded)?;
    Ok(Inline::Link {
        href: normalized,
        label,
        internal: true,
    })
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
    if !(suffix.starts_with("http://") || suffix.starts_with("https://"))
        || input[..position]
            .chars()
            .next_back()
            .is_some_and(|c| c.is_alphanumeric() || c == '_')
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
    let mut out = Vec::new();
    let mut pos = 0;
    while pos < input.len() {
        let rest = &input[pos..];
        if let Some(body) = rest.strip_prefix("[[") {
            let end = body
                .find("]]")
                .ok_or_else(|| ConversionError::new("unclosed internal link"))?;
            let link = &body[..end];
            let (target, label) = match link.split_once('|') {
                Some((target, label)) if !label.contains('|') && !label.is_empty() => {
                    (target, label)
                }
                Some(_) => return Err(ConversionError::new("unsupported wikilink label")),
                None => (link, link),
            };
            if label.contains("[[")
                || label.contains("{{")
                || label.contains('<')
                || label.contains("''")
            {
                return Err(ConversionError::new("unsupported link label"));
            }
            let target = decoded(target);
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
            let mut label = decoded(label);
            label.push_str(&body[end + 2..end + 2 + trail_len]);
            out.push(Inline::Link {
                href: internal_href(&target)?,
                label,
                internal: true,
            });
            pos += end + 4 + trail_len;
        } else if rest.starts_with("[http://") || rest.starts_with("[https://") {
            let end = rest
                .find(']')
                .ok_or_else(|| ConversionError::new("unclosed external link"))?;
            let body = &rest[1..end];
            let (url, label) = body
                .split_once(char::is_whitespace)
                .ok_or_else(|| ConversionError::new("external links require a label"))?;
            let label = label.trim_start();
            let url = decoded(url);
            if !external_valid(&url)
                || label.is_empty()
                || label.contains(['[', ']', '<'])
                || label.contains("{{")
                || label.contains("''")
            {
                return Err(ConversionError::new("invalid external link"));
            }
            out.push(Inline::Link {
                href: url,
                label: decoded(label),
                internal: false,
            });
            pos += end + 1;
        } else if let Some((source, url)) = bare_external(input, pos) {
            out.push(Inline::Link {
                href: url.clone(),
                label: url,
                internal: false,
            });
            pos += source.len();
        } else if let Some((source, value)) = character_reference(rest) {
            text(&mut out, &value);
            pos += source.len();
        } else {
            let ch = rest.chars().next().unwrap();
            if rest.starts_with("{{") || rest.starts_with("''") || ch == '<' {
                return Err(ConversionError::new("unsupported wikitext syntax"));
            }
            pos += ch.len_utf8();
            text(&mut out, &rest[..ch.len_utf8()]);
        }
    }
    Ok(out)
}

pub(crate) fn wiki_target(href: &str) -> Result<String, ConversionError> {
    let value = href.strip_prefix("./").unwrap_or(href);
    Ok(decode_component(value)?.replace('_', " "))
}
pub(crate) fn plain(inlines: &[Inline]) -> String {
    inlines
        .iter()
        .map(|i| match i {
            Inline::Text(s) | Inline::Link { label: s, .. } => s.as_str(),
        })
        .collect()
}
