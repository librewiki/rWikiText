use crate::{core, inline, Block, Format, Inline, PageMetadata};

fn escape(text: &str) -> String {
    let mut out = String::new();
    for ch in text.chars() {
        if ch.is_ascii_punctuation() {
            out.push('\\');
        }
        out.push(ch);
    }
    out
}

fn inline_text(parts: &[Inline]) -> (String, bool) {
    let mut out = String::new();
    let mut loss = false;
    for part in parts {
        match part {
            Inline::Text(text) => out.push_str(&escape(text)),
            Inline::Emphasis { strong, children } => {
                let (content, lost) = inline_text(children);
                let mark = if *strong { "'''" } else { "''" };
                out.push_str(mark);
                out.push_str(&content);
                out.push_str(mark);
                loss |= lost;
            }
            Inline::Html(html)
                if html.eq_ignore_ascii_case("<br>") || html.eq_ignore_ascii_case("<br />") =>
            {
                out.push_str("[br]")
            }
            Inline::Html(html) if html.starts_with("<code>") && html.ends_with("</code>") => {
                let content = html_escape::decode_html_entities(&html[6..html.len() - 7]);
                if content.contains("}}}")
                    || content.ends_with('}')
                    || content
                        .bytes()
                        .rev()
                        .take_while(|&byte| byte == b'\\')
                        .count()
                        % 2
                        == 1
                    || content.contains('\n')
                    || content.starts_with('#')
                {
                    out.push_str(&escape(&content));
                    loss = true;
                } else {
                    out.push_str("{{{");
                    out.push_str(&content);
                    out.push_str("}}}");
                }
            }
            Inline::Html(html) => {
                out.push_str(&escape(html));
                loss = true;
            }
            Inline::Link {
                href,
                label,
                internal,
            } => {
                let (label, lost) = inline_text(label);
                loss |= lost;
                let decoded = if *internal {
                    inline::decode_component(href.strip_prefix("./").unwrap_or(href))
                        .unwrap_or_else(|_| href.clone())
                } else {
                    href.clone()
                };
                let target = if let Some(name) = decoded.strip_prefix("Category:") {
                    format!(":분류:{name}")
                } else {
                    decoded
                };
                let valid_external = target
                    .get(..7)
                    .is_some_and(|s| s.eq_ignore_ascii_case("http://"))
                    || target
                        .get(..8)
                        .is_some_and(|s| s.eq_ignore_ascii_case("https://"));
                if target.is_empty()
                    || target
                        .chars()
                        .any(|c| c.is_control() || matches!(c, '<' | '>' | '"'))
                    || (!internal
                        && (!valid_external
                            || target
                                .chars()
                                .any(|c| c.is_whitespace() || matches!(c, '[' | ']' | '\\'))))
                {
                    out.push_str(&label);
                    loss = true;
                } else {
                    out.push_str(&format!("[[{}|{label}]]", escape(&target)));
                }
            }
        }
    }
    (out, loss)
}

pub(crate) fn render_block(block: &Block) -> (String, bool) {
    match block {
        Block::Paragraph(parts) => inline_text(parts),
        Block::Heading { level, text } => {
            let (text, loss) = inline_text(text);
            let marker = "=".repeat(*level as usize);
            (format!("{marker} {text} {marker}"), loss)
        }
        Block::Core(core::Block::Rule) => ("----".into(), false),
        Block::Core(core::Block::RuleWithText(text)) => {
            let (text, _) = inline_text(text);
            (format!("----\n\n{text}"), true)
        }
        Block::Core(core::Block::Pre(text)) => pre(text),
        Block::Core(core::Block::WikiPre(parts)) => {
            let text = inline::plain(parts);
            let (rendered, loss) = pre(&text);
            (
                rendered,
                loss || parts.iter().any(|p| !matches!(p, Inline::Text(_))),
            )
        }
        Block::Core(core::Block::List(lines)) => {
            let mut loss = false;
            let mut previous = None;
            let mut out = Vec::new();
            for line in lines {
                let marker = match line.markers.as_str() {
                    "*" => " * ",
                    "#" => " 1. ",
                    _ => {
                        loss = true;
                        " * "
                    }
                };
                if previous.is_some_and(|p| p != marker) {
                    loss = true;
                }
                previous = Some(marker);
                let (text, lost) = inline_text(&line.content);
                loss |= lost;
                out.push(format!("{marker}{text}"));
            }
            (out.join("\n"), loss)
        }
        Block::Table(table) => (
            escape(&table.render(Format::Html).unwrap_or_default()),
            true,
        ),
        Block::Html(html) => (escape(html), true),
    }
}

fn pre(text: &str) -> (String, bool) {
    if text.contains("}}}") {
        (escape(text), true)
    } else {
        (format!("{{{{{{\n{text}\n}}}}}}"), false)
    }
}

pub(crate) fn render_metadata(metadata: &PageMetadata) -> (String, bool) {
    let mut parts = Vec::new();
    let mut loss = metadata.display_title.is_some();
    if let Some(target) = &metadata.redirect_target {
        if target.contains(['\n', '\r']) {
            loss = true;
        } else {
            parts.push(format!("#redirect {target}"));
        }
    }
    for (i, category) in metadata.categories.iter().enumerate() {
        let sort = metadata
            .category_sort_keys
            .get(i)
            .and_then(|s| s.as_deref());
        if category.contains(['[', ']', '|', '\n', '\r'])
            || sort.is_some_and(|s| s.contains([']', '\n', '\r']))
        {
            loss = true;
            parts.push(escape(&format!("[[분류:{category}]]")));
        } else {
            parts.push(format!(
                "[[분류:{}{}]]",
                escape(category),
                sort.map(|s| format!("|{}", escape(s))).unwrap_or_default()
            ));
        }
    }
    (parts.join("\n"), loss)
}
