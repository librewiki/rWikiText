use crate::{html::html_to_wiki, html_policy, inline, Inline};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum HtmlStyle {
    /// MediaWiki / Parsoid DOM conventions (`<b>` / `<i>`).
    Parsoid,
    /// Semantic HTML conventions for NamuMark baseline (`<strong>` / `<em>`).
    Semantic,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum RenderTarget {
    Wikitext,
    Gfm,
    Html(HtmlStyle),
}

impl RenderTarget {
    pub(crate) fn html_style(self) -> HtmlStyle {
        match self {
            Self::Html(style) => style,
            Self::Wikitext | Self::Gfm => HtmlStyle::Parsoid,
        }
    }
}

pub(crate) fn heading_id(text: &[Inline], ids: &mut std::collections::HashSet<String>) -> String {
    let base = inline::normalize_fragment(&inline::plain(text));
    let mut id = base.clone();
    let mut suffix = 2;
    while !ids.insert(id.to_lowercase()) {
        id = format!("{base}_{suffix}");
        suffix += 1;
    }
    id
}

pub(crate) fn render_inline(content: &[Inline], target: RenderTarget) -> String {
    let mut output = String::new();
    let mut previous_wikilink = false;
    for part in content {
        match part {
            Inline::Html(html) => match target {
                RenderTarget::Wikitext => output.push_str(&html_to_wiki(html)),
                RenderTarget::Gfm | RenderTarget::Html(_) => output.push_str(html),
            },
            Inline::Emphasis { strong, children } => {
                let (open, close) = match (target, strong) {
                    (RenderTarget::Html(HtmlStyle::Parsoid), true) => ("<b>", "</b>"),
                    (RenderTarget::Html(HtmlStyle::Parsoid), false) => ("<i>", "</i>"),
                    (RenderTarget::Html(HtmlStyle::Semantic), true) => ("<strong>", "</strong>"),
                    (RenderTarget::Html(HtmlStyle::Semantic), false) => ("<em>", "</em>"),
                    (RenderTarget::Wikitext, true) => ("'''", "'''"),
                    (RenderTarget::Wikitext, false) => ("''", "''"),
                    (RenderTarget::Gfm, true) => ("**", "**"),
                    (RenderTarget::Gfm, false) => ("*", "*"),
                };
                output.push_str(open);
                output.push_str(&render_inline(children, target));
                output.push_str(close);
            }
            Inline::Text(text) => match target {
                RenderTarget::Wikitext => {
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
                RenderTarget::Gfm => output.push_str(&escape_ascii_punctuation(text)),
                RenderTarget::Html(_) => output.push_str(&html_escape::encode_text(text)),
            },
            Inline::Link {
                href,
                label,
                internal,
            } => match target {
                RenderTarget::Wikitext if *internal => {
                    let target_title = inline::wiki_target(href).expect("validated wiki href");
                    let colon = if target_title.split_once(':').is_some_and(|(namespace, _)| {
                        ["file", "image", "category", "media"]
                            .iter()
                            .any(|name| namespace.eq_ignore_ascii_case(name))
                    }) {
                        ":"
                    } else {
                        ""
                    };
                    output.push_str(&format!(
                        "[[{colon}{}|{}]]",
                        target_title.replace('&', "&amp;"),
                        render_inline(label, target)
                    ));
                }
                RenderTarget::Wikitext => output.push_str(&format!(
                    "[{} {}]",
                    html_policy::escape_wiki_attribute_value(href),
                    render_inline(label, target)
                )),
                RenderTarget::Gfm => output.push_str(&format!(
                    "[{}](<{}>)",
                    render_inline(label, target),
                    href.replace('&', "&amp;")
                )),
                RenderTarget::Html(_) => output.push_str(&format!(
                    "<a rel=\"{}\" href=\"{}\">{}</a>",
                    if *internal {
                        "mw:WikiLink"
                    } else {
                        "mw:ExtLink"
                    },
                    html_escape::encode_double_quoted_attribute(href),
                    render_inline(label, target)
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

pub(crate) fn escape_wikitext(text: &str) -> String {
    escape_wikitext_at(text, true)
}

pub(crate) fn escape_wikitext_at(text: &str, mut line_start: bool) -> String {
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

pub(crate) fn escape_ascii_punctuation(text: &str) -> String {
    let mut output = String::new();
    for ch in text.chars() {
        if ch.is_ascii_punctuation() {
            output.push('\\');
        }
        output.push(ch);
    }
    output
}
