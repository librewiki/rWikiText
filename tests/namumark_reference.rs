//! Original hand-authored snapshot oracle, distinct from actual renderer evidence.
//! See fixtures/namumark-baseline.provenance.json and docs/namumark.md.
use std::io::Write;
use std::process::Stdio;

use pulldown_cmark::{html, Options, Parser};
use rwikitext::{
    convert, convert_with_mode, ConversionMode, ConversionRequest, ConversionResult, DiagnosticKind,
    Format, PageMetadata, WikiContext,
};

#[path = "support/document.rs"]
mod document;
#[path = "support/parsoid.rs"]
mod parsoid;

use document::document_structure as body;
use Format::{Gfm, Html, NamuMark, Wikitext};

const NAMU: &str = include_str!("fixtures/namumark-baseline.namu");
const WIKI: &str = include_str!("fixtures/namumark-baseline.wiki");
const GFM: &str = include_str!("fixtures/namumark-baseline.gfm");
const EXPECTED: &str = include_str!("fixtures/namumark-baseline.html");
const CONTEXT: WikiContext<'static> = WikiContext {
    page_title: "Orchard notebook",
    site_language: "en",
    reference_time: "2026-11-24T00:00:00Z",
};

fn run(source: Format, target: Format, input: &str) -> ConversionResult {
    convert(ConversionRequest {
        source,
        target,
        input,
        context: &CONTEXT,
    })
    .unwrap_or_else(|error| panic!("{source:?} -> {target:?}: {error}; {:?}", error.diagnostics()))
}

fn render_gfm(input: &str) -> String {
    let mut rendered = String::new();
    html::push_html(&mut rendered, Parser::new_ext(input, Options::ENABLE_TABLES));
    rendered
}

fn rendered_body(format: Format, input: &str) -> String {
    match format {
        Html => body(input),
        Gfm => body(&render_gfm(input)),
        Wikitext | NamuMark => body(&run(format, Html, input).content),
    }
}

#[test]
fn all_six_directions_match_independently_authored_semantic_fixture() {
    let expected = body(EXPECTED);
    // Independently authored GFM also renders to the fixed semantic oracle.
    assert_eq!(body(&render_gfm(GFM)), expected);
    for (source, target, input) in [
        (NamuMark, Wikitext, NAMU),
        (NamuMark, Gfm, NAMU),
        (NamuMark, Html, NAMU),
        (Wikitext, NamuMark, WIKI),
        (Gfm, NamuMark, GFM),
        (Html, NamuMark, EXPECTED),
    ] {
        let result = run(source, target, input);
        assert!(result.diagnostics.is_empty(), "{source:?} -> {target:?}: {:?}", result.diagnostics);
        assert_eq!(result.metadata, PageMetadata::default());
        assert_eq!(rendered_body(target, &result.content), expected, "{source:?} -> {target:?}");
    }
}

#[test]
fn round_trips_starting_at_every_format_match_fixed_expected_meaning() {
    let expected = body(EXPECTED);
    for other in [Wikitext, Gfm, Html] {
        let intermediate = run(NamuMark, other, NAMU);
        let returned = run(other, NamuMark, &intermediate.content);
        assert_eq!(rendered_body(NamuMark, &returned.content), expected, "NamuMark -> {other:?} -> NamuMark");
    }
    for (other, original) in [(Wikitext, WIKI), (Gfm, GFM), (Html, EXPECTED)] {
        let intermediate = run(other, NamuMark, original);
        let returned = run(NamuMark, other, &intermediate.content);
        assert_eq!(rendered_body(other, &returned.content), expected, "{other:?} -> NamuMark -> {other:?}");
    }
}

#[test]
fn raw_newline_and_br_have_the_declared_namumark_break_meaning() {
    let expected = body("<p>Spring<br>harvest</p>");
    for input in ["Spring\nharvest", "Spring[br]harvest", "Spring\r\nharvest"] {
        assert_eq!(body(&run(NamuMark, Html, input).content), expected, "{input:?}");
        let gfm = run(NamuMark, Gfm, input);
        assert_eq!(body(&render_gfm(&gfm.content)), expected);
    }
}

#[test]
fn metadata_uses_declared_transport_and_body_only_gfm_loss() {
    let category = "[[분류:Orchards|Quince]]";
    for (input, expected) in [
        (
            category,
            PageMetadata {
                categories: vec!["Orchards".into()],
                category_sort_keys: vec![Some("Quince".into())],
                ..PageMetadata::default()
            },
        ),
        (
            "#redirect Orchard",
            PageMetadata { redirect_target: Some("Orchard".into()), ..PageMetadata::default() },
        ),
    ] {
        for target in [Wikitext, Html] {
            let result = run(NamuMark, target, input);
            assert_eq!(result.metadata, expected);
            assert!(result.diagnostics.is_empty(), "{:?}", result.diagnostics);
            let returned = run(target, NamuMark, &result.content);
            assert_eq!(returned.metadata, expected);
            assert_eq!(run(NamuMark, Html, &returned.content).metadata, expected);
        }
        let gfm = run(NamuMark, Gfm, input);
        assert_eq!(gfm.metadata, expected);
        assert!(gfm.diagnostics.iter().any(|loss| {
            loss.kind == DiagnosticKind::ConversionLoss && loss.span == (0..input.len())
        }), "{:?}", gfm.diagnostics);
        assert_eq!(run(Gfm, NamuMark, &gfm.content).metadata, PageMetadata::default());
        let strict = convert_with_mode(ConversionRequest {
            source: NamuMark,
            target: Gfm,
            input,
            context: &CONTEXT,
        }, ConversionMode::Strict).unwrap_err();
        assert!(strict.diagnostics().iter().any(|loss| loss.kind == DiagnosticKind::ConversionLoss));
    }
}

#[test]
fn pinned_manifest_loss_cases_use_original_utf8_byte_ranges() {
    let manifest: serde_json::Value = serde_json::from_str(include_str!("../docs/namumark-baseline.json")).unwrap();
    for fixture in manifest["fixtures"].as_array().unwrap() {
        let input = fixture["namumark"].as_str().unwrap();
        for expected in fixture["losses"].as_array().unwrap() {
            let span = expected["span"][0].as_u64().unwrap() as usize
                ..expected["span"][1].as_u64().unwrap() as usize;
            assert!(span.start < span.end && span.end <= input.len(), "{}: {span:?}", fixture["id"]);
            assert!(input.is_char_boundary(span.start) && input.is_char_boundary(span.end));
            let target = if expected["target"] == "gfm" { Gfm } else { Html };
            let result = run(NamuMark, target, input);
            assert!(result.diagnostics.iter().any(|actual| {
                actual.kind == DiagnosticKind::ConversionLoss && actual.span == span
            }), "{} -> {target:?}: expected {span:?}, got {:?}", fixture["id"], result.diagnostics);
        }
    }
}

#[test]
#[ignore = "requires pinned MediaWiki 1.46.2 and PHP; external evidence only when actually run"]
fn pinned_parsoid_confirms_namumark_mediawiki_bridge() {
    fn render(input: &str) -> String {
        let mut child = parsoid::parsoid_command(&CONTEXT)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .expect("start pinned Parsoid; see docs/compatibility.md");
        child.stdin.take().unwrap().write_all(input.as_bytes()).unwrap();
        let output = child.wait_with_output().unwrap();
        assert!(output.status.success(), "{}", String::from_utf8_lossy(&output.stderr));
        String::from_utf8(output.stdout).unwrap()
    }
    let expected = body(EXPECTED);
    assert_eq!(body(&render(WIKI)), expected, "authored MediaWiki reference fixture");
    let converted = run(NamuMark, Wikitext, NAMU);
    assert_eq!(body(&render(&converted.content)), expected, "actual Parsoid rendering of NamuMark -> MediaWiki output");
    let imported = run(Wikitext, NamuMark, WIKI);
    let returned = run(NamuMark, Wikitext, &imported.content);
    assert_eq!(body(&render(&returned.content)), expected, "actual Parsoid rendering of MediaWiki-origin round trip");
}
