use crate::{inline, ConversionError, Format, Inline};
use scraper::{ElementRef, Node};

#[derive(Debug, Clone)]
pub(crate) struct ListLine {
    pub markers: String,
    pub content: Vec<Inline>,
}

#[derive(Debug, Clone)]
pub(crate) enum Block {
    Pre(String),
    WikiPre(Vec<Inline>),
    Rule,
    RuleWithText(Vec<Inline>),
    List(Vec<ListLine>),
}

pub(crate) fn parse_wiki(
    lines: &[&str],
    language: &str,
) -> Result<Option<(Block, usize)>, ConversionError> {
    let Some(first) = lines.first() else {
        return Ok(None);
    };
    if first.starts_with("----") {
        let count = first.bytes().take_while(|c| *c == b'-').count();
        let rest = &first[count..];
        return Ok(Some((
            if rest.trim().is_empty() {
                Block::Rule
            } else {
                Block::RuleWithText(inline::parse_wiki(rest, language)?)
            },
            1,
        )));
    }
    if first.starts_with(' ') {
        let content: Vec<_> = lines
            .iter()
            .map_while(|line| line.strip_prefix(' '))
            .collect();
        return Ok(Some((
            Block::WikiPre(inline::parse_wiki(&content.join("\n"), language)?),
            content.len(),
        )));
    }
    if !first.starts_with(['*', '#', ';', ':']) {
        return Ok(None);
    }
    let mut items = Vec::new();
    let mut consumed = 0;
    for line in lines {
        let n = line
            .bytes()
            .take_while(|c| matches!(c, b'*' | b'#' | b';' | b':'))
            .count();
        if n == 0 {
            break;
        }
        consumed += 1;
        let markers = &line[..n];
        let content = line[n..].trim_start_matches([' ', '\t']);
        if markers.ends_with(';') {
            if let Some((term, definition)) = split_definition(content) {
                items.push(ListLine {
                    markers: markers.to_owned(),
                    content: inline::parse_wiki(term.trim_end(), language)?,
                });
                let mut def_markers = markers.to_owned();
                def_markers.pop();
                def_markers.push(':');
                items.push(ListLine {
                    markers: def_markers,
                    content: inline::parse_wiki(definition.trim_start(), language)?,
                });
                continue;
            }
        }
        items.push(ListLine {
            markers: markers.to_owned(),
            content: inline::parse_wiki(content, language)?,
        });
    }
    Ok(Some((Block::List(items), consumed)))
}

fn split_definition(content: &str) -> Option<(&str, &str)> {
    let mut depth = 0usize;
    let mut index = 0;
    while index < content.len() {
        let rest = &content[index..];
        if rest.starts_with("[[") || rest.starts_with("{{") {
            depth += 1;
            index += 2;
        } else if rest.starts_with("]]") || rest.starts_with("}}") {
            depth = depth.saturating_sub(1);
            index += 2;
        } else if rest.starts_with(':') && depth == 0 {
            return Some((&content[..index], &content[index + 1..]));
        } else {
            index += rest.chars().next().unwrap().len_utf8();
        }
    }
    None
}

fn tag(marker: char) -> &'static str {
    match marker {
        '*' => "ul",
        '#' => "ol",
        ';' | ':' => "dl",
        _ => unreachable!(),
    }
}
fn item_tag(marker: char) -> &'static str {
    match marker {
        '*' | '#' => "li",
        ';' => "dt",
        ':' => "dd",
        _ => unreachable!(),
    }
}
fn same_container(a: char, b: char) -> bool {
    tag(a) == tag(b)
}

pub(crate) fn list_type(block: &Block) -> Option<bool> {
    match block {
        Block::List(lines) => match lines.first()?.markers.chars().next()? {
            '*' => Some(false),
            '#' => Some(true),
            _ => None,
        },
        _ => None,
    }
}

pub(crate) fn render(block: &Block, format: Format, alternate: bool) -> String {
    match block {
        Block::Rule => match format {
            Format::Wikitext => "----",
            Format::Gfm => "---",
            Format::Html => "<hr>",
        }
        .into(),
        Block::RuleWithText(text) => match format {
            Format::Wikitext => format!("----{}", crate::render_inline(text, format)),
            Format::Gfm => {
                let rendered = crate::render_inline(text, format);
                let leading = rendered.bytes().take_while(|b| *b == b' ').count();
                format!("---\n\n{}{}", "&#32;".repeat(leading), &rendered[leading..])
            }
            Format::Html => format!("<hr><p>{}</p>", crate::render_inline(text, format)),
        },
        Block::Pre(text) => match format {
            Format::Wikitext => crate::escape_wikitext(text)
                .split('\n')
                .map(|line| format!(" {line}"))
                .collect::<Vec<_>>()
                .join("\n"),
            Format::Html => format!("<pre>{}</pre>", html_escape::encode_text(text)),
            Format::Gfm => format!("<pre>{}</pre>", html_escape::encode_text(text)),
        },
        Block::WikiPre(content) => match format {
            Format::Wikitext => crate::render_inline(content, format)
                .split('\n')
                .map(|line| format!(" {line}"))
                .collect::<Vec<_>>()
                .join("\n"),
            Format::Html | Format::Gfm => {
                format!("<pre>{}</pre>", crate::render_inline(content, Format::Html))
            }
        },
        Block::List(lines) if format == Format::Wikitext => lines
            .iter()
            .map(|line| {
                format!(
                    "{} {}",
                    line.markers,
                    crate::render_inline(&line.content, format)
                )
            })
            .collect::<Vec<_>>()
            .join("\n"),
        Block::List(lines)
            if format == Format::Gfm
                && lines.iter().all(|line| !line.markers.contains([';', ':'])) =>
        {
            lines
                .iter()
                .map(|line| {
                    let indent: usize = line
                        .markers
                        .chars()
                        .take(line.markers.len().saturating_sub(1))
                        .map(|c| if c == '#' { 3 } else { 2 })
                        .sum();
                    let marker = match (line.markers.ends_with('#'), alternate) {
                        (true, false) => "1.",
                        (true, true) => "1)",
                        (false, false) => "-",
                        (false, true) => "*",
                    };
                    format!(
                        "{}{} {}",
                        " ".repeat(indent),
                        marker,
                        crate::render_inline(&line.content, format)
                    )
                })
                .collect::<Vec<_>>()
                .join("\n")
        }
        Block::List(lines) => render_html_list(lines),
    }
}
fn render_html_list(lines: &[ListLine]) -> String {
    let mut output = String::new();
    let mut path: Vec<char> = Vec::new();
    for line in lines {
        let next: Vec<char> = line.markers.chars().collect();
        let shared = path
            .iter()
            .zip(&next)
            .take_while(|(a, b)| same_container(**a, **b))
            .count();
        while path.len() > shared {
            let old = path.pop().unwrap();
            output.push_str(&format!("</{}></{}>", item_tag(old), tag(old)));
        }
        if shared == next.len() && shared > 0 {
            let old = path.pop().unwrap();
            output.push_str(&format!(
                "</{}><{}>",
                item_tag(old),
                item_tag(next[shared - 1])
            ));
            path.push(next[shared - 1]);
        } else {
            for marker in &next[shared..] {
                output.push_str(&format!("<{}><{}>", tag(*marker), item_tag(*marker)));
                path.push(*marker);
            }
        }
        output.push_str(&crate::render_inline(&line.content, Format::Html));
    }
    while let Some(old) = path.pop() {
        output.push_str(&format!("</{}></{}>", item_tag(old), tag(old)));
    }
    output
}

// The callback parses inline children while ignoring nested list elements.
pub(crate) fn parse_html<F>(
    element: ElementRef<'_>,
    parse_inline: &F,
) -> Result<Option<Block>, ConversionError>
where
    F: Fn(ElementRef<'_>) -> Result<Vec<Inline>, ConversionError>,
{
    fn list<F>(
        element: ElementRef<'_>,
        prefix: &str,
        output: &mut Vec<ListLine>,
        parse_inline: &F,
    ) -> Result<(), ConversionError>
    where
        F: Fn(ElementRef<'_>) -> Result<Vec<Inline>, ConversionError>,
    {
        let name = element.value().name();
        if name == "ol"
            && (element.value().attr("reversed").is_some()
                || element.value().attr("start").is_some_and(|v| v != "1"))
        {
            return Err(ConversionError::new(
                "ordered list numbering is unsupported",
            ));
        }
        for child in element.children() {
            match child.value() {
                Node::Text(t) if t.trim().is_empty() => (),
                Node::Comment(_) => (),
                Node::Element(el)
                    if matches!((name, el.name()), ("ul" | "ol", "li") | ("dl", "dt" | "dd"))
                        && el.attr("value").is_none() =>
                {
                    let marker = match (name, el.name()) {
                        ("ul", _) => '*',
                        ("ol", _) => '#',
                        (_, "dt") => ';',
                        _ => ':',
                    };
                    let path = format!("{prefix}{marker}");
                    let item = ElementRef::wrap(child).unwrap();
                    output.push(ListLine {
                        markers: path.clone(),
                        content: parse_inline(item)?,
                    });
                    for nested in item.children() {
                        if let Node::Element(e) = nested.value() {
                            if matches!(e.name(), "ul" | "ol" | "dl") {
                                list(
                                    ElementRef::wrap(nested).unwrap(),
                                    &path,
                                    output,
                                    parse_inline,
                                )?;
                            }
                        }
                    }
                }
                _ => return Err(ConversionError::new("unsupported HTML list content")),
            }
        }
        Ok(())
    }
    Ok(match element.value().name() {
        "hr" => Some(Block::Rule),
        "pre" => {
            let elements: Vec<_> = element.children().filter_map(ElementRef::wrap).collect();
            if elements.is_empty() || (elements.len() == 1 && elements[0].value().name() == "code")
            {
                Some(Block::Pre(element.text().collect::<String>()))
            } else {
                Some(Block::WikiPre(parse_inline(element)?))
            }
        }
        "ul" | "ol" | "dl" => {
            let mut lines = Vec::new();
            list(element, "", &mut lines, parse_inline)?;
            if lines.is_empty() {
                return Err(ConversionError::new("empty list is unsupported"));
            }
            Some(Block::List(lines))
        }
        _ => None,
    })
}
