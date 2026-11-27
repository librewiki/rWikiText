use super::{
    inline::InlineParser,
    scan::{find_unescaped, unescape},
};
use crate::{html_policy, inline, Format, Inline};
use std::ops::Range;

impl InlineParser<'_, '_> {
    /// Parse a `[[...]]` NamuMark link starting at `rest`, returning the number of
    /// consumed bytes or `None` when the link is unclosed and the rest of the
    /// input has been emitted as literal text.
    pub(super) fn parse_link(
        &mut self,
        rest: &str,
        span_start: usize,
        depth: usize,
        out: &mut Vec<Inline>,
    ) -> Option<usize> {
        let body = rest.strip_prefix("[[")?;
        let Some(end) = find_unescaped(body, "]]") else {
            self.push_loss(
                span_start..span_start + rest.len(),
                "unclosed NamuMark link",
            );
            inline::text(out, rest);
            return None;
        };
        let consumed = end + 4;
        let span = span_start..span_start + consumed;
        let inside = &body[..end];
        let (raw_title, label) = find_unescaped(inside, "|")
            .map_or((inside, inside), |n| (&inside[..n], &inside[n + 1..]));
        let title_value = unescape(raw_title);
        let title = title_value.as_str();
        let external = html_policy::has_http_scheme(title);
        let bad = title.is_empty()
            || title
                .chars()
                .any(|c| c.is_control() || matches!(c, '<' | '>' | '"'))
            || (external && title.chars().any(|c| matches!(c, '[' | ']' | '\\')))
            || (!external
                && (find_unescaped(raw_title, "[").is_some()
                    || find_unescaped(raw_title, "]").is_some()
                    || title.contains("://")
                    || title.starts_with("//")
                    || title.split_once(':').is_some_and(|(prefix, _)| {
                        [
                            "javascript",
                            "data",
                            "mailto",
                            "ftp",
                            "file",
                            "vbscript",
                            "tel",
                        ]
                        .iter()
                        .any(|s| prefix.eq_ignore_ascii_case(s))
                    })));
        if bad
            || (external
                && (title.chars().any(char::is_whitespace)
                    || title
                        .split_once("://")
                        .is_none_or(|(_, h)| h.is_empty() || h.starts_with(['/', '?', '#']))))
        {
            self.push_loss(span, "unsupported or unsafe NamuMark link");
            inline::text(out, &rest[..consumed]);
            return Some(consumed);
        }
        // A link target is "page#fragment"; only an unescaped '#' starts a
        // fragment (docs/namumark.md: never reinterpret escaped syntax), so
        // split on raw_title before unescaping and keep page/fragment separate
        // through relative resolution and URL encoding.
        let (title_page, title_fragment) = if external {
            (title.to_owned(), None)
        } else {
            match find_unescaped(raw_title, "#") {
                Some(hash) => (
                    unescape(&raw_title[..hash]),
                    Some(unescape(&raw_title[hash + 1..])),
                ),
                None => (title.to_owned(), None),
            }
        };
        if !external
            && title_fragment
                .as_deref()
                .is_some_and(is_automatic_section_anchor)
        {
            self.push_loss(span, "unsupported NamuMark automatic section anchor");
            inline::text(out, &rest[..consumed]);
            return Some(consumed);
        }
        if self.target() == Format::Wikitext
            && !external
            && (title
                .chars()
                .any(|c| matches!(c, '[' | ']' | '{' | '}' | '|'))
                || title_page.contains('#'))
        {
            self.push_loss(span, "MediaWiki cannot represent the NamuMark link title");
            inline::text(out, &rest[..consumed]);
            return Some(consumed);
        }
        let resolved_page = self.resolve_relative_page(&title_page, span.clone());
        let resolved_page = if self.target() == Format::Wikitext {
            resolved_page
                .strip_prefix(":분류:")
                .map_or(resolved_page.clone(), |name| format!(":Category:{name}"))
        } else {
            resolved_page
        };
        let href = if external {
            resolved_page
        } else if resolved_page.is_empty() {
            match title_fragment.as_deref() {
                Some(fragment) => format!("#{}", inline::encode_component(fragment)),
                None => String::new(),
            }
        } else {
            let page = resolved_page.strip_prefix(':').unwrap_or(&resolved_page);
            match title_fragment.as_deref() {
                Some(fragment) => format!(
                    "./{}#{}",
                    inline::encode_component(page),
                    inline::encode_component(fragment)
                ),
                None => format!("./{}", inline::encode_component(page)),
            }
        };
        if self.target() == Format::Wikitext && !external && changes_wiki_title(title) {
            self.push_loss(
                span,
                "MediaWiki title normalization changes the NamuMark title",
            );
        }
        out.push(Inline::Link {
            href,
            label: self.parse(
                label,
                span_start + 2 + inside.len() - label.len(),
                depth + 1,
            ),
            internal: !external,
        });
        Some(consumed)
    }

    /// Resolve `/` (subpage) and `../` (parent-relative) NamuMark link targets
    /// against `self.page_title()`, recording a single `ConversionLoss`
    /// diagnostic and keeping the raw target when `page_title` is empty.
    fn resolve_relative_page(&mut self, title_page: &str, span: Range<usize>) -> String {
        let page_title = self.page_title();
        let (base_page, child) = if let Some(child) = title_page.strip_prefix('/') {
            (page_title, child)
        } else if let Some(child) = title_page.strip_prefix("../") {
            let parent = page_title.rsplit_once('/').map_or(page_title, |(p, _)| p);
            (parent, child)
        } else {
            return title_page.to_owned();
        };
        if page_title.is_empty() {
            self.push_loss(span, "relative link requires page title");
            return title_page.to_owned();
        }
        if child.is_empty() {
            base_page.to_owned()
        } else {
            format!("{base_page}/{child}")
        }
    }
}

/// Automatic NamuMark section anchors look like `s-1`, `s-2`, or dotted forms
/// such as `s-1.3`; each dot-separated part must be non-empty ASCII digits.
fn is_automatic_section_anchor(fragment: &str) -> bool {
    fragment.strip_prefix("s-").is_some_and(|rest| {
        rest.split('.')
            .all(|part| !part.is_empty() && part.bytes().all(|b| b.is_ascii_digit()))
    })
}

pub(crate) fn internal_transport(href: &str) -> bool {
    if !(href.starts_with("./") || href.starts_with('#'))
        || href.chars().any(|c| {
            c.is_control() || c.is_whitespace() || matches!(c, '<' | '>' | '"' | '\\' | '[' | ']')
        })
    {
        return false;
    }
    inline::decode_component(href).is_ok_and(|decoded| !decoded.chars().any(char::is_control))
}

pub(super) fn changes_wiki_title(title: &str) -> bool {
    title.contains('_') || title.starts_with(|c: char| c.is_lowercase())
}
