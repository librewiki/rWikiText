use crate::{inline, transclusion, Diagnostic, Format, Inline};
use std::collections::HashMap;
use std::ops::Range;

#[derive(Default)]
pub(crate) struct Markup {
    changes: Vec<(Range<usize>, String)>,
    replacements: Vec<(String, String)>,
    groups: Vec<Group>,
}

struct Group {
    open: String,
    close: String,
    block: bool,
    mw: String,
    parsoid: String,
    source: Range<usize>,
}

impl Markup {
    fn token(&self, block: bool) -> String {
        let tag = if block { "div" } else { "span" };
        format!(
            "<{tag} data-rwikitext-token=\"{}\"></{tag}>",
            self.changes.len()
        )
    }

    pub fn replace(&mut self, range: Range<usize>, value: String, block: bool, isolate: bool) {
        let token = self.token(block);
        let marker = if isolate {
            format!("\n{token}\n")
        } else {
            token.clone()
        };
        self.changes.push((range, marker));
        self.replacements.push((token, value));
    }

    pub fn group(
        &mut self,
        range: Range<usize>,
        source: Range<usize>,
        block: bool,
        mw: String,
        parsoid: String,
    ) {
        let open = self.token(block);
        let start = if block {
            format!("{open}\n")
        } else {
            open.clone()
        };
        self.changes.push((range.start..range.start, start));
        let close = self.token(block);
        let end = if block {
            format!("\n{close}")
        } else {
            close.clone()
        };
        self.changes.push((range.end..range.end, end));
        self.groups.push(Group {
            open,
            close,
            block,
            mw,
            parsoid,
            source,
        });
    }

    pub fn apply(&mut self, input: &str) -> MarkedText {
        self.changes
            .sort_by_key(|(range, _)| (range.start, !range.is_empty()));
        let mut result = MarkedText {
            text: String::new(),
            changes: Vec::new(),
        };
        let mut pos = 0;
        for (range, value) in &self.changes {
            if range.start < pos {
                continue;
            }
            result.text.push_str(&input[pos..range.start]);
            let start = result.text.len();
            result.text.push_str(value);
            result
                .changes
                .push((range.clone(), start..result.text.len()));
            pos = range.end;
        }
        result.text.push_str(&input[pos..]);
        result
    }

    pub fn render(self, mut output: String, diagnostics: &mut Vec<Diagnostic>) -> String {
        if !self.replacements.is_empty() {
            let replacements: HashMap<_, _> = self.replacements.into_iter().collect();
            let mut result = String::new();
            let mut previous = 0;
            for range in tokens(&output) {
                result.push_str(&output[previous..range.start]);
                let original = &output[range.clone()];
                result.push_str(replacements.get(original).map_or(original, String::as_str));
                previous = range.end;
            }
            result.push_str(&output[previous..]);
            output = result;
        }
        if self.groups.is_empty() {
            return output;
        }
        let locations: HashMap<_, _> = tokens(&output)
            .map(|range| (&output[range.clone()], range))
            .collect();
        let mut edits = Vec::new();
        for (index, group) in self.groups.into_iter().enumerate() {
            let open = locations.get(group.open.as_str());
            let close = locations.get(group.close.as_str());
            if let (Some(open), Some(close)) = (open, close) {
                if let Some(fragment) = output.get(open.end..close.start) {
                    if let Some(annotated) = transclusion::annotate(
                        fragment,
                        index + 1,
                        group.block,
                        &group.mw,
                        &group.parsoid,
                    ) {
                        edits.push((open.start..close.end, annotated));
                        continue;
                    }
                }
            }
            edits.extend(
                open.into_iter()
                    .chain(close)
                    .map(|range| (range.clone(), String::new())),
            );
            diagnostics.push(Diagnostic::loss(
                group.source,
                "cannot attach Parsoid metadata across HTML elements",
            ));
        }
        edits.sort_by_key(|(range, _)| range.start);
        let mut result = String::new();
        let mut position = 0;
        for (range, value) in edits {
            if range.start >= position {
                result.push_str(&output[position..range.start]);
                result.push_str(&value);
                position = range.end;
            }
        }
        result.push_str(&output[position..]);
        result
    }
}

fn tokens(output: &str) -> impl Iterator<Item = Range<usize>> + '_ {
    const ATTRIBUTE: &str = " data-rwikitext-token=\"";
    output.match_indices(ATTRIBUTE).filter_map(|(index, _)| {
        let (start, end_tag) = if output[..index].ends_with("<span") {
            (index - 5, "\"></span>")
        } else if output[..index].ends_with("<div") {
            (index - 4, "\"></div>")
        } else {
            return None;
        };
        let rest = &output[index + ATTRIBUTE.len()..];
        let digits = rest.bytes().take_while(u8::is_ascii_digit).count();
        (digits != 0 && rest[digits..].starts_with(end_tag))
            .then_some(start..index + ATTRIBUTE.len() + digits + end_tag.len())
    })
}

pub(crate) fn literal_inlines(input: &str) -> Vec<Inline> {
    let mut content = Vec::new();
    let mut position = 0;
    for token in tokens(input) {
        inline::text(&mut content, &input[position..token.start]);
        position = token.end;
        content.push(Inline::Html(input[token].into()));
    }
    inline::text(&mut content, &input[position..]);
    content
}

pub(crate) struct MarkedText {
    pub text: String,
    changes: Vec<(Range<usize>, Range<usize>)>,
}

impl MarkedText {
    pub fn original_span(&self, span: Range<usize>) -> Range<usize> {
        let boundary = |offset, end| {
            let mut mapped = offset;
            for (source, marked) in &self.changes {
                if offset < marked.start {
                    break;
                }
                if offset < marked.end {
                    return if end { source.end } else { source.start };
                }
                mapped = source.end + (offset - marked.end);
            }
            mapped
        };
        boundary(span.start, false)..boundary(span.end, true)
    }
}

pub(crate) fn redlink(title: &str, format: Format) -> String {
    let href = format!("./{}", inline::encode_component(title));
    let title = title.replace('_', " ");
    if format == Format::Gfm {
        return format!("[{}](<{}>)", crate::escape_gfm(&title), href);
    }
    format!("<a rel=\"mw:WikiLink\" href=\"{}?action=edit&amp;redlink=1\" title=\"{}\" class=\"new\" typeof=\"mw:LocalizedAttrs\">{}</a>",
        html_escape::encode_double_quoted_attribute(&href),
        html_escape::encode_double_quoted_attribute(&title),
        html_escape::encode_text(&title))
}
