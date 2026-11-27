use super::{inline::InlineParser, links::changes_wiki_title};
use crate::{
    core, metadata, Block, Diagnostic, Document, Format, Inline, PageMetadata, WikiContext,
};

pub(crate) fn parse(
    input: &str,
    context: &WikiContext<'_>,
    target: Format,
    diagnostics: &mut Vec<Diagnostic>,
) -> (Document, PageMetadata) {
    let mut blocks = Vec::new();
    let mut inline_parser = InlineParser::new(context, target, diagnostics);
    let mut paragraph = Vec::new();
    let mut list: Vec<core::ListLine> = Vec::new();
    let mut in_list_continuation = false;
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
            in_list_continuation = false;
            flush(&mut blocks, &mut paragraph, &mut list);
            let start = offset;
            let mut consumed = raw.len();
            let mut end = offset + line.len();
            for next in lines.by_ref() {
                let text = next.trim_end_matches('\n').trim_end_matches('\r');
                end = offset + consumed + text.len();
                consumed += next.len();
                if text.contains("}}}") {
                    break;
                }
            }
            inline_parser.push_loss(start..end, "unsupported NamuMark directive");
            blocks.push(Block::Paragraph(vec![Inline::Text(
                input[start..end].into(),
            )]));
            offset += consumed;
            continue;
        }
        if line == "{{{" {
            in_list_continuation = false;
            flush(&mut blocks, &mut paragraph, &mut list);
            let start = offset;
            let mut literal = String::new();
            let mut consumed = raw.len();
            let mut closed = false;
            for (literal_lines, next) in lines.by_ref().enumerate() {
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
            }
            if closed {
                blocks.push(Block::Core(core::Block::Pre(literal)));
            } else {
                inline_parser.push_loss(start..start + consumed, "unclosed NamuMark literal block");
                blocks.push(Block::Paragraph(vec![Inline::Text(
                    input[start..start + consumed].into(),
                )]));
            }
            offset += consumed;
            continue;
        }
        if line.trim().is_empty() {
            in_list_continuation = false;
            flush(&mut blocks, &mut paragraph, &mut list);
        } else if line.starts_with("||") {
            in_list_continuation = false;
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
            inline_parser.push_loss(offset..end, "unsupported NamuMark table");
            blocks.push(Block::Paragraph(vec![Inline::Text(text)]));
            offset += consumed;
            continue;
        } else if let Some((level, text)) = heading(line) {
            in_list_continuation = false;
            flush(&mut blocks, &mut paragraph, &mut list);
            let pos = text.as_ptr() as usize - line.as_ptr() as usize;
            blocks.push(Block::Heading {
                level,
                text: inline_parser.parse_line(text, offset + pos),
            });
        } else if line.trim_start().len() >= 4
            && line.trim_start().len() <= 9
            && line.trim_start().bytes().all(|b| b == b'-')
        {
            in_list_continuation = false;
            flush(&mut blocks, &mut paragraph, &mut list);
            blocks.push(Block::Core(core::Block::Rule));
        } else if let Some((mark, text)) = line
            .strip_prefix(" * ")
            .map(|s| ("*", s))
            .or_else(|| line.strip_prefix(" 1. ").map(|s| ("#", s)))
        {
            in_list_continuation = false;
            if !paragraph.is_empty() {
                flush(&mut blocks, &mut paragraph, &mut list);
            }
            let pos = text.as_ptr() as usize - line.as_ptr() as usize;
            if list.last().is_some_and(|last| last.markers != mark) {
                inline_parser.push_loss(span.clone(), "unsupported mixed NamuMark list");
                flush(&mut blocks, &mut paragraph, &mut list);
            }
            list.push(core::ListLine {
                markers: mark.into(),
                content: inline_parser.parse_line(text, offset + pos),
            });
        } else if let Some(title) = line
            .strip_prefix("#redirect ")
            .or_else(|| line.strip_prefix("#넘겨주기 "))
        {
            in_list_continuation = false;
            flush(&mut blocks, &mut paragraph, &mut list);
            if inline_parser.metadata().redirect_target.is_some() {
                inline_parser.push_loss(span.clone(), "additional NamuMark redirect");
                blocks.push(Block::Paragraph(vec![Inline::Text(line.into())]));
                offset += raw.len();
                continue;
            }
            if target == Format::Wikitext
                && (!metadata::valid_title(title) || changes_wiki_title(title))
            {
                inline_parser.push_loss(
                    span.clone(),
                    "MediaWiki cannot preserve this redirect title",
                );
            }
            if target == Format::Html && !metadata::valid_title(title) {
                inline_parser.push_loss(
                    span.clone(),
                    "HTML metadata transport cannot preserve this redirect title",
                );
            }
            inline_parser.metadata_mut().redirect_target = Some(title.to_owned());
            if target == Format::Gfm {
                inline_parser.push_loss(span.clone(), "saving the GFM body alone loses the redirect");
            }
        } else {
            if line.starts_with('>')
                || line.starts_with("##")
                || line.starts_with('=')
                || is_excluded_namumark_list(line)
            {
                in_list_continuation = false;
                inline_parser.push_loss(
                    span.clone(),
                    "unsupported or malformed NamuMark block syntax",
                );
                flush(&mut blocks, &mut paragraph, &mut list);
                blocks.push(Block::Paragraph(vec![Inline::Text(line.into())]));
                offset += raw.len();
                continue;
            }
            if (!list.is_empty() || in_list_continuation) && line.starts_with(' ') {
                in_list_continuation = true;
                inline_parser.push_loss(span.clone(), "unsupported NamuMark list continuation");
                flush(&mut blocks, &mut paragraph, &mut list);
                blocks.push(Block::Paragraph(vec![Inline::Text(line.into())]));
                offset += raw.len();
                continue;
            }
            in_list_continuation = false;
            if !list.is_empty() {
                flush(&mut blocks, &mut paragraph, &mut list);
            }
            if !paragraph.is_empty() {
                paragraph.push(Inline::Html("<br>".into()));
            }
            paragraph.extend(inline_parser.parse_line(line, offset));
        }
        offset += raw.len();
    }
    flush(&mut blocks, &mut paragraph, &mut list);
    let metadata = inline_parser.into_metadata();
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

fn is_excluded_namumark_list(line: &str) -> bool {
    let spaces = line.bytes().take_while(|&b| b == b' ').count();
    if spaces == 0 {
        return false;
    }
    let rest = &line[spaces..];
    if spaces >= 2 && (rest.starts_with("* ") || rest.starts_with("1. ")) {
        return true;
    }
    if let Some(after) = rest
        .strip_prefix("1.")
        .or_else(|| rest.strip_prefix("a."))
        .or_else(|| rest.strip_prefix("A."))
        .or_else(|| rest.strip_prefix("i."))
        .or_else(|| rest.strip_prefix("I."))
    {
        if let Some(after_hash) = after.strip_prefix('#') {
            let digits = after_hash.bytes().take_while(u8::is_ascii_digit).count();
            return digits > 0 && after_hash[digits..].starts_with(' ');
        }
        return !rest.starts_with("1.") && after.starts_with(' ');
    }
    let digits = rest.bytes().take_while(u8::is_ascii_digit).count();
    digits > 0 && &rest[..digits] != "1" && rest[digits..].starts_with(". ")
}
