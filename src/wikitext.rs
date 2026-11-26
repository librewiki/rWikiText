use std::ops::Range;

use crate::{core, html_policy, inline, spans, table, wiki_markup, Block, Diagnostic, Document, Inline};

pub(crate) fn wiki_table_ranges(input: &str) -> Vec<Range<usize>> {
    let mut ranges = Vec::new();
    let mut depth = 0usize;
    let mut start = 0;
    let mut pos = 0;
    while pos < input.len() {
        if depth == 0 {
            if let Some((name, length)) = html_policy::wiki_fragment(&input[pos..]) {
                if html_policy::is_block(&name) {
                    let end = pos + length;
                    pos = input[end..].find('\n').map_or(input.len(), |i| end + i + 1);
                    continue;
                }
            }
        }
        let length = input[pos..].find('\n').map_or(input.len() - pos, |i| i + 1);
        let line = &input[pos..pos + length];
        let trimmed = line.trim_start();
        if (depth == 0 && line.starts_with("{|")) || (depth > 0 && trimmed.starts_with("{|")) {
            if depth == 0 {
                start = pos;
            }
            depth += 1;
        } else if trimmed.starts_with("|}") && depth > 0 {
            depth -= 1;
            if depth == 0 {
                ranges.push(start..pos + line.len());
            }
        }
        pos += length;
    }
    if depth > 0 {
        ranges.push(start..input.len());
    }
    ranges
}

pub(crate) fn table_delimiter(input: &str, span: &Range<usize>) -> bool {
    let start = input[..span.start].rfind('\n').map_or(0, |i| i + 1);
    let end = input[span.end..]
        .find('\n')
        .map_or(input.len(), |i| span.end + i);
    let line = &input[start..end];
    let fragment = &input[span.clone()];
    if line.starts_with("{|") || line.starts_with("|-") {
        return true;
    }
    if line.starts_with('|') || line.starts_with('!') {
        return fragment.contains('|') || line[span.end - start..].contains('|');
    }
    false
}

pub(crate) fn heading_delimiter(input: &str, span: &Range<usize>) -> bool {
    let start = input[..span.start].rfind('\n').map_or(0, |i| i + 1);
    let end = input[span.end..]
        .find('\n')
        .map_or(input.len(), |i| span.end + i);
    let line = input[start..end].trim_end();
    let Some((level, _)) = heading(line) else {
        return false;
    };
    let level = usize::from(level);
    span.start < start + level || span.end > start + line.len() - level
}

pub(crate) fn is_line_aligned(input: &str, span: &Range<usize>) -> bool {
    input[..span.start]
        .rsplit('\n')
        .next()
        .unwrap_or("")
        .trim()
        .is_empty()
        && input[span.end..]
            .split('\n')
            .next()
            .unwrap_or("")
            .trim()
            .is_empty()
}

pub(crate) fn looks_like_block(input: &str) -> bool {
    let first = input
        .trim_start_matches(['\n', '\r'])
        .split('\n')
        .next()
        .unwrap_or("");
    first.starts_with(['*', '#', ';', ':', ' '])
        || first.starts_with("{|")
        || first.starts_with("----")
        || heading(first).is_some()
        || input.contains("\n\n")
        || html_policy::tag_info(first).is_some_and(|(name, _, _)| html_policy::is_block(&name))
}

pub(crate) fn continues_wiki_block(input: &str, span: &Range<usize>) -> bool {
    let group = |line: &str| match line.as_bytes().first() {
        Some(b'*' | b'#' | b':' | b';') => Some("list"),
        Some(b' ') => Some("pre"),
        _ => None,
    };
    let fragment = input[span.clone()].trim_matches(['\r', '\n']);
    let first = fragment.split('\n').next().unwrap_or("");
    let last = fragment.rsplit('\n').next().unwrap_or("");
    let before = input[..span.start].rsplit('\n').nth(1).unwrap_or("");
    let after = input[span.end..].split('\n').nth(1).unwrap_or("");
    group(first).is_some_and(|value| group(before) == Some(value))
        || group(last).is_some_and(|value| group(after) == Some(value))
}

pub(crate) fn inside_wiki_token(input: &str, at: usize) -> bool {
    let prefix = input[..at].rsplit('\n').next().unwrap_or("");
    let delimited = |open, close| {
        prefix
            .rfind(open)
            .is_some_and(|start| !prefix[start..].contains(close))
    };
    delimited("[[", "]]") || delimited("<", ">")
}

pub(crate) fn parse_wikitext(
    input: &str,
    language: &str,
    source_span: &impl Fn(Range<usize>) -> Range<usize>,
    diagnostics: &mut Vec<Diagnostic>,
) -> Document {
    let removed_crs: Vec<_> = input
        .match_indices("\r\n")
        .map(|(pos, _)| pos..pos + 1)
        .collect();
    let normalized = input.replace("\r\n", "\n").replace('\r', "\n");
    let mut lines: Vec<_> = normalized.split('\n').collect();
    let location = |part: &str, length| {
        let start = part.as_ptr() as usize - normalized.as_ptr() as usize;
        source_span(spans::original_range(start..start + length, &removed_crs))
    };
    let mut blocks = Vec::new();
    let mut pos = 0;
    while pos < lines.len() {
        let line = lines[pos];
        let remainder = &normalized[line.as_ptr() as usize - normalized.as_ptr() as usize..];
        if let Some((name, end)) = html_policy::wiki_fragment(remainder) {
            if html_policy::is_block(&name) {
                let html = &remainder[..end];
                blocks.push(Block::Html(html.to_owned()));
                pos += html.bytes().filter(|&b| b == b'\n').count();
                let rest = remainder[end..]
                    .split('\n')
                    .next()
                    .unwrap_or("")
                    .trim_start_matches([' ', '\t']);
                if rest.is_empty() {
                    pos += 1;
                } else {
                    lines[pos] = rest;
                }
                continue;
            }
        }
        if line.starts_with("{|") {
            let mut depth = 0;
            let length = lines[pos..]
                .iter()
                .position(|line| {
                    let line = line.trim_start();
                    if line.starts_with("{|") {
                        depth += 1;
                    } else if line.starts_with("|}") {
                        depth -= 1;
                        return depth == 0;
                    }
                    false
                })
                .map_or(lines.len() - pos, |end| end + 1);
            let table_source = lines[pos..pos + length].join("\n");
            let span = location(line, table_source.len());
            match table::Table::parse_wiki(&table_source, language) {
                Ok(table) => {
                    diagnostics.extend(table.wiki_import_losses().iter().map(|diagnostic| {
                        Diagnostic::new(diagnostic.kind, span.clone(), &diagnostic.message)
                    }));
                    blocks.push(Block::Table(table));
                }
                Err(error) => {
                    diagnostics.push(Diagnostic::loss(span, format!("table omitted: {error}")))
                }
            }
            pos += length;
            continue;
        }
        if line.trim_ascii().is_empty() {
            pos += 1;
            continue;
        }
        match core::parse_wiki(&lines[pos..], language) {
            Ok(Some((block, length))) => {
                blocks.push(Block::Core(block));
                pos += length;
                continue;
            }
            Ok(None) => {}
            Err(error) => {
                diagnostics.push(Diagnostic::loss(
                    location(line, line.len()),
                    error.to_string(),
                ));
                blocks.push(Block::Paragraph(wiki_markup::literal_inlines(line)));
                pos += 1;
                continue;
            }
        }
        if let Some((level, text)) = heading(line) {
            blocks.push(Block::Heading {
                level,
                text: parse_wiki_inline(text, language, location(text, text.len()), diagnostics),
            });
            pos += 1;
            continue;
        }
        let end = lines[pos + 1..]
            .iter()
            .position(|line| {
                line.trim_ascii().is_empty()
                    || heading(line).is_some()
                    || line.starts_with(['*', '#', ':', ';', ' '])
                    || line.starts_with("----")
                    || line.starts_with("{|")
                    || html_policy::wiki_fragment(line)
                        .is_some_and(|(name, _)| html_policy::is_block(&name))
            })
            .map(|offset| pos + 1 + offset)
            .unwrap_or(lines.len());
        let paragraph = lines[pos..end].join("\n");
        blocks.push(Block::Paragraph(parse_wiki_inline(
            &paragraph,
            language,
            location(line, paragraph.len()),
            diagnostics,
        )));
        pos = end;
    }
    Document { blocks }
}

fn parse_wiki_inline(
    input: &str,
    language: &str,
    location: Range<usize>,
    diagnostics: &mut Vec<Diagnostic>,
) -> Vec<Inline> {
    match inline::parse_wiki(input, language) {
        Ok(inlines) => inlines,
        Err(error) => {
            diagnostics.push(Diagnostic::loss(location, error.to_string()));
            wiki_markup::literal_inlines(input)
        }
    }
}

fn heading(line: &str) -> Option<(u8, &str)> {
    let line = line.trim_end();
    let start = line.bytes().take_while(|&c| c == b'=').count();
    let end = line.bytes().rev().take_while(|&c| c == b'=').count();
    let level = if start == line.len() {
        start.saturating_sub(1) / 2
    } else {
        start.min(end)
    }
    .min(6);
    (level > 0).then(|| (level as u8, line[level..line.len() - level].trim()))
}
