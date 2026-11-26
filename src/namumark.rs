use crate::{core, inline, Block, Diagnostic, Document, Format, Inline, PageMetadata, WikiContext};

pub(crate) fn parse(
    input: &str,
    context: &WikiContext<'_>,
    target: Format,
    diagnostics: &mut Vec<Diagnostic>,
) -> (Document, PageMetadata) {
    let mut blocks = Vec::new();
    let mut metadata = PageMetadata::default();
    let mut paragraph = Vec::new();
    let mut list: Vec<core::ListLine> = Vec::new();
    let mut offset = 0;
    let flush =
        |blocks: &mut Vec<Block>, paragraph: &mut Vec<Inline>, list: &mut Vec<core::ListLine>| {
            if !paragraph.is_empty() {
                blocks.push(Block::Paragraph(std::mem::take(paragraph)));
            }
            if !list.is_empty() {
                blocks.push(Block::Core(core::Block::List(std::mem::take(list))));
            }
        };
    let mut lines = input.split_inclusive('\n').peekable();
    while let Some(raw) = lines.next() {
        let line = raw.trim_end_matches('\n').trim_end_matches('\r');
        let span = offset..offset + line.len();
        if line.starts_with("{{{#!") && !line.contains("}}}") {
            flush(&mut blocks, &mut paragraph, &mut list);
            let start = offset;
            let mut consumed = raw.len();
            let mut end = offset + line.len();
            while let Some(next) = lines.next() {
                let text = next.trim_end_matches('\n').trim_end_matches('\r');
                end = offset + consumed + text.len();
                consumed += next.len();
                if text.contains("}}}") {
                    break;
                }
            }
            diagnostics.push(Diagnostic::loss(
                start..end,
                "unsupported NamuMark directive",
            ));
            blocks.push(Block::Paragraph(vec![Inline::Text(
                input[start..end].into(),
            )]));
            offset += consumed;
            continue;
        }
        if line == "{{{" {
            flush(&mut blocks, &mut paragraph, &mut list);
            let start = offset;
            let mut literal = String::new();
            let mut literal_lines = 0usize;
            let mut consumed = raw.len();
            let mut closed = false;
            while let Some(next) = lines.next() {
                consumed += next.len();
                let text = next.trim_end_matches('\n').trim_end_matches('\r');
                if text == "}}}" {
                    closed = true;
                    break;
                }
                if literal_lines > 0 {
                    literal.push('\n');
                }
                literal.push_str(text);
                literal_lines += 1;
            }
            if closed {
                blocks.push(Block::Core(core::Block::Pre(literal)));
            } else {
                diagnostics.push(Diagnostic::loss(
                    start..start + consumed,
                    "unclosed NamuMark literal block",
                ));
                blocks.push(Block::Paragraph(vec![Inline::Text(
                    input[start..start + consumed].into(),
                )]));
            }
            offset += consumed;
            continue;
        }
        if line.trim().is_empty() {
            flush(&mut blocks, &mut paragraph, &mut list);
        } else if line.starts_with("||") {
            flush(&mut blocks, &mut paragraph, &mut list);
            let mut text = line.to_owned();
            let mut consumed = raw.len();
            let mut end = offset + line.len();
            while lines.peek().is_some_and(|next| next.starts_with("||")) {
                let next = lines.next().unwrap();
                let clean = next.trim_end_matches('\n').trim_end_matches('\r');
                text.push('\n');
                text.push_str(clean);
                end = offset + consumed + clean.len();
                consumed += next.len();
            }
            diagnostics.push(Diagnostic::loss(offset..end, "unsupported NamuMark table"));
            blocks.push(Block::Paragraph(vec![Inline::Text(text)]));
            offset += consumed;
            continue;
        } else if let Some((level, text)) = heading(line) {
            flush(&mut blocks, &mut paragraph, &mut list);
            let pos = text.as_ptr() as usize - line.as_ptr() as usize;
            blocks.push(Block::Heading {
                level,
                text: inlines(
                    text,
                    offset + pos,
                    context,
                    target,
                    diagnostics,
                    &mut metadata,
                    0,
                ),
            });
        } else if line.trim_start().len() >= 4
            && line.trim_start().len() <= 9
            && line.trim_start().bytes().all(|b| b == b'-')
        {
            flush(&mut blocks, &mut paragraph, &mut list);
            blocks.push(Block::Core(core::Block::Rule));
        } else if let Some((mark, text)) = line
            .strip_prefix(" * ")
            .map(|s| ("*", s))
            .or_else(|| line.strip_prefix(" 1. ").map(|s| ("#", s)))
        {
            if !paragraph.is_empty() {
                flush(&mut blocks, &mut paragraph, &mut list);
            }
            let pos = text.as_ptr() as usize - line.as_ptr() as usize;
            if list.last().is_some_and(|last| last.markers != mark) {
                flush(&mut blocks, &mut paragraph, &mut list);
            }
            list.push(core::ListLine {
                markers: mark.into(),
                content: inlines(
                    text,
                    offset + pos,
                    context,
                    target,
                    diagnostics,
                    &mut metadata,
                    0,
                ),
            });
        } else if let Some(title) = line
            .strip_prefix("#redirect ")
            .or_else(|| line.strip_prefix("#넘겨주기 "))
        {
            flush(&mut blocks, &mut paragraph, &mut list);
            if metadata.redirect_target.is_some() {
                diagnostics.push(Diagnostic::loss(
                    span.clone(),
                    "additional NamuMark redirect",
                ));
                blocks.push(Block::Paragraph(vec![Inline::Text(line.into())]));
                offset += raw.len();
                continue;
            }
            if target == Format::Wikitext
                && (invalid_wiki_title(title) || changes_wiki_title(title))
            {
                diagnostics.push(Diagnostic::loss(
                    span.clone(),
                    "MediaWiki cannot preserve this redirect title",
                ));
            }
            if target == Format::Html && invalid_wiki_title(title) {
                diagnostics.push(Diagnostic::loss(
                    span.clone(),
                    "HTML metadata transport cannot preserve this redirect title",
                ));
            }
            metadata.redirect_target = Some(title.to_owned());
            if target == Format::Gfm {
                diagnostics.push(Diagnostic::loss(
                    span.clone(),
                    "saving the GFM body alone loses the redirect",
                ));
            }
        } else {
            if line.starts_with('>')
                || line.starts_with("##")
                || line.starts_with("  * ")
                || line.starts_with("  1. ")
                || line.starts_with('=')
            {
                diagnostics.push(Diagnostic::loss(
                    span.clone(),
                    "unsupported or malformed NamuMark block syntax",
                ));
                flush(&mut blocks, &mut paragraph, &mut list);
                blocks.push(Block::Paragraph(vec![Inline::Text(line.into())]));
                offset += raw.len();
                continue;
            }
            if !list.is_empty() {
                flush(&mut blocks, &mut paragraph, &mut list);
            }
            if !paragraph.is_empty() {
                paragraph.push(Inline::Html("<br>".into()));
            }
            paragraph.extend(inlines(
                line,
                offset,
                context,
                target,
                diagnostics,
                &mut metadata,
                0,
            ));
        }
        offset += raw.len();
    }
    flush(&mut blocks, &mut paragraph, &mut list);
    if metadata.redirect_target.is_some() && (!blocks.is_empty() || !metadata.categories.is_empty())
    {
        diagnostics.push(Diagnostic::loss(
            0..input.len(),
            "redirect with additional document content",
        ));
    }
    (Document { blocks }, metadata)
}
fn heading(line: &str) -> Option<(u8, &str)> {
    let n = line.bytes().take_while(|&c| c == b'=').count();
    if !(1..=6).contains(&n) || line.len() < 2 * n + 2 || !line.ends_with(&"=".repeat(n)) {
        return None;
    }
    let inner = &line[n..line.len() - n];
    Some((n as u8, inner.strip_prefix(' ')?.strip_suffix(' ')?))
}
fn inlines(
    input: &str,
    base: usize,
    context: &WikiContext<'_>,
    target: Format,
    diagnostics: &mut Vec<Diagnostic>,
    metadata: &mut PageMetadata,
    depth: usize,
) -> Vec<Inline> {
    if depth > 32 {
        diagnostics.push(Diagnostic::loss(
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
            diagnostics.push(Diagnostic::loss(
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
                    if target == Format::Wikitext
                        && (invalid_wiki_title(&name) || changes_wiki_title(&name))
                    {
                        diagnostics.push(Diagnostic::loss(
                            base + i..base + i + full,
                            "MediaWiki cannot preserve this category title",
                        ));
                    }
                    if target == Format::Html
                        && (invalid_wiki_title(&name)
                            || sort.as_ref().is_some_and(|s| invalid_metadata_sort(s)))
                    {
                        diagnostics.push(Diagnostic::loss(
                            base + i..base + i + full,
                            "HTML metadata transport cannot preserve this category",
                        ));
                    }
                    metadata.categories.push(name);
                    metadata.category_sort_keys.push(sort);
                    if target == Format::Gfm {
                        diagnostics.push(Diagnostic::loss(
                            base + i..base + i + full,
                            "saving the GFM body alone loses the category",
                        ));
                    }
                    i += full;
                    continue;
                }
                diagnostics.push(Diagnostic::loss(
                    base + i..base + i + full,
                    "unsupported NamuMark category",
                ));
                inline::text(&mut out, &rest[..full]);
                i += full;
                continue;
            }
        }
        if let Some(body) = rest.strip_prefix("[[") {
            if let Some(end) = find_unescaped(body, "]]") {
                let inside = &body[..end];
                let (raw_title, label) = find_unescaped(inside, "|")
                    .map_or((inside, inside), |n| (&inside[..n], &inside[n + 1..]));
                let title_value = unescape(raw_title);
                let title = title_value.as_str();
                let external = title
                    .get(..7)
                    .is_some_and(|s| s.eq_ignore_ascii_case("http://"))
                    || title
                        .get(..8)
                        .is_some_and(|s| s.eq_ignore_ascii_case("https://"));
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
                            || title.split_once("://").is_none_or(|(_, h)| {
                                h.is_empty() || h.starts_with(['/', '?', '#'])
                            })))
                {
                    diagnostics.push(Diagnostic::loss(
                        base + i..base + i + end + 4,
                        "unsupported or unsafe NamuMark link",
                    ));
                    inline::text(&mut out, &rest[..end + 4]);
                    i += end + 4;
                    continue;
                }
                // A link target is "page#fragment"; only the page part is relative. Split the
                // fragment once so '#' is never treated as a path separator.
                let (title_page, title_fragment) = title
                    .split_once('#')
                    .map_or((title, None), |(page, fragment)| (page, Some(fragment)));
                // '\#' is a literal title character, not a fragment separator
                // (docs/namumark.md: never reinterpret escaped syntax), so the separator is
                // located on the raw title and only an unescaped fragment can be automatic.
                if !external
                    && find_unescaped(raw_title, "#").is_some_and(|hash| {
                        is_automatic_section_anchor(&unescape(&raw_title[hash + 1..]))
                    })
                {
                    diagnostics.push(Diagnostic::loss(
                        base + i..base + i + end + 4,
                        "unsupported NamuMark automatic section anchor",
                    ));
                    inline::text(&mut out, &rest[..end + 4]);
                    i += end + 4;
                    continue;
                }
                if target == Format::Wikitext
                    && !external
                    && title
                        .chars()
                        .any(|c| matches!(c, '[' | ']' | '{' | '}' | '|'))
                {
                    diagnostics.push(Diagnostic::loss(
                        base + i..base + i + end + 4,
                        "MediaWiki cannot represent the NamuMark link title",
                    ));
                    inline::text(&mut out, &rest[..end + 4]);
                    i += end + 4;
                    continue;
                }
                let resolved_page = if let Some(child) = title_page.strip_prefix('/') {
                    if context.page_title.is_empty() {
                        diagnostics.push(Diagnostic::loss(
                            base + i..base + i + end + 4,
                            "relative link requires page title",
                        ));
                        title_page.to_owned()
                    } else if child.is_empty() {
                        context.page_title.to_owned()
                    } else {
                        format!("{}/{child}", context.page_title)
                    }
                } else if let Some(child) = title_page.strip_prefix("../") {
                    if context.page_title.is_empty() {
                        diagnostics.push(Diagnostic::loss(
                            base + i..base + i + end + 4,
                            "relative link requires page title",
                        ));
                        title_page.to_owned()
                    } else {
                        let parent = context
                            .page_title
                            .rsplit_once('/')
                            .map_or(context.page_title, |(p, _)| p);
                        if child.is_empty() {
                            parent.to_owned()
                        } else {
                            format!("{parent}/{child}")
                        }
                    }
                } else {
                    title_page.to_owned()
                };
                let resolved = match title_fragment {
                    Some(fragment) => format!("{resolved_page}#{fragment}"),
                    None => resolved_page,
                };
                let resolved = if target == Format::Wikitext {
                    resolved
                        .strip_prefix(":분류:")
                        .map_or(resolved.clone(), |name| format!(":Category:{name}"))
                } else {
                    resolved
                };
                let href = if external {
                    resolved
                } else if resolved.starts_with('#') {
                    format!("#{}", inline::encode_component(&resolved[1..]))
                } else {
                    let title = resolved.strip_prefix(':').unwrap_or(&resolved);
                    match title.split_once('#') {
                        Some((page, fragment)) => format!(
                            "./{}#{}",
                            inline::encode_component(page),
                            inline::encode_component(fragment)
                        ),
                        None => format!("./{}", inline::encode_component(title)),
                    }
                };
                if target == Format::Wikitext
                    && !external
                    && (title.contains('_') || title.starts_with(|c: char| c.is_lowercase()))
                {
                    diagnostics.push(Diagnostic::loss(
                        base + i..base + i + end + 4,
                        "MediaWiki title normalization changes the NamuMark title",
                    ));
                }
                out.push(Inline::Link {
                    href,
                    label: inlines(
                        label,
                        base + i + 2 + inside.len() - label.len(),
                        context,
                        target,
                        diagnostics,
                        metadata,
                        depth + 1,
                    ),
                    internal: !external,
                });
                i += end + 4;
                continue;
            }
            diagnostics.push(Diagnostic::loss(
                base + i..base + input.len(),
                "unclosed NamuMark link",
            ));
            inline::text(&mut out, rest);
            break;
        }
        if apostrophe_run(rest) >= 2 {
            match emphasis(
                rest,
                base + i,
                context,
                target,
                diagnostics,
                metadata,
                depth,
            ) {
                Ok((nodes, consumed)) => {
                    out.extend(nodes);
                    i += consumed;
                }
                Err(()) => {
                    diagnostics.push(Diagnostic::loss(
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
            diagnostics.push(Diagnostic::loss(
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
                    diagnostics.push(Diagnostic::loss(
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

fn find_unescaped(input: &str, needle: &str) -> Option<usize> {
    let mut pos = 0;
    while pos < input.len() {
        let rest = &input[pos..];
        if rest.starts_with('\\') {
            pos += 1;
            if let Some(c) = input[pos..].chars().next() {
                pos += c.len_utf8();
            }
            continue;
        }
        if rest.starts_with(needle) {
            return Some(pos);
        }
        pos += rest.chars().next().unwrap().len_utf8();
    }
    None
}

/// Number of leading apostrophes in `input`.
fn apostrophe_run(input: &str) -> usize {
    input.bytes().take_while(|&byte| byte == b'\'').count()
}

/// Locate the first unescaped run of exactly `n` apostrophes. Runs of any
/// other length are skipped whole so a longer run never closes a shorter one.
fn find_exact_run(input: &str, n: usize) -> Option<usize> {
    find_apostrophe_run(input, |run| run == n).map(|(offset, _)| offset)
}

/// Locate the first unescaped run of at least `n` apostrophes, returning the
/// offset and the full run length.
fn find_run_at_least(input: &str, n: usize) -> Option<(usize, usize)> {
    find_apostrophe_run(input, |run| run >= n)
}

/// Locate the first unescaped run that can close italic: exactly two
/// apostrophes, or the leading two of a five-or-more run (the remainder can
/// still open bold).
fn find_italic_closer(input: &str) -> Option<(usize, usize)> {
    find_apostrophe_run(input, |run| run == 2 || run >= 5)
}

/// Locate the first unescaped apostrophe run accepted by `accept`, returning
/// the offset and the full run length. Runs the predicate rejects are skipped
/// whole so a longer run never closes a shorter one.
fn find_apostrophe_run(input: &str, accept: impl Fn(usize) -> bool) -> Option<(usize, usize)> {
    let mut pos = 0;
    while pos < input.len() {
        let rest = &input[pos..];
        if let Some(stripped) = rest.strip_prefix('\\') {
            pos += 1;
            if let Some(c) = stripped.chars().next() {
                pos += c.len_utf8();
            }
            continue;
        }
        let run = apostrophe_run(rest);
        if run > 0 {
            if accept(run) {
                return Some((pos, run));
            }
            pos += run;
            continue;
        }
        pos += rest.chars().next().unwrap().len_utf8();
    }
    None
}

/// Whether an emphasis body is free of constructs that could swallow a
/// virtually appended closer: escapes, brackets, and braces.
fn simple_emphasis_body(body: &str) -> bool {
    !body
        .bytes()
        .any(|b| matches!(b, b'\\' | b'[' | b']' | b'{' | b'}'))
}

/// Whether a body contains an odd number of emphasis openers of `width`; a
/// three-apostrophe width also counts a four-apostrophe run, which opens bold
/// and leaves one literal apostrophe.
fn odd_emphasis_runs(body: &str, width: usize) -> bool {
    let mut parity = false;
    let mut pos = 0;
    while pos < body.len() {
        let rest = &body[pos..];
        if let Some(stripped) = rest.strip_prefix('\\') {
            pos += 1;
            if let Some(c) = stripped.chars().next() {
                pos += c.len_utf8();
            }
            continue;
        }
        let run = apostrophe_run(rest);
        if run > 0 {
            if run == width || (width == 3 && run == 4) {
                parity = !parity;
            }
            pos += run;
            continue;
        }
        pos += rest.chars().next().unwrap().len_utf8();
    }
    parity
}

/// Parse an emphasis construct whose input starts at an apostrophe run.
///
/// Two apostrophes are italic, three or four are bold (a fourth opener or
/// closer apostrophe stays literal), and five or more combine both. A combined
/// opener prefers the first run of five or more apostrophes as the symmetric
/// closer, so nested short markers stay inside the body, and only falls back
/// to the earliest short run (two, three, or four apostrophes) when no 5+ run
/// exists. Symmetric closers nest bold inside italic; a two-apostrophe closer
/// closes italic first, so bold wraps the remainder, and a three- or
/// four-apostrophe closer closes bold first, so italic wraps the remainder.
/// Every closer is located before any recursive parsing so a failure never
/// emits nested diagnostics; on failure the whole construct is left to the
/// caller's literal fallback.
fn emphasis(
    input: &str,
    base: usize,
    context: &WikiContext<'_>,
    target: Format,
    diagnostics: &mut Vec<Diagnostic>,
    metadata: &mut PageMetadata,
    depth: usize,
) -> Result<(Vec<Inline>, usize), ()> {
    let opener = apostrophe_run(input);
    let mut nodes = Vec::new();
    let consumed;
    if opener == 2 {
        let (closer_offset, closer_run) = find_italic_closer(&input[2..]).ok_or(())?;
        let body_end = 2 + closer_offset;
        let body = &input[2..body_end];
        // A five-or-more run can donate its leading two apostrophes to close
        // this italic; if the body holds exactly one unmatched bold opener, a
        // virtual `'''` closes it and the rest of the run opens the next
        // construct.
        let append = if closer_run >= 5 && simple_emphasis_body(body) && odd_emphasis_runs(body, 3)
        {
            "'''"
        } else {
            ""
        };
        let body_input;
        let body = if append.is_empty() {
            body
        } else {
            body_input = format!("{body}{append}");
            &body_input
        };
        let children = inlines(
            body,
            base + 2,
            context,
            target,
            diagnostics,
            metadata,
            depth + 1,
        );
        nodes.push(Inline::Emphasis {
            strong: false,
            children,
        });
        // Consume the closer's leading two apostrophes; when a five-or-more
        // run is shared, `append.len()` apostrophes of that run also close the
        // inner bold, and any remainder is re-lexed by the caller.
        consumed = body_end + append.len() + 2;
    } else if opener == 3 || opener == 4 {
        let (closer_offset, closer_run) = find_run_at_least(&input[3..], 3).ok_or(())?;
        let body_end = 3 + closer_offset;
        let body = &input[3..body_end];
        // A five-or-more run can donate its leading three apostrophes to close
        // this bold; if the body holds exactly one unmatched italic opener, a
        // virtual `''` closes it and the rest of the run opens the next
        // construct.
        let append = if closer_run >= 5 && simple_emphasis_body(body) && odd_emphasis_runs(body, 2)
        {
            "''"
        } else {
            ""
        };
        let body_input;
        let body = if append.is_empty() {
            body
        } else {
            body_input = format!("{body}{append}");
            &body_input
        };
        let children = inlines(
            body,
            base + 3,
            context,
            target,
            diagnostics,
            metadata,
            depth + 1,
        );
        nodes.push(Inline::Emphasis {
            strong: true,
            children,
        });
        // Consume three apostrophes; when a five-or-more run is shared,
        // `append.len()` apostrophes also close the inner italic, and any
        // remainder is re-lexed by the caller.
        consumed = body_end + append.len() + 3;
    } else {
        // Five or more apostrophes combine italic and bold. Prefer the first
        // run of five or more apostrophes as the symmetric closer so nested
        // short markers (`''`, `'''`) stay in the body, and fall back to the
        // earliest short run only when no 5+ run exists.
        let body_start = opener;
        let (closer_offset, closer_run) = find_run_at_least(&input[body_start..], 5)
            .or_else(|| find_run_at_least(&input[body_start..], 2))
            .ok_or(())?;
        let body_end = body_start + closer_offset;
        let node;
        if closer_run >= 5 {
            // Symmetric combined form: italic outside bold.
            let children = inlines(
                &input[body_start..body_end],
                base + body_start,
                context,
                target,
                diagnostics,
                metadata,
                depth + 2,
            );
            node = Inline::Emphasis {
                strong: false,
                children: vec![Inline::Emphasis {
                    strong: true,
                    children,
                }],
            };
            // Extra closer-run apostrophes are re-lexed by the caller.
            consumed = body_start + closer_offset + 5;
        } else if closer_run == 2 {
            // Italic closes first, so bold wraps the remainder.
            let tail_start = body_end + 2;
            let (tail_offset, _) = find_run_at_least(&input[tail_start..], 3).ok_or(())?;
            let tail_end = tail_start + tail_offset;
            let inner = inlines(
                &input[body_start..body_end],
                base + body_start,
                context,
                target,
                diagnostics,
                metadata,
                depth + 2,
            );
            let tail = inlines(
                &input[tail_start..tail_end],
                base + tail_start,
                context,
                target,
                diagnostics,
                metadata,
                depth + 1,
            );
            let mut children = vec![Inline::Emphasis {
                strong: false,
                children: inner,
            }];
            children.extend(tail);
            node = Inline::Emphasis {
                strong: true,
                children,
            };
            consumed = tail_end + 3;
        } else {
            // Bold closes first (three or four apostrophes), so italic wraps
            // the remainder.
            let tail_start = body_end + 3;
            let tail_offset = find_exact_run(&input[tail_start..], 2).ok_or(())?;
            let tail_end = tail_start + tail_offset;
            let inner = inlines(
                &input[body_start..body_end],
                base + body_start,
                context,
                target,
                diagnostics,
                metadata,
                depth + 2,
            );
            let tail = inlines(
                &input[tail_start..tail_end],
                base + tail_start,
                context,
                target,
                diagnostics,
                metadata,
                depth + 1,
            );
            let mut children = vec![Inline::Emphasis {
                strong: true,
                children: inner,
            }];
            children.extend(tail);
            node = Inline::Emphasis {
                strong: false,
                children,
            };
            consumed = tail_end + 2;
        };
        // Any opener apostrophes beyond the combined marker stay literal.
        if opener > 5 {
            nodes.push(Inline::Text("'".repeat(opener - 5)));
        }
        nodes.push(node);
    }
    Ok((nodes, consumed))
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
    let (name, has_parentheses) = inner
        .split_once('(')
        .map_or((inner, false), |(name, _)| (name, true));
    if NAMUMARK_MACROS_WITH_ARGUMENTS
        .iter()
        .any(|macro_name| name.eq_ignore_ascii_case(macro_name))
    {
        return has_parentheses;
    }
    NAMUMARK_MACROS
        .iter()
        .any(|macro_name| name.eq_ignore_ascii_case(macro_name))
}

fn is_namumark_footnote(inner: &str) -> bool {
    inner.starts_with('*')
}

fn unescape(input: &str) -> String {
    let mut out = String::new();
    let mut chars = input.chars();
    while let Some(c) = chars.next() {
        if c == '\\' {
            if let Some(next) = chars.next() {
                out.push(next);
            } else {
                out.push(c);
            }
        } else {
            out.push(c);
        }
    }
    out
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

fn invalid_wiki_title(title: &str) -> bool {
    title.trim().is_empty()
        || title
            .chars()
            .any(|c| c.is_control() || matches!(c, '[' | ']' | '{' | '}' | '|' | '<' | '>' | '?'))
}
fn invalid_metadata_sort(sort: &str) -> bool {
    sort.chars()
        .any(|c| c.is_control() || matches!(c, '[' | ']' | '{' | '}' | '|' | '<' | '>'))
}
fn changes_wiki_title(title: &str) -> bool {
    title.contains('_') || title.starts_with(|c: char| c.is_lowercase())
}
pub(crate) fn render_wikitext_metadata(metadata: &PageMetadata) -> String {
    let mut parts = Vec::new();
    if let Some(title) = &metadata.redirect_target {
        if invalid_wiki_title(title) {
            parts.push(crate::escape_wikitext(&format!("#redirect {title}")));
        } else {
            parts.push(format!("#REDIRECT [[{title}]]"));
        }
    }
    for (i, name) in metadata.categories.iter().enumerate() {
        let sort = metadata
            .category_sort_keys
            .get(i)
            .and_then(|s| s.as_deref());
        if invalid_wiki_title(name) {
            parts.push(crate::escape_wikitext(&format!(
                "[[분류:{name}{}]]",
                sort.map(|s| format!("|{s}")).unwrap_or_default()
            )));
        } else {
            parts.push(format!(
                "[[Category:{name}{}]]",
                sort.map(|s| format!("|{}", crate::escape_wikitext(s)))
                    .unwrap_or_default()
            ));
        }
    }
    parts.join("\n")
}
