use super::{
    emphasis::{apostrophe_run, parse_emphasis},
    links::{changes_wiki_title, parse_link},
    scan::{find_unescaped, unescape},
};
use crate::{inline, metadata, Diagnostic, Format, Inline, PageMetadata, WikiContext};

pub(super) struct InlineParser<'a, 'ctx> {
    pub(super) context: &'a WikiContext<'ctx>,
    pub(super) target: Format,
    pub(super) diagnostics: &'a mut Vec<Diagnostic>,
    pub(super) metadata: &'a mut PageMetadata,
}

pub(super) fn parse_inlines(
    input: &str,
    base: usize,
    context: &WikiContext<'_>,
    target: Format,
    diagnostics: &mut Vec<Diagnostic>,
    metadata: &mut PageMetadata,
) -> Vec<Inline> {
    InlineParser {
        context,
        target,
        diagnostics,
        metadata,
    }
    .parse(input, base, 0)
}

impl InlineParser<'_, '_> {
    pub(super) fn parse(&mut self, input: &str, base: usize, depth: usize) -> Vec<Inline> {
        if depth > 32 {
            self.diagnostics.push(Diagnostic::loss(
                base..base + input.len(),
                "NamuMark nesting limit",
            ));
            return vec![Inline::Text(input.into())];
        }
        let mut out = Vec::new();
        let mut i = 0;
        while i < input.len() {
            let rest = &input[i..];
            if let Some(escaped) = rest.strip_prefix('\\').and_then(|s| s.chars().next()) {
                inline::text(&mut out, &escaped.to_string());
                i += 1 + escaped.len_utf8();
                continue;
            }
            if rest
                .get(..4)
                .is_some_and(|piece| piece.eq_ignore_ascii_case("[br]"))
            {
                out.push(Inline::Html("<br>".into()));
                i += 4;
                continue;
            }
            if let Some(body) = rest.strip_prefix("{{{") {
                let end = find_unescaped(body, "}}}").map(|n| n + 3);
                if let Some(end) = end {
                    let literal = &rest[3..end];
                    if !literal.starts_with('#') && !literal.starts_with("#!") {
                        out.push(Inline::Html(format!(
                            "<code>{}</code>",
                            html_escape::encode_text(literal)
                        )));
                        i += end + 3;
                        continue;
                    }
                }
                let end = end.map_or(rest.len(), |n| n + 3);
                self.diagnostics.push(Diagnostic::loss(
                    base + i..base + i + end,
                    "unsupported or malformed NamuMark literal directive",
                ));
                inline::text(&mut out, &rest[..end]);
                i += end;
                continue;
            }
            if let Some(body) = rest.strip_prefix("[[분류:") {
                if let Some(end) = find_unescaped(body, "]]") {
                    let inside = &body[..end];
                    let (name, sort) = find_unescaped(inside, "|").map_or((inside, None), |n| {
                        (&inside[..n], Some(unescape(&inside[n + 1..])))
                    });
                    let full = "[[분류:".len() + end + 2;
                    if !name.is_empty() && !name.contains("[[") && !name.contains("#blur") {
                        let name = unescape(name);
                        if self.target == Format::Wikitext
                            && (!metadata::valid_title(&name) || changes_wiki_title(&name))
                        {
                            self.diagnostics.push(Diagnostic::loss(
                                base + i..base + i + full,
                                "MediaWiki cannot preserve this category title",
                            ));
                        }
                        if self.target == Format::Html
                            && (!metadata::valid_title(&name)
                                || sort
                                    .as_deref()
                                    .is_some_and(|s| !metadata::valid_sort_key(s)))
                        {
                            self.diagnostics.push(Diagnostic::loss(
                                base + i..base + i + full,
                                "HTML metadata transport cannot preserve this category",
                            ));
                        }
                        self.metadata.categories.push(name);
                        self.metadata.category_sort_keys.push(sort);
                        if self.target == Format::Gfm {
                            self.diagnostics.push(Diagnostic::loss(
                                base + i..base + i + full,
                                "saving the GFM body alone loses the category",
                            ));
                        }
                        i += full;
                        continue;
                    }
                    self.diagnostics.push(Diagnostic::loss(
                        base + i..base + i + full,
                        "unsupported NamuMark category",
                    ));
                    inline::text(&mut out, &rest[..full]);
                    i += full;
                    continue;
                }
            }
            if rest.starts_with("[[") {
                match parse_link(self, rest, base + i, depth, &mut out) {
                    Some(consumed) => {
                        i += consumed;
                        continue;
                    }
                    None => break,
                }
            }
            if apostrophe_run(rest) >= 2 {
                match parse_emphasis(self, rest, base + i, depth) {
                    Ok((nodes, consumed)) => {
                        out.extend(nodes);
                        i += consumed;
                    }
                    Err(()) => {
                        self.diagnostics.push(Diagnostic::loss(
                            base + i..base + input.len(),
                            "unclosed NamuMark emphasis",
                        ));
                        inline::text(&mut out, rest);
                        break;
                    }
                }
                continue;
            }
            let excluded = ["__", "~~", "--", "^^", ",,"]
                .into_iter()
                .find(|marker| rest.starts_with(marker) && rest[marker.len()..].contains(marker));
            if let Some(marker) = excluded {
                let end = marker.len() + rest[marker.len()..].find(marker).unwrap() + marker.len();
                self.diagnostics.push(Diagnostic::loss(
                    base + i..base + i + end,
                    "unsupported NamuMark formatting",
                ));
                inline::text(&mut out, &rest[..end]);
                i += end;
                continue;
            }
            if rest.starts_with('[') && !rest.starts_with("[[") {
                if let Some(offset) = find_unescaped(&rest[1..], "]") {
                    let inner = &rest[1..1 + offset];
                    if is_namumark_footnote(inner) || is_namumark_macro(inner) {
                        let end = offset + 2;
                        self.diagnostics.push(Diagnostic::loss(
                            base + i..base + i + end,
                            "unsupported NamuMark macro or footnote",
                        ));
                        inline::text(&mut out, &rest[..end]);
                        i += end;
                        continue;
                    }
                }
                // Ordinary bracketed text falls through and emits only '[' via the
                // default path, so prose like [1] or [sic] is never diagnosed.
            }
            let c = rest.chars().next().unwrap();
            inline::text(&mut out, &c.to_string());
            i += c.len_utf8();
        }
        out
    }
}

/// NamuMark macros that take no arguments.
const NAMUMARK_MACROS: &[&str] = &[
    "br",
    "date",
    "datetime",
    "tableofcontents",
    "목차",
    "footnote",
    "각주",
    "clearfix",
    "pagecount",
];

/// NamuMark macros that only count as macros when parentheses are present
/// (the argument-carrying `[name(...)]` form).
const NAMUMARK_MACROS_WITH_ARGUMENTS: &[&str] = &[
    "include",
    "age",
    "dday",
    "youtube",
    "nicovideo",
    "vimeo",
    "navertv",
    "ruby",
    "anchor",
];

fn is_namumark_macro(inner: &str) -> bool {
    if let Some((name, args)) = inner.split_once('(') {
        if !args.ends_with(')') {
            return false;
        }
        return NAMUMARK_MACROS_WITH_ARGUMENTS
            .iter()
            .chain(NAMUMARK_MACROS.iter())
            .any(|macro_name| name.eq_ignore_ascii_case(macro_name));
    }
    NAMUMARK_MACROS
        .iter()
        .any(|macro_name| inner.eq_ignore_ascii_case(macro_name))
}

fn is_namumark_footnote(inner: &str) -> bool {
    inner.starts_with('*')
}
