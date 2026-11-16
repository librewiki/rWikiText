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
fn decode_component(input: &str) -> Result<String, ConversionError> {
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
    let clean = title.replace('_', " ");
    let clean = clean.split_whitespace().collect::<Vec<_>>().join("_");
    let mut chars = clean.chars();
    match chars.next() {
        Some(c) => c.to_uppercase().chain(chars).collect(),
        None => clean,
    }
}
fn normalize_fragment(fragment: &str) -> String {
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
        && !url.chars().any(|c| {
            c.is_control() || c.is_whitespace() || matches!(c, '<' | '>' | '"' | '[' | ']' | '\\')
        })
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
                    (target, decoded(label))
                }
                Some(_) => return Err(ConversionError::new("unsupported wikilink label")),
                None => (link, decoded(&link.replace('_', " "))),
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
            let mut label = label;
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
            let label = decoded(label.trim());
            let url = decoded(url);
            if !external_valid(&url) || label.is_empty() || label.contains(['[', ']', '<']) {
                return Err(ConversionError::new("invalid external link"));
            }
            out.push(Inline::Link {
                href: url,
                label,
                internal: false,
            });
            pos += end + 1;
        } else if rest.starts_with("http://") || rest.starts_with("https://") {
            let end = rest
                .find(|c: char| c.is_whitespace() || matches!(c, '<' | '>' | '[' | ']' | '"'))
                .unwrap_or(rest.len());
            let candidate = &rest[..end];
            let mut trimmed = candidate.trim_end_matches(['.', ',', ';', '!', '?']);
            while trimmed.ends_with(')')
                && trimmed.matches(')').count() > trimmed.matches('(').count()
            {
                trimmed = &trimmed[..trimmed.len() - 1];
            }
            let url = decoded(trimmed);
            if !external_valid(&url) {
                return Err(ConversionError::new("invalid external link"));
            }
            out.push(Inline::Link {
                href: url.clone(),
                label: url,
                internal: false,
            });
            pos += trimmed.len();
        } else {
            let ch = rest.chars().next().unwrap();
            if rest.starts_with("{{") || rest.starts_with("''") || ch == '<' {
                return Err(ConversionError::new("unsupported wikitext syntax"));
            }
            let start = pos;
            pos += ch.len_utf8();
            // Decode character references without interpreting their resulting punctuation as markup.
            if ch == '&' {
                if let Some(end) = input[start..].find(';') {
                    let entity = &input[start..start + end + 1];
                    let value = decoded(entity);
                    if value != entity {
                        text(&mut out, &value);
                        pos = start + end + 1;
                        continue;
                    }
                }
            }
            text(&mut out, &input[start..pos]);
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
