use crate::{
    core, expansion,
    html::{embedded_html, html_to_wiki, parse_gfm, parse_html},
    html_policy::{self, HtmlPolicy},
    metadata, namumark, namumark_output,
    render::{heading_id, render_inline, HtmlStyle, RenderTarget},
    spans, transclusion, wiki_links, wiki_markup, wikitext, Block, ConversionError,
    ConversionMode, ConversionOptions, ConversionRequest, ConversionResult, Diagnostic,
    DiagnosticKind, Document, Format, PageMetadata,
};

#[derive(Debug, Clone, Copy)]
struct PreviousList {
    kind: core::ListKind,
    alternate_marker: bool,
}

pub(crate) fn convert(
    request: ConversionRequest<'_>,
    options: ConversionOptions<'_>,
) -> Result<ConversionResult, ConversionError> {
    if !matches!(
        (request.source, request.target),
        (Format::Wikitext, Format::Gfm | Format::Html)
            | (Format::Gfm | Format::Html, Format::Wikitext)
            | (Format::NamuMark, Format::Wikitext | Format::Gfm | Format::Html)
            | (Format::Wikitext | Format::Gfm | Format::Html, Format::NamuMark)
    ) {
        return Err(ConversionError::new("unsupported conversion direction"));
    }

    let mut diagnostics = Vec::new();
    let mut markup = wiki_markup::Markup::default();
    let import_policy = if request.target == Format::NamuMark {
        HtmlPolicy::NamuMarkImport
    } else {
        HtmlPolicy::MediaWikiImport
    };
    let (document, mut page_metadata) = match request.source {
        Format::Wikitext => prepare_wikitext(request, options, &mut markup, &mut diagnostics)?,
        Format::NamuMark => namumark::parse(request.input, request.context, request.target, &mut diagnostics),
        Format::Html => {
            let metadata = if request.target == Format::NamuMark {
                metadata::extract_html_for_namumark(request.input, request.context)?
            } else {
                metadata::extract_html(request.input, request.context)?
            };
            let document = parse_html(
                &metadata.body,
                request.input,
                &metadata.ranges,
                &mut markup,
                options.wiki_data,
                &mut diagnostics,
                import_policy,
            )?;
            (document, metadata.metadata)
        }
        Format::Gfm => (
            parse_gfm(request.input, options.wiki_data, &mut diagnostics, import_policy)?,
            PageMetadata::default(),
        ),
    };

    if request.target == Format::NamuMark {
        if request.source == Format::Wikitext {
            for sort in page_metadata.category_sort_keys.iter_mut().flatten() {
                *sort = html_escape::decode_html_entities(sort).into_owned();
            }
        }
        if page_metadata.display_title.is_some() {
            diagnostics.push(Diagnostic::loss(
                0..request.input.len(),
                "NamuMark cannot carry display title metadata",
            ));
        }
    }
    let separator = if request.target == Format::Html {
        "\n"
    } else {
        "\n\n"
    };
    let mut headings = std::collections::HashSet::new();
    let mut previous_list: Option<PreviousList> = None;
    let mut blocks = Vec::new();
    let render_target = match request.target {
        Format::Wikitext => Some(RenderTarget::Wikitext),
        Format::Gfm => Some(RenderTarget::Gfm),
        Format::Html => Some(RenderTarget::Html(if request.source == Format::NamuMark {
            HtmlStyle::Semantic
        } else {
            HtmlStyle::Parsoid
        })),
        Format::NamuMark => None,
    };
    for block in document.blocks {
        let Some(target) = render_target else {
            let (rendered, loss) = namumark_output::render_block(&block);
            if loss {
                diagnostics.push(Diagnostic::loss(
                    0..request.input.len(),
                    "NamuMark cannot represent this source structure or formatting",
                ));
            }
            if !rendered.is_empty() {
                blocks.push(rendered);
            }
            continue;
        };
        let current_list = if let Block::Core(block) = &block {
            core::list_kind(block).map(|kind| PreviousList {
                kind,
                alternate_marker: previous_list
                    .is_some_and(|prev| prev.kind == kind && !prev.alternate_marker),
            })
        } else {
            None
        };
        previous_list = current_list;
        let rendered = match &block {
            Block::Paragraph(text) => match target {
                RenderTarget::Html(_) => format!("<p>{}</p>", render_inline(text, target)),
                format => render_inline(text, format),
            },
            Block::Heading { level, text } => match target {
                RenderTarget::Wikitext => {
                    let delimiter = "=".repeat(*level as usize);
                    format!(
                        "{delimiter} {} {delimiter}",
                        render_inline(text, RenderTarget::Wikitext)
                    )
                }
                RenderTarget::Gfm => format!(
                    "{} {}",
                    "#".repeat(*level as usize),
                    render_inline(text, RenderTarget::Gfm)
                ),
                RenderTarget::Html(_) => {
                    let id = heading_id(text, &mut headings);
                    format!(
                        "<h{level} id=\"{}\">{}</h{level}>",
                        html_escape::encode_double_quoted_attribute(&id),
                        render_inline(text, target)
                    )
                }
            },
            Block::Core(block) => core::render(
                block,
                target,
                current_list.is_some_and(|list| list.alternate_marker),
            ),
            Block::Table(table) => table.render(target)?,
            Block::Html(html) if target == RenderTarget::Wikitext => html_to_wiki(html),
            Block::Html(html) => embedded_html(html, request.context.site_language),
        };
        if !rendered.is_empty() {
            blocks.push(rendered);
        }
    }
    let encoded_metadata = match request.target {
        Format::Wikitext if request.source == Format::NamuMark => {
            namumark::render_wikitext_metadata(&page_metadata)
        }
        Format::Wikitext => metadata::render_wikitext(&page_metadata),
        Format::Html if request.source == Format::NamuMark => {
            metadata::render_html_for_namumark(&page_metadata)
        }
        Format::Html => metadata::render_html(&page_metadata),
        Format::Gfm => String::new(),
        Format::NamuMark => {
            let (text, loss) = namumark_output::render_metadata(&page_metadata);
            if loss && page_metadata.display_title.is_none() {
                diagnostics.push(Diagnostic::loss(
                    0..request.input.len(),
                    "NamuMark cannot represent this page metadata",
                ));
            }
            text
        }
    };
    if !encoded_metadata.is_empty() {
        if page_metadata.redirect_target.is_some()
            && matches!(request.target, Format::Wikitext | Format::NamuMark)
        {
            blocks.insert(0, encoded_metadata);
        } else {
            blocks.push(encoded_metadata);
        }
    }
    let content = markup.render(blocks.join(separator), &mut diagnostics);
    diagnostics.sort_by_key(|diagnostic| diagnostic.span.start);
    if options.mode == ConversionMode::Strict
        && diagnostics
            .iter()
            .any(|diagnostic| diagnostic.kind == DiagnosticKind::ConversionLoss)
    {
        return Err(ConversionError::strict_loss(diagnostics));
    }
    Ok(ConversionResult {
        content,
        metadata: page_metadata,
        diagnostics,
    })
}

fn prepare_wikitext(
    request: ConversionRequest<'_>,
    options: ConversionOptions<'_>,
    markup: &mut wiki_markup::Markup,
    diagnostics: &mut Vec<Diagnostic>,
) -> Result<(Document, PageMetadata), ConversionError> {
    let expanded = expansion::expand(
        request.input,
        request.context,
        options.wiki_data,
        options.expansion_limits,
    )?;
    if request.target == Format::NamuMark {
        for invocation in &expanded.invocations {
            diagnostics.push(Diagnostic::loss(
                invocation.source_range.clone(),
                "NamuMark cannot preserve MediaWiki transclusion identity",
            ));
        }
    }
    let expanded_span = |span| expanded.original_span(span);
    let metadata = metadata::extract_wikitext(&expanded.text, request.input, request.context)?;
    let body_span = |span| expanded_span(spans::original_range(span, &metadata.ranges));
    let invocation_spans: Vec<_> = expanded
        .invocations
        .iter()
        .map(|call| spans::range_after_removals(call.expanded_range.clone(), &metadata.ranges))
        .collect();
    let sanitized = html_policy::sanitize_wikitext(&metadata.body, &invocation_spans);
    let sanitized_span = |span| body_span(sanitized.original_span(span));
    let project =
        |span| sanitized.output_span(spans::range_after_removals(span, &metadata.ranges));

    if request.target == Format::Gfm {
        let metadata_loss = |span, kind| {
            Diagnostic::loss(span, format!("saving the GFM body alone loses the {kind}"))
        };
        diagnostics.extend(
            metadata
                .locations
                .iter()
                .map(|at| metadata_loss(expanded_span(at.start..at.end), at.kind)),
        );
    }
    diagnostics.extend(expanded.diagnostics.iter().cloned());
    diagnostics.extend(sanitized.diagnostics.iter().cloned().map(|mut diagnostic| {
        diagnostic.span = body_span(diagnostic.span);
        diagnostic
    }));

    let links = wiki_links::scan(
        &sanitized.text,
        request.context.site_language,
        request.target,
        options.wiki_data,
    )
    .map_err(|(range, error)| {
        let target = sanitized.text[range.clone()]
            .strip_prefix("[[")
            .and_then(|link| link.split(['|', ']']).next())
            .unwrap_or("");
        ConversionError::lookup(sanitized_span(range), "link", target, error)
    })?;
    if request.target == Format::Html {
        let tables = wikitext::wiki_table_ranges(&sanitized.text);
        for call in &expanded.invocations {
            let span = project(call.expanded_range.clone());
            let source = call.source_range.clone();
            let nearby = links.partition_point(|link| link.range.end <= span.start);
            let overlapping = links[nearby..]
                .iter()
                .take_while(|link| link.range.start < span.end);
            if overlapping.clone().any(|link| {
                (link.range.start < span.start && span.start < link.range.end)
                    || (link.range.start < span.end && span.end < link.range.end)
            }) || wikitext::inside_wiki_token(&sanitized.text, span.start)
                || wikitext::inside_wiki_token(&sanitized.text, span.end)
            {
                diagnostics.push(Diagnostic::loss(
                    source,
                    "cannot attach Parsoid metadata to a partial link or tag",
                ));
                continue;
            }
            let fragment = &sanitized.text[span.clone()];
            let block = wikitext::looks_like_block(fragment)
                || overlapping
                    .clone()
                    .any(|link| link.range == span && link.html.starts_with("<figure"));
            let aligned = wikitext::is_line_aligned(&sanitized.text, &span);
            let joins_linktrail = fragment.ends_with("]]")
                && sanitized.text[span.end..].starts_with(|ch: char| ch.is_ascii_lowercase());
            let joins_block = block
                && if aligned {
                    wikitext::continues_wiki_block(&sanitized.text, &span)
                } else {
                    fragment.contains('\n') || fragment.trim_start().starts_with('<')
                };
            let delimiter = !fragment.trim().is_empty()
                && fragment.trim().bytes().all(|ch| matches!(ch, b'|' | b'='));
            let table = tables.partition_point(|range| range.start <= span.start);
            let table = table > 0
                && tables[table - 1].contains(&span.start)
                && wikitext::table_delimiter(&sanitized.text, &span);
            if joins_block
                || joins_linktrail
                || delimiter && (table || wikitext::heading_delimiter(&sanitized.text, &span))
            {
                diagnostics.push(Diagnostic::loss(
                    source,
                    "cannot attach Parsoid metadata to this combined block or link",
                ));
                continue;
            }
            let block = (block || fragment.is_empty()) && aligned;
            let (mw, parsoid) = transclusion::metadata(call);
            markup.group(span, source, block, mw, parsoid);
        }
    }
    for link in links {
        if request.target == Format::NamuMark {
            diagnostics.push(Diagnostic::loss(
                sanitized_span(link.range.clone()),
                "NamuMark cannot represent MediaWiki file or link metadata",
            ));
            continue;
        }
        if let Some(loss) = link.loss {
            diagnostics.push(Diagnostic::loss(sanitized_span(link.range.clone()), loss));
        }
        let block = link.html.starts_with("<figure");
        let isolate = block && !wikitext::is_line_aligned(&sanitized.text, &link.range);
        markup.replace(link.range, link.html, block, isolate);
    }
    for (span, title) in &expanded.missing_links {
        let projected = project(span.clone());
        if wikitext::inside_wiki_token(&sanitized.text, projected.start)
            || wikitext::inside_wiki_token(&sanitized.text, projected.end)
        {
            diagnostics.push(Diagnostic::loss(
                expanded_span(span.clone()),
                "cannot place a missing-template link inside another link or tag",
            ));
            continue;
        }
        markup.replace(
            projected,
            wiki_markup::redlink(title, request.target),
            false,
            false,
        );
    }
    let marked = markup.apply(&sanitized.text);
    let document = wikitext::parse_wikitext(
        &marked.text,
        request.context.site_language,
        &|span| sanitized_span(marked.original_span(span)),
        diagnostics,
    );

    Ok((document, metadata.metadata))
}
