//! Contract examples authored from docs/namumark.md and the pinned r927 help,
//! especially the escape and hyperlink sections. These are manual-derived
//! expectations, not evidence from a live NamuWiki renderer.
use rwikitext::{
    convert, convert_with_options, ConversionMode, ConversionOptions, ConversionRequest,
    ConversionResult, DiagnosticKind, ExpansionLimits, FileInfo, Format, LookupError, WikiContext,
    WikiData,
};
use scraper::{Html, Selector};

#[path = "support/document.rs"]
mod document;

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
fn escaped_link_separator_belongs_to_title_instead_of_starting_label() {
    let result = run(Format::NamuMark, Format::Html, r"[[Orchard\|Gate|visit]]");
    assert!(result.diagnostics.is_empty(), "{:?}", result.diagnostics);
    let html = Html::parse_fragment(&result.content);
    let links: Vec<_> = html.select(&Selector::parse("a").unwrap()).collect();
    assert_eq!(links.len(), 1);
    assert_eq!(links[0].value().attr("href"), Some("./Orchard%7CGate"));
    assert_eq!(links[0].text().collect::<String>(), "visit");
}

#[test]
fn imported_plain_text_does_not_activate_namumark_blocks_or_metadata() {
    let original = "<p>#redirect Orchard</p><p>----</p><p> * Quince</p>";
    let converted = run(Format::Html, Format::NamuMark, original);
    assert!(
        converted.diagnostics.is_empty(),
        "{:?}",
        converted.diagnostics
    );
    let rendered = run(Format::NamuMark, Format::Html, &converted.content);
    assert_eq!(rendered.metadata, rwikitext::PageMetadata::default());
    assert!(
        rendered.diagnostics.is_empty(),
        "{:?}",
        rendered.diagnostics
    );
    assert_eq!(
        document::document_structure(&rendered.content),
        document::document_structure("<p>#redirect Orchard</p><p>----</p><p>* Quince</p>"),
    );
}

#[test]
fn escaped_backslash_in_internal_title_survives_as_one_literal_character() {
    let result = run(Format::NamuMark, Format::Html, r"[[Orchard\\Gate|visit]]");
    assert!(result.diagnostics.is_empty(), "{:?}", result.diagnostics);
    let html = Html::parse_fragment(&result.content);
    let link = html.select(&Selector::parse("a").unwrap()).next().unwrap();
    assert_eq!(link.value().attr("href"), Some("./Orchard%5CGate"));
    assert_eq!(link.text().collect::<String>(), "visit");
}

#[test]
fn ordinary_html_looking_namumark_text_is_literal_without_conversion_loss() {
    let result = run(Format::NamuMark, Format::Html, "<em>Orchard</em>");
    assert!(result.diagnostics.is_empty(), "{:?}", result.diagnostics);
    assert_eq!(
        document::document_structure(&result.content),
        document::document_structure("<p>&lt;em&gt;Orchard&lt;/em&gt;</p>"),
    );
}

#[test]
fn imported_literal_code_braces_do_not_claim_template_identity_loss() {
    for (source, input, expected) in [
        (
            Format::Gfm,
            "`{{Harvest}} fruit`",
            "<p><code>{{Harvest}} fruit</code></p>",
        ),
        (
            Format::Wikitext,
            "<nowiki>{{Harvest</nowiki>",
            "<p>{{Harvest</p>",
        ),
    ] {
        let converted = run(source, Format::NamuMark, input);
        assert!(
            converted.diagnostics.is_empty(),
            "{:?}",
            converted.diagnostics
        );
        let rendered = run(Format::NamuMark, Format::Html, &converted.content);
        assert!(
            rendered.diagnostics.is_empty(),
            "{:?}",
            rendered.diagnostics
        );
        assert_eq!(
            document::document_structure(&rendered.content),
            document::document_structure(expected),
        );
    }
}

#[test]
fn imported_internal_link_title_can_contain_escaped_delimiters() {
    let input = "<p><a rel=\"mw:WikiLink\" href=\"./Orchard%7CGate%5CPlot%5D\">visit</a></p>";
    let converted = run(Format::Html, Format::NamuMark, input);
    assert!(
        converted.diagnostics.is_empty(),
        "{:?}",
        converted.diagnostics
    );
    let rendered = run(Format::NamuMark, Format::Html, &converted.content);
    assert!(
        rendered.diagnostics.is_empty(),
        "{:?}",
        rendered.diagnostics
    );
    assert_eq!(
        document::document_structure(&rendered.content),
        document::document_structure("<p><a href=\"./Orchard%7CGate%5CPlot%5D\">visit</a></p>"),
    );
}

#[test]
fn combined_bold_and_italic_preserves_both_styles() {
    let result = run(Format::NamuMark, Format::Html, "'''''ripe'''''");
    assert!(result.diagnostics.is_empty(), "{:?}", result.diagnostics);
    let html = Html::parse_fragment(&result.content);
    let nested = Selector::parse(
        "b > i, b > em, strong > i, strong > em, i > b, i > strong, em > b, em > strong",
    )
    .unwrap();
    let styled = html
        .select(&nested)
        .next()
        .expect("bold and italic must both wrap the text");
    assert_eq!(styled.text().collect::<String>(), "ripe");
    assert_eq!(html.root_element().text().collect::<String>(), "ripe");
}

#[test]
fn excluded_include_is_located_and_strict_without_using_wiki_callbacks_or_expansion_budget() {
    struct NoLookups;
    impl WikiData for NoLookups {
        fn template_text(&self, _: &str) -> Result<Option<String>, LookupError> {
            panic!("NamuMark must not request MediaWiki template text")
        }
        fn page_exists(&self, _: &str) -> Result<Option<bool>, LookupError> {
            panic!("NamuMark must not request MediaWiki page existence")
        }
        fn file_info(&self, _: &str) -> Result<Option<FileInfo>, LookupError> {
            panic!("NamuMark must not request MediaWiki file information")
        }
    }

    let input = "배\r\n[include(틀:Harvest)]";
    let context = WikiContext::default();
    let request = ConversionRequest {
        source: Format::NamuMark,
        target: Format::Html,
        input,
        context: &context,
    };
    let options = ConversionOptions {
        wiki_data: Some(&NoLookups),
        expansion_limits: ExpansionLimits {
            max_depth: 0,
            max_expansions: 0,
            max_output_bytes: 4096,
        },
        ..ConversionOptions::default()
    };
    let result = convert_with_options(request, options).unwrap();
    assert_eq!(
        document::document_structure(&result.content),
        document::document_structure("<p>배<br>[include(틀:Harvest)]</p>"),
    );
    assert!(
        result.diagnostics.iter().any(|diagnostic| {
            diagnostic.kind == DiagnosticKind::ConversionLoss && diagnostic.span == (5..27)
        }),
        "{:?}",
        result.diagnostics
    );
    assert!(!result
        .diagnostics
        .iter()
        .any(|diagnostic| { diagnostic.kind == DiagnosticKind::ExpansionLimit }));
    let error = convert_with_options(
        request,
        ConversionOptions {
            mode: ConversionMode::Strict,
            ..options
        },
    )
    .unwrap_err();
    assert!(
        error.diagnostics().iter().any(|diagnostic| {
            diagnostic.kind == DiagnosticKind::ConversionLoss && diagnostic.span == (5..27)
        }),
        "{:?}",
        error.diagnostics()
    );
}

#[test]
fn imported_explicit_internal_transport_preserves_lowercase_and_spaces() {
    let input = "<p><a rel=\"mw:WikiLink\" href=\"./orchard%20tree\">visit</a></p>";
    let converted = run(Format::Html, Format::NamuMark, input);
    assert!(
        converted.diagnostics.is_empty(),
        "{:?}",
        converted.diagnostics
    );
    let rendered = run(Format::NamuMark, Format::Html, &converted.content);
    assert!(
        rendered.diagnostics.is_empty(),
        "{:?}",
        rendered.diagnostics
    );
    assert_eq!(
        document::document_structure(&rendered.content),
        document::document_structure("<p><a href=\"./orchard%20tree\">visit</a></p>"),
    );
}

#[test]
fn mediawiki_link_conversion_emits_a_real_namumark_link() {
    let converted = run(Format::Wikitext, Format::NamuMark, "[[Orchard|visit]]");
    assert!(
        converted.diagnostics.is_empty(),
        "{:?}",
        converted.diagnostics
    );
    let rendered = run(Format::NamuMark, Format::Html, &converted.content);
    assert!(
        rendered.diagnostics.is_empty(),
        "{:?}",
        rendered.diagnostics
    );
    assert_eq!(
        document::document_structure(&rendered.content),
        document::document_structure("<p><a href=\"./Orchard\">visit</a></p>"),
    );
}

#[test]
fn recognized_namumark_macros_and_footnotes_stay_located_exclusions() {
    use rwikitext::convert_with_mode;
    for input in [
        "[* footnote]",
        "[*A]",
        "[include(틀:X)]",
        "[date]",
        "[목차]",
        "[footnote]",
        "[youtube(id)]",
        "[anchor(keepall)]",
        "[age(2000-01-01)]",
        "[DATE]",
        "[Include(x)]",
        "[br(1)]",
    ] {
        let result = run(Format::NamuMark, Format::Html, input);
        assert_eq!(
            result.diagnostics.len(),
            1,
            "input {input}: {:?}",
            result.diagnostics
        );
        assert_eq!(
            result.diagnostics[0].kind,
            DiagnosticKind::ConversionLoss,
            "input {input}"
        );
        assert_eq!(result.diagnostics[0].span, 0..input.len(), "input {input}");
        assert_eq!(
            document::document_structure(&result.content),
            document::document_structure(&format!("<p>{input}</p>")),
            "input {input}"
        );
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
        assert!(
            error
                .diagnostics()
                .iter()
                .any(|diagnostic| diagnostic.kind == DiagnosticKind::ConversionLoss),
            "input {input}"
        );
    }
}

#[test]
fn br_macro_name_matches_ascii_case_insensitively() {
    use rwikitext::convert_with_mode;
    for input in ["[br]", "[BR]", "[Br]"] {
        let result = run(Format::NamuMark, Format::Html, input);
        assert!(
            result.diagnostics.is_empty(),
            "input {input}: {:?}",
            result.diagnostics
        );
        assert_eq!(
            document::document_structure(&result.content),
            document::document_structure("<p><br></p>"),
            "input {input}"
        );
        assert!(
            convert_with_mode(
                ConversionRequest {
                    source: Format::NamuMark,
                    target: Format::Html,
                    input,
                    context: &WikiContext::default(),
                },
                ConversionMode::Strict,
            )
            .is_ok(),
            "input {input}"
        );
    }
}
