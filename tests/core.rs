use std::io::Write;
use std::process::{Command, Stdio};

use rwikitext::{convert, ConversionRequest, Format, WikiContext};
use scraper::{Html, Node};

fn convert_api(source: Format, target: Format, input: &str) -> String {
    convert(ConversionRequest {
        source,
        target,
        input,
        context: &WikiContext::default(),
    })
    .unwrap()
    .content
}

fn convert_cli(source: &str, target: &str, input: &str) -> String {
    let mut child = Command::new(env!("CARGO_BIN_EXE_rwikitext"))
        .args([source, target])
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .spawn()
        .unwrap();
    child
        .stdin
        .take()
        .unwrap()
        .write_all(input.as_bytes())
        .unwrap();
    let output = child.wait_with_output().unwrap();
    assert!(output.status.success());
    String::from_utf8(output.stdout).unwrap()
}

fn structure(input: &str) -> String {
    fn visit(node: scraper::ElementRef<'_>, out: &mut String) {
        for child in node.children() {
            match child.value() {
                Node::Element(el) => {
                    let name = match el.name() {
                        "strong" => "b",
                        "em" => "i",
                        n => n,
                    };
                    out.push_str(&format!("<{name}>"));
                    visit(scraper::ElementRef::wrap(child).unwrap(), out);
                    out.push_str(&format!("</{name}>"));
                }
                Node::Text(text) if !text.trim().is_empty() => out.push_str(text.trim()),
                _ => (),
            }
        }
    }
    let fragment = Html::parse_fragment(input);
    let mut out = String::new();
    visit(fragment.root_element(), &mut out);
    out
}

#[test]
fn core_round_trips_api_and_cli() {
    let source = "''italic'' and '''bold''' &amp; text\n\n* one\n*# two\n*# three\n* four\n\n----\n\n preformatted\n second line";
    let html = convert_api(Format::Wikitext, Format::Html, source);
    for format in [Format::Html, Format::Gfm] {
        let intermediate = convert_api(Format::Wikitext, format, source);
        let restored = convert_api(format, Format::Wikitext, &intermediate);
        assert_eq!(
            structure(&html),
            structure(&convert_api(Format::Wikitext, Format::Html, &restored)),
            "{intermediate}\n{restored}"
        );
        let name = if format == Format::Html {
            "html"
        } else {
            "gfm"
        };
        assert_eq!(
            convert_cli("wikitext", name, source).trim_end(),
            intermediate.trim_end()
        );
        assert_eq!(
            convert_cli(name, "wikitext", &intermediate).trim_end(),
            restored.trim_end()
        );
    }
}

#[test]
fn definition_list_and_entities_round_trip() {
    let source = "; Term\n: Definition &amp; &#x41;\n; Another: Description";
    let expected =
        "<dl><dt>Term</dt><dd>Definition &amp; A</dd><dt>Another</dt><dd>Description</dd></dl>";
    let html = convert_api(Format::Wikitext, Format::Html, source);
    assert_eq!(structure(expected), structure(&html));
    for format in [Format::Html, Format::Gfm] {
        let intermediate = convert_api(Format::Wikitext, format, source);
        let wiki = convert_api(format, Format::Wikitext, &intermediate);
        assert_eq!(
            structure(expected),
            structure(&convert_api(Format::Wikitext, Format::Html, &wiki))
        );
    }
}

#[test]
fn allowed_external_protocols_round_trip() {
    let html = "<p><a href=\"mailto:person@example.org\">mail</a> <a href=\"ftp://example.org/file\">file</a></p>";
    let wiki = convert_api(Format::Html, Format::Wikitext, html);
    assert!(wiki.contains("[mailto:person@example.org mail]"));
    assert!(wiki.contains("[ftp://example.org/file file]"));
    let back = convert_api(Format::Wikitext, Format::Html, &wiki);
    assert!(back.contains("href=\"mailto:person@example.org\""));
    assert!(back.contains("href=\"ftp://example.org/file\""));
}

#[test]
fn inline_quote_processing_preserves_multiline_links() {
    let html = convert_api(Format::Wikitext, Format::Html, "[[Page|first\nsecond]]");
    assert!(html.contains("href=\"./Page\""));
    assert!(html.contains("first\nsecond"));
}

#[test]
fn preformatted_inline_and_literal_text_round_trip() {
    let wiki = " a\n ''b''";
    let expected = "<pre>a\n<i>b</i></pre>";
    assert_eq!(
        structure(expected),
        structure(&convert_api(Format::Wikitext, Format::Html, wiki))
    );
    for format in [Format::Html, Format::Gfm] {
        let intermediate = convert_api(Format::Wikitext, format, wiki);
        let restored = convert_api(format, Format::Wikitext, &intermediate);
        assert_eq!(
            structure(expected),
            structure(&convert_api(Format::Wikitext, Format::Html, &restored)),
            "{restored}"
        );
    }
    let literal = "<pre><code>[[Link]] {{template}} &amp; text</code></pre>";
    let restored = convert_api(Format::Html, Format::Wikitext, literal);
    let html = convert_api(Format::Wikitext, Format::Html, &restored);
    assert_eq!(
        structure("<pre>[[Link]] {{template}} &amp; text</pre>"),
        structure(&html)
    );
}

#[test]
#[ignore = "requires pinned MediaWiki 1.46.2 archive"]
fn pinned_core_parser_cases() {
    let fixtures =
        std::fs::read_to_string("target/upstream/mediawiki-1.46.2/tests/parser/parserTests.txt")
            .unwrap();
    for title in [
        "Horizontal ruler -- 4+ dashes render hr",
        "Horizontal ruler -- eats additional dashes on the same line",
        "Horizontal ruler -- Supports content following dashes on same line",
        "Nested lists 1",
        "Nested lists 2",
        "Nested lists 7 (skip initial nesting levels)",
        "Nested lists 8 (multiple nesting transitions)",
        "Mixed list",
    ] {
        let header = format!("!! test\n{title}\n");
        let case = fixtures
            .split_once(&header)
            .unwrap()
            .1
            .split_once("!! end")
            .unwrap()
            .0;
        let wiki = case
            .split_once("!! wikitext\n")
            .unwrap()
            .1
            .split_once("\n!! ")
            .unwrap()
            .0;
        let expected_section = case
            .split_once("!! html/parsoid\n")
            .or_else(|| case.split_once("!! html\n"))
            .unwrap()
            .1;
        let expected = expected_section
            .split_once("\n!! ")
            .map_or(expected_section.trim(), |(s, _)| s);
        let actual = convert_api(Format::Wikitext, Format::Html, wiki);
        assert_eq!(structure(expected), structure(&actual), "{title}: {actual}");
    }
    let quotes = std::fs::read_to_string(
        "target/upstream/mediawiki-1.46.2/vendor/wikimedia/parsoid/tests/parser/quotes.txt",
    )
    .unwrap();
    for title in [
        "Quotes",
        "Italics and bold",
        "Unclosed and unmatched quotes",
    ] {
        let header = format!("!! test\n{title}\n");
        let case = quotes
            .split_once(&header)
            .unwrap()
            .1
            .split_once("!! end")
            .unwrap()
            .0;
        let wiki = case
            .split_once("!! wikitext\n")
            .unwrap()
            .1
            .split_once("\n!! html")
            .unwrap()
            .0;
        let expected = case
            .split_once("!! html/parsoid\n")
            .or_else(|| case.split_once("!! html\n"))
            .unwrap()
            .1
            .trim();
        let actual = convert_api(Format::Wikitext, Format::Html, wiki);
        assert_eq!(structure(expected), structure(&actual), "{title}: {actual}");
    }
    let pre = std::fs::read_to_string(
        "target/upstream/mediawiki-1.46.2/vendor/wikimedia/parsoid/tests/parser/indentPre.txt",
    )
    .unwrap();
    let case = pre
        .split_once("!! test\nIndent preformatting with inline content\n")
        .unwrap()
        .1
        .split_once("!! end")
        .unwrap()
        .0;
    let wiki = case
        .split_once("!! wikitext\n")
        .unwrap()
        .1
        .split_once("\n!! html\n")
        .unwrap()
        .0;
    let expected = case.split_once("!! html\n").unwrap().1.trim();
    assert_eq!(
        structure(expected),
        structure(&convert_api(Format::Wikitext, Format::Html, wiki))
    );
}
