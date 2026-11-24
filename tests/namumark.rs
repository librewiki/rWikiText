use rwikitext::{convert, ConversionRequest, Format, WikiContext};

#[test]
fn namumark_paragraph_converts_to_html() {
    let context = WikiContext::default();
    let result = convert(ConversionRequest {
        source: Format::NamuMark,
        target: Format::Html,
        input: "안녕 world",
        context: &context,
    }).unwrap();
    assert_eq!(result.content, "<p>안녕 world</p>");
    assert!(result.diagnostics.is_empty());
}

#[test]
fn namumark_heading_and_emphasis_convert_to_html() {
    let context = WikiContext::default();
    let result = convert(ConversionRequest { source: Format::NamuMark, target: Format::Html, input: "== 제목 ==\n\n'''bold''' and ''italic''", context: &context }).unwrap();
    assert_eq!(result.content, "<h2 id=\"제목\">제목</h2>\n<p><b>bold</b> and <i>italic</i></p>");
}

#[test]
fn namumark_links_preserve_title_and_label() {
    let context = WikiContext::default();
    let result = convert(ConversionRequest { source: Format::NamuMark, target: Format::Html, input: "[[orchard tree|visit]]", context: &context }).unwrap();
    assert_eq!(result.content, "<p><a rel=\"mw:WikiLink\" href=\"./orchard%20tree\">visit</a></p>");
    assert!(result.diagnostics.is_empty());
}

#[test]
fn unsupported_namumark_table_reports_original_utf8_span_and_strict_fails() {
    use rwikitext::{convert_with_mode, ConversionMode, DiagnosticKind};
    let context = WikiContext::default();
    let input = "한글\n\n||cell||";
    let request = ConversionRequest { source: Format::NamuMark, target: Format::Html, input, context: &context };
    let result = convert(request).unwrap();
    assert_eq!(result.diagnostics[0].kind, DiagnosticKind::ConversionLoss);
    assert_eq!(result.diagnostics[0].span, 8..16);
    assert!(result.content.contains("||cell||"));
    assert!(convert_with_mode(request, ConversionMode::Strict).is_err());
}

#[test]
fn unsupported_html_table_reports_loss_and_strict_rejects() {
    use rwikitext::{convert_with_mode, ConversionMode, DiagnosticKind};
    let context = WikiContext::default();
    let input = "<table><tr><td>cell</td></tr></table>";
    let request = ConversionRequest { source: Format::Html, target: Format::NamuMark, input, context: &context };
    let result = convert(request).unwrap();
    assert!(result.diagnostics.iter().any(|d| d.kind == DiagnosticKind::ConversionLoss && d.span == (0..input.len())));
    assert!(convert_with_mode(request, ConversionMode::Strict).is_err());
}

#[test]
fn namumark_literal_block_does_not_expand_macros() {
    let context = WikiContext::default();
    let input = "{{{\n[[A]]\n}}}";
    let result = convert(ConversionRequest { source: Format::NamuMark, target: Format::Html, input, context: &context }).unwrap();
    assert_eq!(result.content, "<pre>[[A]]</pre>");
    assert!(result.diagnostics.is_empty());
}

#[test]
fn namumark_internal_link_preserves_fragment() {
    let context = WikiContext::default();
    let result = convert(ConversionRequest { source: Format::NamuMark, target: Format::Html, input: "[[Orchard#Late fruit|visit]]", context: &context }).unwrap();
    assert_eq!(result.content, "<p><a rel=\"mw:WikiLink\" href=\"./Orchard#Late%20fruit\">visit</a></p>");
    assert!(result.diagnostics.is_empty());
}

#[test]
fn namumark_ignores_mediawiki_callbacks_and_expansion_limits() {
    use rwikitext::{convert_with_options, ConversionOptions, ExpansionLimits, LookupError, WikiData};
    struct NoLookup;
    impl WikiData for NoLookup {
        fn template_text(&self, _: &str) -> Result<Option<String>, LookupError> { panic!("template lookup") }
        fn page_exists(&self, _: &str) -> Result<Option<bool>, LookupError> { panic!("page lookup") }
        fn file_info(&self, _: &str) -> Result<Option<rwikitext::FileInfo>, LookupError> { panic!("file lookup") }
    }
    let context = WikiContext::default();
    let result = convert_with_options(
        ConversionRequest { source: Format::NamuMark, target: Format::Html, input: "[[Orchard|fruit]] and {{{literal}}}", context: &context },
        ConversionOptions { wiki_data: Some(&NoLookup), expansion_limits: ExpansionLimits { max_depth: 0, max_expansions: 0, max_output_bytes: 0 }, ..Default::default() }
    ).unwrap();
    assert_eq!(result.content, "<p><a rel=\"mw:WikiLink\" href=\"./Orchard\">fruit</a> and <code>literal</code></p>");
    assert!(result.diagnostics.is_empty());
}

#[test]
fn inline_categories_are_extracted_without_consuming_surrounding_text() {
    let context = WikiContext::default();
    let result = convert(ConversionRequest { source: Format::NamuMark, target: Format::Html, input: "A[[분류:Orchards|Q]][[분류:Fruit]] B", context: &context }).unwrap();
    assert_eq!(result.metadata.categories, vec!["Orchards", "Fruit"]);
    assert_eq!(result.metadata.category_sort_keys, vec![Some("Q".into()), None]);
    assert!(result.content.starts_with("<p>A B</p>"));
    assert!(result.diagnostics.is_empty());
}

#[test]
fn namumark_html_round_trip_preserves_internal_title_spelling() {
    let context = WikiContext::default();
    let input = "[[orchard tree|visit]]";
    let html = convert(ConversionRequest { source: Format::NamuMark, target: Format::Html, input, context: &context }).unwrap();
    let back = convert(ConversionRequest { source: Format::Html, target: Format::NamuMark, input: &html.content, context: &context }).unwrap();
    let again = convert(ConversionRequest { source: Format::NamuMark, target: Format::Html, input: &back.content, context: &context }).unwrap();
    assert_eq!(again.content, html.content);
}

#[test]
fn redirect_with_extra_body_reports_loss() {
    use rwikitext::{convert_with_mode, ConversionMode};
    let context = WikiContext::default();
    let input = "#redirect Orchard\nExtra text";
    let request = ConversionRequest { source: Format::NamuMark, target: Format::Html, input, context: &context };
    let result = convert(request).unwrap();
    assert!(result.diagnostics.iter().any(|d| d.span == (0..input.len())));
    assert!(result.content.contains("Extra text"));
    assert!(convert_with_mode(request, ConversionMode::Strict).is_err());
}

#[test]
fn namumark_literal_block_preserves_blank_lines() {
    let context = WikiContext::default();
    let input = "{{{\n\nseed\n\n}}}";
    let result = convert(ConversionRequest { source: Format::NamuMark, target: Format::Html, input, context: &context }).unwrap();
    assert_eq!(result.content, "<pre>\nseed\n</pre>");
}
