use std::fs;
use std::io::Write;
use std::path::Path;
use std::process::{Command, Output, Stdio};

use pulldown_cmark::{html, Options, Parser};
use rwikitext::{convert, ConversionRequest, Format, WikiContext};
use scraper::Html;

const CONTEXT: WikiContext<'static> = WikiContext {
    page_title: "Parser test",
    site_language: "en",
    reference_time: "2026-11-17T00:00:00Z",
};

const WIKITEXT: &str = "First paragraph.\nIt continues here.\n\nSecond paragraph & more.";
const GFM: &str = "First paragraph.\nIt continues here.\n\nSecond paragraph & more.";
const HTML: &str = "<p>First paragraph.\nIt continues here.</p><p>Second paragraph &amp; more.</p>";

fn api(source: Format, target: Format, input: &str) -> String {
    let result = convert(ConversionRequest {
        source,
        target,
        input,
        context: &CONTEXT,
    })
    .expect("convert using Rust API");
    assert!(result.diagnostics.is_empty(), "{:?}", result.diagnostics);
    result.content
}

fn format_name(format: Format) -> &'static str {
    match format {
        Format::Wikitext => "wikitext",
        Format::Gfm => "gfm",
        Format::Html => "html",
    }
}

fn cli_command(source: Format, target: Format) -> Command {
    let mut command = Command::new(env!("CARGO_BIN_EXE_rwikitext"));
    command
        .args([format_name(source), format_name(target)])
        .args(["--page-title", CONTEXT.page_title])
        .args(["--site-language", CONTEXT.site_language])
        .args(["--reference-time", CONTEXT.reference_time]);
    command
}

fn cli_output(source: Format, target: Format, input: &str) -> Output {
    let mut child = cli_command(source, target)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("start CLI");
    child
        .stdin
        .take()
        .expect("open stdin")
        .write_all(input.as_bytes())
        .expect("write stdin");
    child.wait_with_output().expect("wait for CLI")
}

fn stdout(output: Output) -> String {
    assert!(output.status.success(), "{:?}", output);
    assert!(output.stderr.is_empty(), "{:?}", output);
    String::from_utf8(output.stdout).expect("convert CLI stdout to UTF-8")
}

fn cli(source: Format, target: Format, input: &str) -> String {
    stdout(cli_output(source, target, input))
}

fn render_gfm(input: &str) -> String {
    let mut html = String::new();
    html::push_html(&mut html, Parser::new_ext(input, Options::ENABLE_GFM));
    html
}

fn paragraphs(input: &str) -> Vec<String> {
    Html::parse_fragment(input)
        .root_element()
        .child_elements()
        .map(|element| {
            assert_eq!(element.value().name(), "p");
            assert_eq!(element.child_elements().count(), 0);
            element
                .text()
                .collect::<String>()
                .split_whitespace()
                .collect::<Vec<_>>()
                .join(" ")
        })
        .collect()
}

fn assert_paragraph_conversions(convert: fn(Format, Format, &str) -> String) {
    use Format::{Gfm, Html, Wikitext};

    let expected = paragraphs(HTML);
    let render_wikitext = |wikitext: &str| paragraphs(&convert(Wikitext, Html, wikitext));

    let markdown = convert(Wikitext, Gfm, WIKITEXT);
    let html = convert(Wikitext, Html, WIKITEXT);
    assert_eq!(paragraphs(&render_gfm(&markdown)), expected);
    assert_eq!(paragraphs(&html), expected);
    assert_eq!(render_wikitext(&convert(Gfm, Wikitext, GFM)), expected);
    assert_eq!(render_wikitext(&convert(Html, Wikitext, HTML)), expected);
    assert_eq!(
        render_wikitext(&convert(Gfm, Wikitext, &markdown)),
        expected
    );
    assert_eq!(render_wikitext(&convert(Html, Wikitext, &html)), expected);
}

#[test]
fn api_converts_and_round_trips_paragraphs() {
    assert_paragraph_conversions(api);
}

#[test]
fn imported_paragraphs_preserve_literal_punctuation() {
    use Format::{Gfm, Html, Wikitext};

    let markdown = "A \"quote\" -- with ... and ---- prose.";
    let html = "<p>A \"quote\" -- with ... and ---- prose.</p><p>----</p>";
    for (source, input, expected) in [
        (Gfm, markdown, render_gfm(markdown)),
        (Html, html, html.into()),
    ] {
        let wikitext = api(source, Wikitext, input);
        assert_eq!(
            paragraphs(&api(Wikitext, Html, &wikitext)),
            paragraphs(&expected)
        );
    }
}

#[test]
fn cli_converts_and_round_trips_paragraphs_from_stdin() {
    assert_paragraph_conversions(cli);
}

#[test]
fn cli_reads_files_and_writes_only_content_to_stdout() {
    let output = cli_command(Format::Wikitext, Format::Html)
        .arg(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/tests/fixtures/paragraphs.wiki"
        ))
        .stdin(Stdio::null())
        .output()
        .expect("read CLI file input");
    assert_eq!(paragraphs(&stdout(output)), paragraphs(HTML));
}

#[test]
fn cli_writes_errors_to_stderr() {
    let output = cli_command(Format::Gfm, Format::Html)
        .stdin(Stdio::null())
        .output()
        .expect("run unsupported conversion");
    assert!(!output.status.success());
    assert!(output.stdout.is_empty());
    assert!(!output.stderr.is_empty());
}

fn parser_case(source: &str, title: &str) -> (String, String) {
    let mut lines = source.lines();
    while let Some(line) = lines.next() {
        if line.strip_prefix("!!").map(str::trim) == Some("test") && lines.next() == Some(title) {
            let mut section = "";
            let mut wikitext = None;
            let mut html = None;
            let mut parsoid = None;
            for line in lines.by_ref() {
                if let Some(name) = line.strip_prefix("!!") {
                    section = name.trim();
                    match section {
                        "end" => break,
                        "wikitext" => wikitext = Some(Vec::new()),
                        "html" => html = Some(Vec::new()),
                        "html/parsoid" => parsoid = Some(Vec::new()),
                        _ => {}
                    }
                } else {
                    match section {
                        "wikitext" => wikitext.as_mut().unwrap().push(line),
                        "html" => html.as_mut().unwrap().push(line),
                        "html/parsoid" => parsoid.as_mut().unwrap().push(line),
                        _ => {}
                    }
                }
            }
            return (
                wikitext.expect("upstream wikitext section").join("\n"),
                parsoid
                    .or(html)
                    .expect("upstream Parsoid HTML section")
                    .join("\n"),
            );
        }
    }
    panic!("upstream case missing: {title}");
}

#[test]
#[ignore = "requires the pinned MediaWiki source archive; see docs/compatibility.md"]
fn pinned_parser_cases() {
    let release = Path::new(env!("CARGO_MANIFEST_DIR")).join("target/upstream/mediawiki-1.46.2");
    let cases = [
        ("tests/parser/parserTests.txt", "Blank input"),
        ("tests/parser/parserTests.txt", "Simple paragraph"),
        (
            "vendor/wikimedia/parsoid/tests/parser/pWrapping.txt",
            "No block tags",
        ),
    ];
    for (path, title) in cases {
        let source = fs::read_to_string(release.join(path)).expect("read pinned parser cases");
        let (wikitext, expected) = parser_case(&source, title);
        assert_eq!(
            paragraphs(&api(Format::Wikitext, Format::Html, &wikitext)),
            paragraphs(&expected),
            "{path}: {title}"
        );
        println!("{path}: {title}");
    }
}
