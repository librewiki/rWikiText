//! NamuMark title spelling follows its own profile across the HTML transport.
use rwikitext::{
    convert, convert_with_mode, ConversionMode, ConversionRequest, ConversionResult,
    DiagnosticKind, Format, WikiContext,
};
use scraper::Html;

fn run(source: Format, target: Format, input: &str) -> ConversionResult {
    convert(ConversionRequest {
        source,
        target,
        input,
        context: &WikiContext::default(),
    })
    .unwrap()
}

#[test]
fn category_name_and_sort_key_keep_literal_spelling_through_html() {
    for (input, name, sort_key) in [
        ("[[분류:orchard_tree|quince_tree]]", "orchard_tree", "quince_tree"),
        (r"[[분류:Orchard\\Gate|Quince\\Plot]]", r"Orchard\Gate", r"Quince\Plot"),
    ] {
        let original = run(Format::NamuMark, Format::Html, input);
        assert!(original.diagnostics.is_empty(), "{:?}", original.diagnostics);
        assert_eq!(original.metadata.categories, vec![name.to_owned()]);
        assert_eq!(original.metadata.category_sort_keys, vec![Some(sort_key.to_owned())]);
        let imported = run(Format::Html, Format::NamuMark, &original.content);
        assert!(imported.diagnostics.is_empty(), "{:?}", imported.diagnostics);
        assert_eq!(imported.metadata, original.metadata);
        let returned = run(Format::NamuMark, Format::Html, &imported.content);
        assert!(returned.diagnostics.is_empty(), "{:?}", returned.diagnostics);
        assert_eq!(returned.metadata, original.metadata);
    }
}

#[test]
fn redirect_target_and_fragment_keep_literal_spelling_through_html() {
    let input = "#redirect orchard_tree#late_fruit";
    let original = run(Format::NamuMark, Format::Html, input);
    assert!(original.diagnostics.is_empty(), "{:?}", original.diagnostics);
    let imported = run(Format::Html, Format::NamuMark, &original.content);
    assert!(imported.diagnostics.is_empty(), "{:?}", imported.diagnostics);
    assert_eq!(imported.metadata.redirect_target.as_deref(), Some("orchard_tree#late_fruit"));
    let returned = run(Format::NamuMark, Format::Html, &imported.content);
    assert!(returned.diagnostics.is_empty(), "{:?}", returned.diagnostics);
    assert_eq!(returned.metadata, original.metadata);
}

#[test]
fn inline_code_ending_in_backslash_has_visible_located_fallback() {
    let input = r"<p><code>x\</code></p>";
    let converted = run(Format::Html, Format::NamuMark, input);
    assert!(converted.diagnostics.iter().any(|diagnostic| {
        diagnostic.kind == DiagnosticKind::ConversionLoss
            && diagnostic.span == (0..input.len())
    }), "{:?}", converted.diagnostics);
    let rendered = run(Format::NamuMark, Format::Html, &converted.content);
    assert!(rendered.diagnostics.is_empty(), "{:?}", rendered.diagnostics);
    let html = Html::parse_fragment(&rendered.content);
    assert_eq!(html.root_element().text().collect::<String>(), r"x\");
    assert!(convert_with_mode(
        ConversionRequest {
            source: Format::Html,
            target: Format::NamuMark,
            input,
            context: &WikiContext::default(),
        },
        ConversionMode::Strict,
    ).is_err());
}
