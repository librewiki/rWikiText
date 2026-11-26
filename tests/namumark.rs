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
    assert_eq!(result.content, "<h2 id=\"제목\">제목</h2>\n<p><strong>bold</strong> and <em>italic</em></p>");
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

#[test]
fn mediawiki_rejects_unrepresentable_namumark_link_title() {
    use rwikitext::{convert_with_mode, ConversionMode};
    let context = WikiContext::default(); let input = r"[[Orchard\|Gate|visit]]";
    let request = ConversionRequest { source: Format::NamuMark, target: Format::Wikitext, input, context: &context };
    let result = convert(request).unwrap();
    assert!(result.diagnostics.iter().any(|d| d.span == (0..input.len())));
    assert!(!result.content.contains("[[Orchard|Gate|visit]]"));
    assert!(convert_with_mode(request, ConversionMode::Strict).is_err());
}

#[test]
fn mediawiki_metadata_never_activates_namumark_title_markup() {
    use rwikitext::{convert_with_mode, ConversionMode};
    let context = WikiContext::default(); let input = "[[분류:Orchard{{bad}}]]";
    let request = ConversionRequest { source: Format::NamuMark, target: Format::Wikitext, input, context: &context };
    let result = convert(request).unwrap();
    assert_eq!(result.metadata.categories, vec!["Orchard{{bad}}"]);
    assert!(result.diagnostics.iter().any(|d| d.span == (0..input.len())));
    assert!(!result.content.contains("{{bad}}"));
    assert!(convert_with_mode(request, ConversionMode::Strict).is_err());
}

#[test]
fn namumark_parent_relative_link_resolves_without_trailing_slash() {
    use rwikitext::{convert_with_mode, ConversionMode};
    let context = WikiContext { page_title: "Parent/Child", ..WikiContext::default() };
    let cases = [
        (Format::Html, "<p><a rel=\"mw:WikiLink\" href=\"./Parent\">../</a></p>"),
        (Format::Wikitext, "[[Parent|../]]"),
        (Format::Gfm, "[\\.\\.\\/](<./Parent>)"),
    ];
    for (target, expected) in cases {
        let request = ConversionRequest { source: Format::NamuMark, target, input: "[[../]]", context: &context };
        let result = convert(request).unwrap();
        assert_eq!(result.content, expected, "target {target:?}");
        assert!(result.diagnostics.is_empty(), "target {target:?}");
        assert!(convert_with_mode(request, ConversionMode::Strict).is_ok(), "target {target:?}");
    }
}

#[test]
fn namumark_parent_relative_link_with_sibling_segment_resolves() {
    use rwikitext::{convert_with_mode, ConversionMode};
    let context = WikiContext { page_title: "Parent/Child", ..WikiContext::default() };
    let cases = [
        (Format::Html, "<p><a rel=\"mw:WikiLink\" href=\"./Parent/Sibling\">../Sibling</a></p>"),
        (Format::Wikitext, "[[Parent/Sibling|../Sibling]]"),
        (Format::Gfm, "[\\.\\.\\/Sibling](<./Parent/Sibling>)"),
    ];
    for (target, expected) in cases {
        let request = ConversionRequest { source: Format::NamuMark, target, input: "[[../Sibling]]", context: &context };
        let result = convert(request).unwrap();
        assert_eq!(result.content, expected, "target {target:?}");
        assert!(result.diagnostics.is_empty(), "target {target:?}");
        assert!(convert_with_mode(request, ConversionMode::Strict).is_ok(), "target {target:?}");
    }
}

#[test]
fn namumark_parent_relative_link_on_top_level_title_stays_current() {
    let context = WikiContext { page_title: "Parent", ..WikiContext::default() };
    let result = convert(ConversionRequest { source: Format::NamuMark, target: Format::Html, input: "[[../]]", context: &context }).unwrap();
    assert_eq!(result.content, "<p><a rel=\"mw:WikiLink\" href=\"./Parent\">../</a></p>");
    assert!(result.diagnostics.is_empty());
}

#[test]
fn namumark_automatic_section_anchors_are_excluded_with_loss() {
    use rwikitext::{convert_with_mode, ConversionMode, DiagnosticKind};
    let context = WikiContext::default();
    for input in ["[[#s-2]]", "[[Orchard#s-1]]"] {
        let request = ConversionRequest { source: Format::NamuMark, target: Format::Html, input, context: &context };
        let result = convert(request).unwrap();
        assert_eq!(result.diagnostics.len(), 1, "input {input}");
        assert_eq!(result.diagnostics[0].kind, DiagnosticKind::ConversionLoss, "input {input}");
        assert_eq!(result.diagnostics[0].span, 0..input.len(), "input {input}");
        assert!(!result.content.contains("<a "), "input {input}");
        assert!(result.content.contains(input), "input {input}");
        assert!(convert_with_mode(request, ConversionMode::Strict).is_err(), "input {input}");
    }
}

#[test]
fn namumark_subnumbered_section_anchor_is_excluded() {
    use rwikitext::{convert_with_mode, ConversionMode, DiagnosticKind};
    let context = WikiContext::default();
    let input = "[[Orchard#s-1.3|visit]]";
    let request = ConversionRequest { source: Format::NamuMark, target: Format::Html, input, context: &context };
    let result = convert(request).unwrap();
    assert_eq!(result.diagnostics.len(), 1);
    assert_eq!(result.diagnostics[0].kind, DiagnosticKind::ConversionLoss);
    assert_eq!(result.diagnostics[0].span, 0..input.len());
    assert!(!result.content.contains("<a "));
    assert!(result.content.contains(input));
    assert!(convert_with_mode(request, ConversionMode::Strict).is_err());
}

#[test]
fn namumark_explicit_non_automatic_fragments_remain_active() {
    use rwikitext::{convert_with_mode, ConversionMode, DiagnosticKind};
    let context = WikiContext::default();
    let cases = [
        ("[[Orchard#s-note|note]]", "<p><a rel=\"mw:WikiLink\" href=\"./Orchard#s-note\">note</a></p>"),
        ("[[Orchard#s-1x]]", "<p><a rel=\"mw:WikiLink\" href=\"./Orchard#s-1x\">Orchard#s-1x</a></p>"),
        ("[[Orchard#s-1.]]", "<p><a rel=\"mw:WikiLink\" href=\"./Orchard#s-1.\">Orchard#s-1.</a></p>"),
        ("[[Orchard#s-]]", "<p><a rel=\"mw:WikiLink\" href=\"./Orchard#s-\">Orchard#s-</a></p>"),
    ];
    for (input, expected) in cases {
        let request = ConversionRequest { source: Format::NamuMark, target: Format::Html, input, context: &context };
        let result = convert(request).unwrap();
        assert_eq!(result.content, expected, "input {input}");
        assert!(
            result.diagnostics.iter().all(|d| d.kind != DiagnosticKind::ConversionLoss),
            "input {input}: {:?}", result.diagnostics
        );
        assert!(convert_with_mode(request, ConversionMode::Strict).is_ok(), "input {input}");
    }
}

#[test]
fn namumark_parent_relative_fragment_resolves_without_trailing_slash() {
    use rwikitext::{convert_with_mode, ConversionMode};
    let context = WikiContext {
        page_title: "Parent/Child",
        ..WikiContext::default()
    };
    let cases = [
        (
            "[[../#fragment]]",
            Format::Html,
            "<p><a rel=\"mw:WikiLink\" href=\"./Parent#fragment\">../#fragment</a></p>",
        ),
        (
            "[[../#fragment]]",
            Format::Wikitext,
            "[[Parent#fragment|../&#35;fragment]]",
        ),
        (
            "[[../#fragment]]",
            Format::Gfm,
            "[\\.\\.\\/\\#fragment](<./Parent#fragment>)",
        ),
        (
            "[[../#s-note]]",
            Format::Html,
            "<p><a rel=\"mw:WikiLink\" href=\"./Parent#s-note\">../#s-note</a></p>",
        ),
        (
            "[[../#s-note]]",
            Format::Wikitext,
            "[[Parent#s-note|../&#35;s&#45;note]]",
        ),
        (
            "[[../#s-note]]",
            Format::Gfm,
            "[\\.\\.\\/\\#s\\-note](<./Parent#s-note>)",
        ),
    ];
    for (input, target, expected) in cases {
        let request = ConversionRequest {
            source: Format::NamuMark,
            target,
            input,
            context: &context,
        };
        let result = convert(request).unwrap();
        assert_eq!(result.content, expected, "target {target:?} input {input}");
        assert!(
            result.diagnostics.is_empty(),
            "target {target:?} input {input}"
        );
        assert!(
            convert_with_mode(request, ConversionMode::Strict).is_ok(),
            "target {target:?} input {input}"
        );
    }
}

#[test]
fn namumark_parent_relative_fragment_on_top_level_title_resolves() {
    use rwikitext::{convert_with_mode, ConversionMode};
    let context = WikiContext {
        page_title: "Parent",
        ..WikiContext::default()
    };
    let cases = [
        (
            Format::Html,
            "<p><a rel=\"mw:WikiLink\" href=\"./Parent#fragment\">../#fragment</a></p>",
        ),
        (Format::Wikitext, "[[Parent#fragment|../&#35;fragment]]"),
        (Format::Gfm, "[\\.\\.\\/\\#fragment](<./Parent#fragment>)"),
    ];
    for (target, expected) in cases {
        let request = ConversionRequest {
            source: Format::NamuMark,
            target,
            input: "[[../#fragment]]",
            context: &context,
        };
        let result = convert(request).unwrap();
        assert_eq!(result.content, expected, "target {target:?}");
        assert!(result.diagnostics.is_empty(), "target {target:?}");
        assert!(
            convert_with_mode(request, ConversionMode::Strict).is_ok(),
            "target {target:?}"
        );
    }
}

#[test]
fn namumark_parent_relative_automatic_anchor_is_excluded_before_resolution() {
    use rwikitext::{convert_with_mode, ConversionMode, DiagnosticKind};
    let cases = [
        (Format::Html, "<p>[[../#s-1]]</p>"),
        (Format::Wikitext, "&#91;&#91;../&#35;s&#45;1&#93;&#93;"),
        (Format::Gfm, "\\[\\[\\.\\.\\/\\#s\\-1\\]\\]"),
    ];
    for page_title in ["Parent/Child", ""] {
        let context = WikiContext {
            page_title,
            ..WikiContext::default()
        };
        for (target, expected) in cases {
            let request = ConversionRequest {
                source: Format::NamuMark,
                target,
                input: "[[../#s-1]]",
                context: &context,
            };
            let result = convert(request).unwrap();
            assert_eq!(
                result.content, expected,
                "target {target:?} page_title {page_title:?}"
            );
            assert_eq!(
                result.diagnostics.len(),
                1,
                "target {target:?} page_title {page_title:?}"
            );
            assert_eq!(
                result.diagnostics[0].kind,
                DiagnosticKind::ConversionLoss,
                "target {target:?} page_title {page_title:?}"
            );
            assert_eq!(
                result.diagnostics[0].span,
                0..11,
                "target {target:?} page_title {page_title:?}"
            );
            assert!(
                !result
                    .diagnostics
                    .iter()
                    .any(|d| d.message.contains("relative link requires page title")),
                "target {target:?} page_title {page_title:?}"
            );
            assert!(
                convert_with_mode(request, ConversionMode::Strict).is_err(),
                "target {target:?} page_title {page_title:?}"
            );
        }
    }
}

#[test]
fn namumark_child_relative_fragment_resolves_without_trailing_slash() {
    use rwikitext::{convert_with_mode, ConversionMode};
    let context = WikiContext {
        page_title: "Parent/Child",
        ..WikiContext::default()
    };
    let cases = [
        (
            Format::Html,
            "<p><a rel=\"mw:WikiLink\" href=\"./Parent/Child#fragment\">/#fragment</a></p>",
        ),
        (Format::Wikitext, "[[Parent/Child#fragment|/&#35;fragment]]"),
        (Format::Gfm, "[\\/\\#fragment](<./Parent/Child#fragment>)"),
    ];
    for (target, expected) in cases {
        let request = ConversionRequest {
            source: Format::NamuMark,
            target,
            input: "[[/#fragment]]",
            context: &context,
        };
        let result = convert(request).unwrap();
        assert_eq!(result.content, expected, "target {target:?}");
        assert!(result.diagnostics.is_empty(), "target {target:?}");
        assert!(
            convert_with_mode(request, ConversionMode::Strict).is_ok(),
            "target {target:?}"
        );
    }
}

#[test]
fn namumark_child_relative_link_without_child_resolves_to_current_page() {
    use rwikitext::{convert_with_mode, ConversionMode};
    let context = WikiContext {
        page_title: "Parent/Child",
        ..WikiContext::default()
    };
    let cases = [
        (
            Format::Html,
            "<p><a rel=\"mw:WikiLink\" href=\"./Parent/Child\">/</a></p>",
        ),
        (Format::Wikitext, "[[Parent/Child|/]]"),
        (Format::Gfm, "[\\/](<./Parent/Child>)"),
    ];
    for (target, expected) in cases {
        let request = ConversionRequest {
            source: Format::NamuMark,
            target,
            input: "[[/]]",
            context: &context,
        };
        let result = convert(request).unwrap();
        assert_eq!(result.content, expected, "target {target:?}");
        assert!(result.diagnostics.is_empty(), "target {target:?}");
        assert!(
            convert_with_mode(request, ConversionMode::Strict).is_ok(),
            "target {target:?}"
        );
    }
}

#[test]
fn namumark_escaped_hash_is_not_an_automatic_section_anchor() {
    use rwikitext::{convert_with_mode, ConversionMode};
    let context = WikiContext::default();
    let cases = [
        (
            Format::Html,
            "<p><a rel=\"mw:WikiLink\" href=\"./Orchard#s-1\">Orchard#s-1</a></p>",
        ),
        (Format::Wikitext, "[[Orchard#s-1|Orchard&#35;s&#45;1]]"),
        (Format::Gfm, "[Orchard\\#s\\-1](<./Orchard#s-1>)"),
    ];
    for (target, expected) in cases {
        let request = ConversionRequest {
            source: Format::NamuMark,
            target,
            input: r"[[Orchard\#s-1]]",
            context: &context,
        };
        let result = convert(request).unwrap();
        assert_eq!(result.content, expected, "target {target:?}");
        assert!(result.diagnostics.is_empty(), "target {target:?}");
        assert!(
            convert_with_mode(request, ConversionMode::Strict).is_ok(),
            "target {target:?}"
        );
    }
}

#[test]
fn namumark_relative_fragment_regression_guards_stay_byte_identical() {
    use rwikitext::{convert_with_mode, ConversionMode};
    let context = WikiContext {
        page_title: "Parent/Child",
        ..WikiContext::default()
    };
    let cases = [
        (
            "[[../Sibling#fragment]]",
            Format::Html,
            "<p><a rel=\"mw:WikiLink\" href=\"./Parent/Sibling#fragment\">../Sibling#fragment</a></p>",
        ),
        (
            "[[../Sibling#fragment]]",
            Format::Wikitext,
            "[[Parent/Sibling#fragment|../Sibling&#35;fragment]]",
        ),
        (
            "[[../Sibling#fragment]]",
            Format::Gfm,
            "[\\.\\.\\/Sibling\\#fragment](<./Parent/Sibling#fragment>)",
        ),
        (
            "[[/Child#fragment]]",
            Format::Html,
            "<p><a rel=\"mw:WikiLink\" href=\"./Parent/Child/Child#fragment\">/Child#fragment</a></p>",
        ),
        (
            "[[/Child#fragment]]",
            Format::Wikitext,
            "[[Parent/Child/Child#fragment|/Child&#35;fragment]]",
        ),
        (
            "[[/Child#fragment]]",
            Format::Gfm,
            "[\\/Child\\#fragment](<./Parent/Child/Child#fragment>)",
        ),
    ];
    for (input, target, expected) in cases {
        let request = ConversionRequest {
            source: Format::NamuMark,
            target,
            input,
            context: &context,
        };
        let result = convert(request).unwrap();
        assert_eq!(result.content, expected, "target {target:?} input {input}");
        assert!(
            result.diagnostics.is_empty(),
            "target {target:?} input {input}"
        );
        assert!(
            convert_with_mode(request, ConversionMode::Strict).is_ok(),
            "target {target:?} input {input}"
        );
    }
}

#[test]
fn namumark_mixed_unordered_and_ordered_lists_emit_loss_and_fail_strict() {
    use rwikitext::{convert_with_mode, ConversionMode, DiagnosticKind};
    let context = WikiContext::default();
    for (input, expected_span) in [
        (" * a\n 1. b", 5..10),
        (" 1. a\n * b", 6..10),
        ("배\n\n * a\n 1. b", 10..15),
    ] {
        let request = ConversionRequest {
            source: Format::NamuMark,
            target: Format::Html,
            input,
            context: &context,
        };
        let result = convert(request).unwrap();
        assert!(
            result.diagnostics.iter().any(|d| {
                d.kind == DiagnosticKind::ConversionLoss && d.span == expected_span
            }),
            "input {input:?}: {:?}",
            result.diagnostics
        );
        assert!(result.content.contains("a"), "input {input:?}");
        assert!(result.content.contains("b"), "input {input:?}");
        assert!(
            convert_with_mode(request, ConversionMode::Strict).is_err(),
            "input {input:?}"
        );
    }
}

#[test]
fn namumark_alternate_list_numbering_and_nested_lists_emit_loss_and_fail_strict() {
    use rwikitext::{convert_with_mode, ConversionMode, DiagnosticKind};
    let context = WikiContext::default();
    for input in [
        " a. alpha",
        " A. upper",
        " i. roman",
        " I. ROMAN",
        " 2. second",
        " 10. tenth",
        " 1.#5 start five",
        " I.#11 start eleven",
        "   * deeply nested",
    ] {
        let request = ConversionRequest {
            source: Format::NamuMark,
            target: Format::Html,
            input,
            context: &context,
        };
        let result = convert(request).unwrap();
        assert!(
            result.diagnostics.iter().any(|d| {
                d.kind == DiagnosticKind::ConversionLoss && d.span == (0..input.len())
            }),
            "input {input:?}: {:?}",
            result.diagnostics
        );
        assert!(
            result.content.contains(input.trim()),
            "input {input:?}: {}",
            result.content
        );
        assert!(
            convert_with_mode(request, ConversionMode::Strict).is_err(),
            "input {input:?}"
        );
    }
}

#[test]
fn namumark_list_continuation_lines_emit_loss_and_fail_strict() {
    use rwikitext::{convert_with_mode, ConversionMode, DiagnosticKind};
    let context = WikiContext::default();
    let input = " * first\n continued\n still continued";
    let request = ConversionRequest {
        source: Format::NamuMark,
        target: Format::Html,
        input,
        context: &context,
    };
    let result = convert(request).unwrap();
    let losses: Vec<_> = result
        .diagnostics
        .iter()
        .filter(|d| d.kind == DiagnosticKind::ConversionLoss)
        .collect();
    assert_eq!(losses.len(), 2, "{:?}", result.diagnostics);
    assert_eq!(losses[0].span, 9..19);
    assert_eq!(losses[1].span, 20..36);
    assert!(result.content.contains("first"));
    assert!(result.content.contains("continued"));
    assert!(result.content.contains("still continued"));
    assert!(convert_with_mode(request, ConversionMode::Strict).is_err());
}

#[test]
fn namumark_basic_single_level_lists_and_separated_lists_convert_cleanly() {
    use rwikitext::{convert_with_mode, ConversionMode};
    let context = WikiContext::default();
    for (input, expected) in [
        (" * Apricot\n * Quince", "<ul><li>Apricot</li><li>Quince</li></ul>"),
        (" 1. First\n 1. Second", "<ol><li>First</li><li>Second</li></ol>"),
        (
            " * Apricot\n\n 1. First",
            "<ul><li>Apricot</li></ul>\n<ol><li>First</li></ol>",
        ),
        (
            " * Apricot\nAfter list",
            "<ul><li>Apricot</li></ul>\n<p>After list</p>",
        ),
    ] {
        let request = ConversionRequest {
            source: Format::NamuMark,
            target: Format::Html,
            input,
            context: &context,
        };
        let result = convert(request).unwrap();
        assert_eq!(result.content, expected, "input {input:?}");
        assert!(
            result.diagnostics.is_empty(),
            "input {input:?}: {:?}",
            result.diagnostics
        );
        assert!(
            convert_with_mode(request, ConversionMode::Strict).is_ok(),
            "input {input:?}"
        );
    }
}

#[test]
fn namumark_html_emphasis_uses_semantic_tags_while_wikitext_preserves_parsoid_tags() {
    use rwikitext::{convert_with_mode, ConversionMode};
    let context = WikiContext::default();
    let namu_input = "== '''Title''' ==\n\n * ''item''\n\n'''''both''''' and [[Orchard|'''visit''']]";
    let namu_html = convert_with_mode(
        ConversionRequest {
            source: Format::NamuMark,
            target: Format::Html,
            input: namu_input,
            context: &context,
        },
        ConversionMode::Strict,
    )
    .unwrap();
    assert!(
        namu_html.content.contains("<strong>Title</strong>"),
        "{}",
        namu_html.content
    );
    assert!(
        namu_html.content.contains("<li><em>item</em></li>"),
        "{}",
        namu_html.content
    );
    assert!(
        namu_html.content.contains("<em><strong>both</strong></em>"),
        "{}",
        namu_html.content
    );
    assert!(
        namu_html.content.contains("<strong>visit</strong></a>"),
        "{}",
        namu_html.content
    );
    assert!(!namu_html.content.contains("<b>"), "{}", namu_html.content);
    assert!(!namu_html.content.contains("<i>"), "{}", namu_html.content);

    let back = convert_with_mode(
        ConversionRequest {
            source: Format::Html,
            target: Format::NamuMark,
            input: &namu_html.content,
            context: &context,
        },
        ConversionMode::Strict,
    )
    .unwrap();
    let round_trip = convert_with_mode(
        ConversionRequest {
            source: Format::NamuMark,
            target: Format::Html,
            input: &back.content,
            context: &context,
        },
        ConversionMode::Strict,
    )
    .unwrap();
    assert_eq!(round_trip.content, namu_html.content);

    let wiki_html = convert_with_mode(
        ConversionRequest {
            source: Format::Wikitext,
            target: Format::Html,
            input: "'''bold''' and ''italic''",
            context: &context,
        },
        ConversionMode::Strict,
    )
    .unwrap();
    assert_eq!(wiki_html.content, "<p><b>bold</b> and <i>italic</i></p>");
}
