//! Regression expectations from the fixed NamuMark profile, not converter output.
use rwikitext::{
    convert, convert_with_mode, ConversionMode, ConversionRequest, ConversionResult,
    DiagnosticKind, Format, WikiContext,
};
use scraper::{Html, Selector};

fn run(source: Format, target: Format, input: &str) -> ConversionResult {
    convert(ConversionRequest {
        source,
        target,
        input,
        context: &WikiContext::default(),
    })
    .unwrap()
}

fn strict_rejects(input: &str) {
    let error = convert_with_mode(
        ConversionRequest {
            source: Format::NamuMark,
            target: Format::Html,
            input,
            context: &WikiContext::default(),
        },
        ConversionMode::Strict,
    )
    .unwrap_err();
    assert!(error
        .diagnostics()
        .iter()
        .any(|diagnostic| diagnostic.kind == DiagnosticKind::ConversionLoss));
}

#[test]
fn visible_category_page_link_translates_to_mediawiki_category_namespace() {
    let converted = run(
        Format::NamuMark,
        Format::Wikitext,
        "[[:분류:Orchards|visit]]",
    );
    assert!(converted.diagnostics.is_empty(), "{:?}", converted.diagnostics);
    let rendered = run(Format::Wikitext, Format::Html, &converted.content);
    assert!(rendered.metadata.categories.is_empty());
    let html = Html::parse_fragment(&rendered.content);
    let selector = Selector::parse("a").unwrap();
    let links: Vec<_> = html.select(&selector).collect();
    assert_eq!(links.len(), 1);
    assert_eq!(links[0].attr("href"), Some("./Category:Orchards"));
    assert_eq!(links[0].text().collect::<String>(), "visit");
}

#[test]
fn excluded_multiline_html_directive_remains_one_located_literal() {
    let excluded = "{{{#!html\n<b>[[Orchard|visit]]</b>\n}}}";
    let input = format!("배\n\n{excluded}");
    let converted = run(Format::NamuMark, Format::Html, &input);
    let html = Html::parse_fragment(&converted.content);
    assert!(html.root_element().text().collect::<String>().contains(excluded));
    assert_eq!(html.select(&Selector::parse("a, b, strong").unwrap()).count(), 0);
    assert!(converted.diagnostics.iter().any(|diagnostic| {
        diagnostic.kind == DiagnosticKind::ConversionLoss
            && diagnostic.span == (5..input.len())
    }), "{:?}", converted.diagnostics);
    strict_rejects(&input);
}

#[test]
fn unclosed_heading_is_visible_but_strict_rejects_its_lost_structure() {
    let input = "== Orchard";
    let converted = run(Format::NamuMark, Format::Html, input);
    let html = Html::parse_fragment(&converted.content);
    assert_eq!(html.root_element().text().collect::<String>(), input);
    assert_eq!(html.select(&Selector::parse("h1, h2, h3, h4, h5, h6").unwrap()).count(), 0);
    assert!(converted.diagnostics.iter().any(|diagnostic| {
        diagnostic.kind == DiagnosticKind::ConversionLoss
            && diagnostic.span == (0..input.len())
    }), "{:?}", converted.diagnostics);
    strict_rejects(input);
}

#[test]
fn second_redirect_is_diagnosed_without_silently_discarding_a_target() {
    let input = "#redirect Orchard\n#redirect Gate";
    let converted = run(Format::NamuMark, Format::Html, input);
    assert!(converted.content.contains("Orchard"), "{}", converted.content);
    assert!(converted.content.contains("Gate"), "{}", converted.content);
    assert!(converted.diagnostics.iter().any(|diagnostic| {
        diagnostic.kind == DiagnosticKind::ConversionLoss
    }), "{:?}", converted.diagnostics);
    strict_rejects(input);
}
