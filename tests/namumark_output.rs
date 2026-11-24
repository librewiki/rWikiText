use rwikitext::{
    convert, convert_with_mode, ConversionMode, ConversionRequest, Format, WikiContext,
};

fn from_html(input: &str) -> rwikitext::ConversionResult {
    convert(ConversionRequest {
        source: Format::Html,
        target: Format::NamuMark,
        input,
        context: &WikiContext::default(),
    })
    .unwrap()
}

#[test]
fn html_heading_and_inline_code_have_namumark_syntax() {
    let result = from_html("<h2>제목 <b>굵게</b></h2><p><code>x &amp; y</code></p>");
    assert!(
        result.content.contains("== 제목 '''굵게''' =="),
        "{}",
        result.content
    );
    assert!(result.content.contains("{{{x & y}}}"), "{}", result.content);
    assert!(result.diagnostics.is_empty(), "{:?}", result.diagnostics);
}

#[test]
fn html_preformatted_and_top_level_lists_use_namumark_blocks() {
    let result = from_html("<pre>one\ntwo</pre><ul><li>apple</li><li>pear</li></ul>");
    assert!(
        result.content.contains("{{{\none\ntwo\n}}}"),
        "{}",
        result.content
    );
    assert!(
        result.content.contains(" * apple\n * pear"),
        "{}",
        result.content
    );
    assert!(result.diagnostics.is_empty(), "{:?}", result.diagnostics);
}

#[test]
fn nested_html_lists_are_reported_as_loss_in_strict_mode() {
    let input = "<ul><li>outer<ul><li>inner</li></ul></li></ul>";
    let request = ConversionRequest {
        source: Format::Html,
        target: Format::NamuMark,
        input,
        context: &WikiContext::default(),
    };
    assert!(convert_with_mode(request, ConversionMode::Strict).is_err());
}

#[test]
fn code_ending_in_closing_brace_reports_loss_and_keeps_text() {
    let input = "<p><code>{{Harvest}}</code></p>";
    let result = from_html(input);
    assert!(result.content.contains("Harvest"), "{}", result.content);
    assert!(result
        .diagnostics
        .iter()
        .any(|d| d.kind == rwikitext::DiagnosticKind::ConversionLoss));
    let request = ConversionRequest {
        source: Format::Html,
        target: Format::NamuMark,
        input,
        context: &WikiContext::default(),
    };
    assert!(convert_with_mode(request, ConversionMode::Strict).is_err());
}

#[test]
fn namumark_html_metadata_round_trip_keeps_exact_titles_and_sort_keys() {
    let context = WikiContext::default();
    for input in [
        "[[분류:orchard_tree|sort key]]",
        "#redirect orchard_tree#lower_part",
        "[[분류:orchard tree|sort key]]",
    ] {
        let first = convert_with_mode(
            ConversionRequest {
                source: Format::NamuMark,
                target: Format::Html,
                input,
                context: &context,
            },
            ConversionMode::Strict,
        )
        .unwrap();
        let second = convert_with_mode(
            ConversionRequest {
                source: Format::Html,
                target: Format::NamuMark,
                input: &first.content,
                context: &context,
            },
            ConversionMode::Strict,
        )
        .unwrap();
        assert_eq!(second.metadata, first.metadata, "{}", first.content);
        let third = convert_with_mode(
            ConversionRequest {
                source: Format::NamuMark,
                target: Format::Html,
                input: &second.content,
                context: &context,
            },
            ConversionMode::Strict,
        )
        .unwrap();
        assert_eq!(third.metadata, first.metadata, "{}", second.content);
    }
}

#[test]
fn code_ending_in_backslash_reports_loss_without_malformed_output() {
    let input = "<p><code>x\\</code></p>";
    let result = from_html(input);
    assert!(
        result
            .diagnostics
            .iter()
            .any(|d| d.kind == rwikitext::DiagnosticKind::ConversionLoss),
        "{:?}",
        result
    );
    let request = ConversionRequest {
        source: Format::Html,
        target: Format::NamuMark,
        input,
        context: &WikiContext::default(),
    };
    assert!(convert_with_mode(request, ConversionMode::Strict).is_err());
    let restored = convert(ConversionRequest {
        source: Format::NamuMark,
        target: Format::Html,
        input: &result.content,
        context: &WikiContext::default(),
    })
    .unwrap();
    assert!(restored.diagnostics.is_empty(), "{:?}", restored);
    assert!(restored.content.contains("x\\"), "{}", restored.content);
}

#[test]
fn category_backslashes_survive_namumark_html_round_trip() {
    let context = WikiContext::default();
    let input = r"[[분류:orchard\\grove|sort\\key]]";
    let first = convert_with_mode(
        ConversionRequest {
            source: Format::NamuMark,
            target: Format::Html,
            input,
            context: &context,
        },
        ConversionMode::Strict,
    )
    .unwrap();
    let second = convert_with_mode(
        ConversionRequest {
            source: Format::Html,
            target: Format::NamuMark,
            input: &first.content,
            context: &context,
        },
        ConversionMode::Strict,
    )
    .unwrap();
    let third = convert_with_mode(
        ConversionRequest {
            source: Format::NamuMark,
            target: Format::Html,
            input: &second.content,
            context: &context,
        },
        ConversionMode::Strict,
    )
    .unwrap();
    assert_eq!(third.metadata, first.metadata, "{}", second.content);
}

#[test]
fn preformatted_hash_line_round_trips_without_loss() {
    let context = WikiContext::default();
    let original = "<pre>#orchard</pre>";
    let converted = convert_with_mode(
        ConversionRequest {
            source: Format::Html,
            target: Format::NamuMark,
            input: original,
            context: &context,
        },
        ConversionMode::Strict,
    )
    .unwrap();
    assert!(converted.diagnostics.is_empty());
    let restored = convert_with_mode(
        ConversionRequest {
            source: Format::NamuMark,
            target: Format::Html,
            input: &converted.content,
            context: &context,
        },
        ConversionMode::Strict,
    )
    .unwrap();
    assert!(
        restored.content.contains("<pre>#orchard</pre>"),
        "{}",
        restored.content
    );
}
