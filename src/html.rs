use std::ops::Range;

use scraper::{ElementRef, Html, Node};

use crate::{
    core, escape_wikitext, heading_id, html_policy, inline, locate, original_range, render_inline,
    table, transclusion, wiki_links, wiki_markup, Block, ConversionError, Diagnostic, Document,
    Format, Inline, WikiData,
};

enum MarkdownSource {
    RawHtml,
    Link(String),
    Image,
}

fn markdown_span(
    input: &str,
    lines: &[usize],
    position: comrak::nodes::Sourcepos,
) -> Option<Range<usize>> {
    let start =
        lines.get(position.start.line.checked_sub(1)?)? + position.start.column.checked_sub(1)?;
    let end = lines.get(position.end.line.checked_sub(1)?)? + position.end.column;
    let span = start..end.min(input.len());
    input.get(span.clone()).map(|_| span)
}

fn markdown_locations<'a>(
    root: &'a comrak::nodes::AstNode<'a>,
    input: &str,
) -> Vec<(Range<usize>, MarkdownSource)> {
    use comrak::nodes::NodeValue;

    let lines: Vec<_> = std::iter::once(0)
        .chain(input.match_indices('\n').map(|(offset, _)| offset + 1))
        .collect();
    root.descendants()
        .filter_map(|node| {
            let node = node.data.borrow();
            let kind = match &node.value {
                NodeValue::HtmlInline(_) | NodeValue::HtmlBlock(_) => MarkdownSource::RawHtml,
                NodeValue::Link(link) => MarkdownSource::Link(link.url.clone()),
                NodeValue::Image(_) => MarkdownSource::Image,
                _ => return None,
            };
            markdown_span(input, &lines, node.sourcepos).map(|span| (span, kind))
        })
        .collect()
}

fn markdown_diagnostics<'a>(
    html: &'a str,
    markdown: &'a str,
    locations: &'a [(Range<usize>, MarkdownSource)],
    diagnostics: Vec<Diagnostic>,
) -> impl Iterator<Item = Diagnostic> + 'a {
    let mut raw_cursor = 0;
    let mut link_cursor = 0;
    let mut image_cursor = 0;
    diagnostics.into_iter().map(move |mut diagnostic| {
        let fragment = &html[diagnostic.span.clone()];
        let raw = locations.iter().find_map(|(span, kind)| {
            if !matches!(kind, MarkdownSource::RawHtml) || span.end <= raw_cursor {
                return None;
            }
            let start = span.start.max(raw_cursor);
            markdown[start..span.end].find(fragment).map(|offset| {
                let start = start + offset;
                start..start + fragment.len()
            })
        });
        diagnostic.span = if let Some(raw) = raw {
            raw_cursor = raw.end;
            raw
        } else if let Some(url) = fragment
            .strip_prefix("href=\"")
            .and_then(|href| href.strip_suffix('"'))
        {
            let url = html_escape::decode_html_entities(url);
            locations
                .iter()
                .find_map(|(span, kind)| {
                    if span.start < link_cursor
                        || !matches!(kind, MarkdownSource::Link(candidate) if *candidate == url)
                    {
                        return None;
                    }
                    link_cursor = span.end;
                    Some(span.clone())
                })
                .unwrap_or(0..markdown.len())
        } else if fragment.starts_with("<img ") {
            locations
                .iter()
                .find_map(|(span, kind)| {
                    if span.start < image_cursor || !matches!(kind, MarkdownSource::Image) {
                        return None;
                    }
                    image_cursor = span.end;
                    Some(span.clone())
                })
                .unwrap_or(0..markdown.len())
        } else {
            locate(markdown, fragment)
        };
        diagnostic
    })
}

pub(crate) fn parse_gfm(
    input: &str,
    wiki_data: Option<&dyn WikiData>,
    diagnostics: &mut Vec<Diagnostic>,
) -> Result<Document, ConversionError> {
    let mut options = comrak::Options::default();
    options.extension.autolink = true;
    options.extension.table = true;
    options.extension.strikethrough = true;
    options.extension.tasklist = true;
    // HTML and URLs are validated below at the same boundary as HTML imports.
    options.render.unsafe_ = true;
    let arena = comrak::Arena::new();
    let root = comrak::parse_document(&arena, input, &options);
    let mut output = Vec::new();
    comrak::format_html(root, &options, &mut output)
        .map_err(|error| ConversionError::new(format!("failed to render GFM: {error}")))?;
    let html = String::from_utf8(output)
        .map_err(|_| ConversionError::new("GFM renderer produced invalid UTF-8"))?;
    let locations = markdown_locations(root, input);
    let sanitized = html_policy::sanitize(&html, false);
    diagnostics.extend(markdown_diagnostics(
        &html,
        input,
        &locations,
        sanitized.diagnostics,
    ));
    parse_html_document(&sanitized.text, input, wiki_data, diagnostics)
}

fn normalized_html_text(text: &str) -> String {
    let mut output = String::new();
    let mut space = false;
    for c in text.chars() {
        if matches!(c, ' ' | '\t' | '\n' | '\r' | '\u{000C}') {
            if !space {
                output.push(' ');
            }
            space = true;
        } else {
            output.push(c);
            space = false;
        }
    }
    output
}

fn push_html_inline(element: ElementRef<'_>, content: &mut Vec<Inline>) {
    match element.value().name() {
        "a" if element.value().attrs().all(|(name, _)| name == "href") => {
            content.extend(html_link(element));
        }
        "b" | "strong" | "i" | "em" if element.value().attrs().next().is_none() => {
            let children = html_inlines(element, false);
            content.push(Inline::Emphasis {
                strong: matches!(element.value().name(), "b" | "strong"),
                children,
            });
        }
        _ => content.push(Inline::Html(element.html())),
    }
}

fn html_link(element: ElementRef<'_>) -> Vec<Inline> {
    let label = html_inlines(element, false);
    if let Some((href, internal)) = element
        .attr("href")
        .and_then(|href| inline::link_target(href).ok())
    {
        vec![Inline::Link {
            href,
            label,
            internal,
        }]
    } else {
        label
    }
}

fn html_inlines(parent: ElementRef<'_>, trim: bool) -> Vec<Inline> {
    let mut content = Vec::new();
    for child in parent.children() {
        match child.value() {
            Node::Text(text)
                if parent
                    .ancestors()
                    .filter_map(ElementRef::wrap)
                    .any(|ancestor| ancestor.value().name() == "pre")
                    || parent.value().name() == "pre" =>
            {
                inline::text(&mut content, text)
            }
            Node::Text(text) => inline::text(&mut content, &normalized_html_text(text)),
            Node::Element(element)
                if matches!(parent.value().name(), "li" | "dt" | "dd")
                    && matches!(element.name(), "ol" | "ul" | "dl") => {}
            Node::Element(element) if element.name() == "p" && parent.value().name() == "li" => {
                content.extend(html_inlines(ElementRef::wrap(child).unwrap(), trim));
            }
            Node::Element(_) => push_html_inline(ElementRef::wrap(child).unwrap(), &mut content),
            _ => {}
        }
    }
    if trim {
        if let Some(Inline::Text(first)) = content.first_mut() {
            *first = first.trim_start_matches(' ').to_owned();
        }
        if let Some(Inline::Text(last)) = content.last_mut() {
            *last = last.trim_end_matches(' ').to_owned();
        }
    }
    content
}

pub(crate) fn parse_html_inlines(parent: ElementRef<'_>) -> Result<Vec<Inline>, ConversionError> {
    Ok(html_inlines(parent, parent.value().name() != "pre"))
}

pub(crate) fn parse_html(
    input: &str,
    original: &str,
    extracted: &[Range<usize>],
    markup: &mut wiki_markup::Markup,
    wiki_data: Option<&dyn WikiData>,
    diagnostics: &mut Vec<Diagnostic>,
) -> Result<Document, ConversionError> {
    let (edits, metadata_diagnostics) = transclusion::recover(input);
    let sanitized = html_policy::sanitize(input, false);
    diagnostics.extend(
        metadata_diagnostics
            .into_iter()
            .chain(sanitized.diagnostics.iter().cloned())
            .map(|mut diagnostic| {
                diagnostic.span = original_range(diagnostic.span, extracted);
                diagnostic
            }),
    );
    for edit in edits {
        markup.replace(
            sanitized.output_span(edit.span),
            edit.code,
            edit.block,
            false,
        );
    }
    parse_html_document(
        &markup.apply(&sanitized.text).text,
        original,
        wiki_data,
        diagnostics,
    )
}

fn parse_html_document(
    sanitized: &str,
    original: &str,
    wiki_data: Option<&dyn WikiData>,
    diagnostics: &mut Vec<Diagnostic>,
) -> Result<Document, ConversionError> {
    let dom = Html::parse_fragment(sanitized);
    let check_url = |resource: &str, url: &str, diagnostics: &mut Vec<Diagnostic>| {
        let span = locate(original, url);
        let title = inline::normalize_title(resource);
        let expected = wiki_data
            .map_or(Ok(None), |data| data.file_info(&title))
            .map_err(|error| {
                ConversionError::lookup(span.clone(), "file information", &title, error)
            })?;
        if expected.is_none_or(|file| file.url != url) {
            diagnostics.push(Diagnostic::loss(
                span,
                "wiki file syntax resolves the media URL through wiki data",
            ));
        }
        Ok::<(), ConversionError>(())
    };
    for link in dom
        .root_element()
        .descendent_elements()
        .filter(|element| element.value().name() == "a")
    {
        if let (Some(name), Some(url)) = (wiki_links::media_link_name(link), link.attr("href")) {
            let resource = format!("File:{name}");
            if inline::internal_href(&resource).ok().as_deref() != Some(url) {
                check_url(&resource, url, diagnostics)?;
            }
        }
    }
    for image in dom
        .root_element()
        .descendent_elements()
        .filter(|element| element.value().name() == "img")
    {
        let src = image.attr("src");
        let span = src.map_or_else(|| locate(original, "<img"), |src| locate(original, src));
        let resource = image
            .ancestors()
            .filter_map(ElementRef::wrap)
            .find_map(|parent| {
                wiki_links::file_element(parent)
                    .filter(|media| media.id() == image.id())
                    .and_then(wiki_links::file_resource)
            });
        if let Some(resource) = resource {
            if let Some(src) = src {
                check_url(&resource, src, diagnostics)?;
            }
        } else {
            diagnostics.push(Diagnostic::loss(
                span,
                "image has no valid wiki file resource",
            ));
        }
    }
    let mut blocks = Vec::new();
    let mut ids = std::collections::HashSet::new();
    html_blocks(
        dom.root_element(),
        original,
        diagnostics,
        &mut ids,
        &mut blocks,
    )?;
    Ok(Document { blocks })
}

fn html_blocks(
    parent: ElementRef<'_>,
    original: &str,
    diagnostics: &mut Vec<Diagnostic>,
    ids: &mut std::collections::HashSet<String>,
    blocks: &mut Vec<Block>,
) -> Result<(), ConversionError> {
    let mut paragraph = Vec::new();
    for child in parent.children() {
        if let Some(element) = ElementRef::wrap(child) {
            let name = element.value().name();
            if html_policy::is_block(name) || matches!(name, "link" | "meta") {
                if has_inline_content(&paragraph) {
                    blocks.push(Block::Paragraph(std::mem::take(&mut paragraph)));
                }
                paragraph.clear();
                let block = match name {
                    "section" if element.value().attrs().next().is_none() => {
                        html_blocks(element, original, diagnostics, ids, blocks)?;
                        continue;
                    }
                    "p" if element.value().attrs().next().is_none()
                        && (!element.text().collect::<String>().trim_ascii().is_empty()
                            || element
                                .descendent_elements()
                                .any(|child| child.attr("data-rwikitext-token").is_some())) =>
                    {
                        Block::Paragraph(parse_html_inlines(element)?)
                    }
                    "h1" | "h2" | "h3" | "h4" | "h5" | "h6" => {
                        let text = parse_html_inlines(element)?;
                        let canonical = heading_id(&text, ids);
                        if element
                            .value()
                            .attrs()
                            .all(|(key, value)| key == "id" && value == canonical)
                        {
                            Block::Heading {
                                level: name.as_bytes()[1] - b'0',
                                text,
                            }
                        } else {
                            Block::Html(element.html())
                        }
                    }
                    "table" => {
                        let span = locate(original, "<table");
                        diagnostics.extend(
                            table::Table::html_import_losses(element)
                                .into_iter()
                                .map(|reason| Diagnostic::loss(span.clone(), reason)),
                        );
                        match table::Table::from_html(element) {
                            Ok(table) => Block::Table(table),
                            Err(error) => {
                                diagnostics.push(Diagnostic::loss(
                                    span,
                                    format!("table structure cannot be represented: {error}"),
                                ));
                                Block::Html(element.html())
                            }
                        }
                    }
                    "pre" | "hr" | "ul" | "ol" | "dl" if can_use_native_block(element) => {
                        if name == "pre" {
                            for code in element
                                .child_elements()
                                .filter(|el| el.value().name() == "code")
                            {
                                for (key, value) in code.value().attrs() {
                                    diagnostics.push(Diagnostic::loss(locate(original, value),
                                        format!("preformatted code attribute {key} cannot be represented in wikitext")));
                                }
                            }
                        }
                        match core::parse_html(element, &parse_html_inlines) {
                            Ok(Some(block)) => Block::Core(block),
                            _ => Block::Html(element.html()),
                        }
                    }
                    _ => Block::Html(element.html()),
                };
                blocks.push(block);
            } else {
                push_html_inline(element, &mut paragraph);
            }
        } else if let Node::Text(text) = child.value() {
            inline::text(&mut paragraph, &normalized_html_text(text));
        }
    }
    if has_inline_content(&paragraph) {
        blocks.push(Block::Paragraph(paragraph));
    }
    Ok(())
}

fn has_inline_content(content: &[Inline]) -> bool {
    content.iter().any(|item| match item {
        Inline::Text(text) => !text.trim_ascii().is_empty(),
        _ => true,
    })
}

fn can_use_native_block(element: ElementRef<'_>) -> bool {
    if element.value().attrs().next().is_some() {
        return false;
    }
    element.descendent_elements().all(|child| {
        if matches!(
            child.value().name(),
            "ul" | "ol" | "li" | "dl" | "dt" | "dd"
        ) && child.value().attrs().next().is_some()
        {
            return false;
        }
        if !matches!(child.value().name(), "li" | "dt" | "dd") {
            return true;
        }
        let mut paragraphs = 0;
        let mut has_nested_list = false;
        for node in child.children() {
            match node.value() {
                Node::Element(e) if matches!(e.name(), "ol" | "ul" | "dl") => {
                    has_nested_list = true
                }
                Node::Element(e) if e.name() == "p" => {
                    paragraphs += 1;
                    if paragraphs > 1 || has_nested_list || e.attrs().next().is_some() {
                        return false;
                    }
                }
                Node::Text(text) if text.trim_ascii().is_empty() => {}
                Node::Text(_) | Node::Element(_) if has_nested_list => return false,
                _ => {}
            }
        }
        true
    })
}

fn wiki_attributes<'a>(attributes: impl Iterator<Item = (&'a str, &'a str)>) -> String {
    attributes
        .map(|(key, value)| {
            format!(
                " {key}=\"{}\"",
                html_policy::escape_wiki_attribute_value(value)
            )
        })
        .collect()
}

// Safe literal HTML keeps attributes or structures that have no native wiki
// spelling. Rewrite anchors to wiki syntax within these fragments.
pub(crate) fn html_to_wiki(source: &str) -> String {
    fn render_element(element: ElementRef<'_>, output: &mut String) {
        let name = element.value().name();
        if let Some(wiki) = wiki_links::media_to_wikitext(element)
            .or_else(|| wiki_links::media_link_to_wikitext(element))
        {
            output.push_str(&wiki);
            return;
        }
        if name == "img" {
            let alt = element.attr("alt").unwrap_or("");
            if let Some(src) = element
                .attr("src")
                .filter(|src| html_policy::external_url(src))
            {
                output.push_str(&format!(
                    "[{} {}]",
                    html_policy::escape_wiki_attribute_value(src),
                    escape_wikitext(if alt.is_empty() { src } else { alt })
                ));
            } else {
                output.push_str(&escape_wikitext(alt));
            }
            return;
        }
        if name == "a" {
            let attributes = wiki_attributes(
                element
                    .value()
                    .attrs()
                    .filter(|(key, _)| !matches!(*key, "href" | "rel" | "rev")),
            );
            if !attributes.is_empty() {
                output.push_str(&format!("<span{attributes}>"));
            }
            output.push_str(&render_inline(&html_link(element), Format::Wikitext));
            if !attributes.is_empty() {
                output.push_str("</span>");
            }
            return;
        }
        let name = match name {
            "section" | "figure" | "figcaption" => "div",
            name => name,
        };
        let wrapper = !matches!(name, "html" | "thead" | "tbody" | "tfoot");
        if wrapper {
            let attributes = wiki_attributes(element.value().attrs());
            output.push_str(&format!("<{name}{attributes}>"));
        }
        for child in element.children() {
            if let Some(element) = ElementRef::wrap(child) {
                render_element(element, output);
            } else if let Node::Text(text) = child.value() {
                output.push_str(&escape_wikitext(text));
            }
        }
        if wrapper && !matches!(name, "br" | "hr" | "wbr" | "meta" | "link") {
            output.push_str(&format!("</{name}>"));
        }
    }
    let mut output = String::new();
    render_element(Html::parse_fragment(source).root_element(), &mut output);
    output
}

pub(crate) fn embedded_html(source: &str, language: &str) -> String {
    let mut html = String::new();
    let mut rest = source;
    let mut preformatted = false;
    while !rest.is_empty() {
        if let Some((name, length, closing)) = html_policy::tag_info(rest) {
            html.push_str(&rest[..length]);
            rest = &rest[length..];
            if name == "pre" {
                preformatted = !closing;
            }
            continue;
        }

        let first = rest.chars().next().unwrap().len_utf8();
        let length = rest[first..]
            .find('<')
            .map_or(rest.len(), |next| next + first);
        let text = &rest[..length];
        if preformatted {
            html.push_str(text);
        } else if let Ok(parts) = inline::parse_wiki(text, language) {
            html.push_str(&render_inline(&parts, Format::Html));
        } else {
            html.push_str(text);
        }
        rest = &rest[length..];
    }
    Html::parse_fragment(&html).root_element().inner_html()
}
