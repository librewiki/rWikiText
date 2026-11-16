use std::fs;
use std::io::Write;
use std::path::Path;
use std::process::{Command, Output, Stdio};

use pulldown_cmark::{html, Options, Parser};
use rwikitext::{convert, ConversionRequest, Format, WikiContext};
use scraper::{ElementRef, Html, Node};

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
    output_with_stdin(&mut cli_command(source, target), input)
}

fn output_with_stdin(command: &mut Command, input: &str) -> Output {
    let mut child = command
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("start converter process");
    child
        .stdin
        .take()
        .expect("open stdin")
        .write_all(input.as_bytes())
        .expect("write stdin");
    child
        .wait_with_output()
        .expect("wait for converter process")
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

fn document_structure(input: &str) -> String {
    fn append_text(output: &mut String, text: &str) {
        for character in html_escape::encode_text(text).chars() {
            if character.is_ascii_whitespace() {
                if !output.ends_with(' ') {
                    output.push(' ');
                }
            } else {
                output.push(character);
            }
        }
    }

    fn children(parent: ElementRef<'_>, inline: bool) -> String {
        let mut output = String::new();
        for child in parent.children() {
            match child.value() {
                Node::Text(text) if inline || !text.trim_ascii().is_empty() => {
                    append_text(&mut output, text);
                }
                Node::Element(node) => {
                    let element = ElementRef::wrap(child).unwrap();
                    if node.name() == "span" && node.attr("typeof") == Some("mw:Entity") {
                        append_text(&mut output, &element.text().collect::<String>());
                    } else {
                        output.push_str(&normalize_element(element));
                    }
                }
                Node::Text(_) | Node::Comment(_) => {}
                other => panic!("unexpected DOM node {other:?}"),
            }
        }
        output.trim_ascii().into()
    }

    fn normalize_element(element: ElementRef<'_>) -> String {
        let name = element.value().name();
        if element.value().classes().any(|class| class == "mw-heading") {
            return normalize_element(
                element
                    .child_elements()
                    .next()
                    .expect("heading in MediaWiki wrapper"),
            );
        }
        if name == "section" {
            return children(element, false);
        }
        let href = element
            .attr("href")
            .map(|href| {
                format!(
                    " href=\"{}\"",
                    html_escape::encode_double_quoted_attribute(href)
                )
            })
            .unwrap_or_default();
        let content = children(element, !matches!(name, "ul" | "ol"));
        format!("<{name}{href}>{content}</{name}>")
    }

    children(Html::parse_fragment(input).root_element(), false)
}

fn assert_document_conversions(
    convert: fn(Format, Format, &str) -> String,
    wikitext: &str,
    gfm: &str,
    html: &str,
) {
    assert_conversions_with_renderer(convert, wikitext, gfm, html, |source| {
        convert(Format::Wikitext, Format::Html, source)
    });
}

fn assert_conversions_with_renderer(
    convert: fn(Format, Format, &str) -> String,
    wikitext: &str,
    gfm: &str,
    html: &str,
    render: impl Fn(&str) -> String,
) {
    use Format::{Gfm, Html, Wikitext};

    let expected = document_structure(html);
    let render_wikitext = |wikitext: &str| document_structure(&render(wikitext));

    let markdown = convert(Wikitext, Gfm, wikitext);
    let exported_html = convert(Wikitext, Html, wikitext);
    assert_eq!(render_wikitext(wikitext), expected);
    assert_eq!(document_structure(&render_gfm(&markdown)), expected);
    assert_eq!(document_structure(&exported_html), expected);
    assert_eq!(render_wikitext(&convert(Gfm, Wikitext, gfm)), expected);
    assert_eq!(render_wikitext(&convert(Html, Wikitext, html)), expected);
    assert_eq!(
        render_wikitext(&convert(Gfm, Wikitext, &markdown)),
        expected
    );
    assert_eq!(
        render_wikitext(&convert(Html, Wikitext, &exported_html)),
        expected
    );
}

#[test]
fn api_converts_and_round_trips_paragraphs() {
    assert_document_conversions(api, WIKITEXT, GFM, HTML);
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
            document_structure(&api(Wikitext, Html, &wikitext)),
            document_structure(&expected)
        );
    }
}

#[test]
fn cli_converts_and_round_trips_paragraphs_from_stdin() {
    assert_document_conversions(cli, WIKITEXT, GFM, HTML);
}

#[test]
fn api_and_cli_preserve_heading_levels_text_and_placement() {
    let wiki = "= One =\nIntro text.\n== Two ==\n=== Three & more ===\n==== Four ====\nAfter four.\n===== Five =====\n====== Six ======\nClosing text.";
    let gfm = "One\n===\nIntro text.\n## Two\n### Three & more\n#### Four\nAfter four.\n##### Five\n###### Six\nClosing text.";
    let html = "<h1>One</h1><p>Intro text.</p><h2>Two</h2><h3>Three &amp; more</h3><h4>Four</h4><p>After four.</p><h5>Five</h5><h6>Six</h6><p>Closing text.</p>";

    for convert in [api, cli] {
        assert_document_conversions(convert, wiki, gfm, html);
    }
}

#[test]
fn asymmetric_wikitext_heading_keeps_extra_equals_in_the_label() {
    let expected = document_structure("<h2>= Heading</h2>");
    for convert in [api, cli] {
        let output = convert(Format::Wikitext, Format::Html, "=== Heading ==");
        assert_eq!(document_structure(&output), expected);
    }
}

#[test]
fn api_and_cli_preserve_basic_link_labels_destinations_and_prose() {
    let wiki = "See [[Main Page]] and [[other page|the guide]], then [https://example.org/find?q=rust&lang=en search].\n\nVisit https://example.net.";
    let gfm = "See [Main Page](./Main_Page) and [the guide](./Other_page), then [search](https://example.org/find?q=rust&lang=en).\n\nVisit https://example.net.";
    let html = "<p>See <a href=\"./Main_Page\">Main Page</a> and <a href=\"./Other_page\">the guide</a>, then <a href=\"https://example.org/find?q=rust&amp;lang=en\">search</a>.</p><p>Visit <a href=\"https://example.net\">https://example.net</a>.</p>";

    for convert in [api, cli] {
        assert_document_conversions(convert, wiki, gfm, html);
    }
}

#[test]
fn api_and_cli_preserve_same_page_link_fragments() {
    let wiki = "See [[#Local section|below]].\n\n== Local section ==";
    let gfm = "See [below](#Local_section).\n\n## Local section";
    let html = "<p>See <a href=\"#Local_section\">below</a>.</p><h2 id=\"Local_section\">Local section</h2>";

    for convert in [api, cli] {
        assert_document_conversions(convert, wiki, gfm, html);
    }
}

#[test]
fn same_page_link_resolves_to_a_heading_with_repeated_underscores() {
    for convert in [api, cli] {
        let html = convert(
            Format::Wikitext,
            Format::Html,
            "== A__B ==\n\nJump to [[#A__B|the heading]].",
        );
        let document = Html::parse_fragment(&html);
        let heading = scraper::Selector::parse("h2[id='A_B']").unwrap();
        let link = scraper::Selector::parse("a[href='#A_B']").unwrap();
        assert_eq!(document.select(&heading).count(), 1);
        assert_eq!(document.select(&link).count(), 1);
    }
}

#[test]
fn html_import_reports_a_same_page_link_to_an_unrepresentable_heading_id() {
    let input = "<h2 id=\"legacy\">Heading</h2><p><a href=\"#legacy\">Jump</a></p>";
    let result = convert(ConversionRequest {
        source: Format::Html,
        target: Format::Wikitext,
        input,
        context: &CONTEXT,
    });
    assert!(result.unwrap_err().to_string().contains("heading ID"));
    let output = cli_output(Format::Html, Format::Wikitext, input);
    assert!(!output.status.success());
    assert!(output.stdout.is_empty());
    assert!(String::from_utf8_lossy(&output.stderr).contains("heading ID"));
}

#[test]
fn external_link_targets_preserve_literal_entity_spelling() {
    let html = "<p><a href=\"https://example.org/?value=&amp;amp;\">Example</a></p>";
    let expected = document_structure(html);
    for convert in [api, cli] {
        let wiki = convert(Format::Html, Format::Wikitext, html);
        let gfm = convert(Format::Wikitext, Format::Gfm, &wiki);
        assert_eq!(document_structure(&render_gfm(&gfm)), expected);
        let recovered = convert(Format::Gfm, Format::Wikitext, &gfm);
        assert_eq!(
            document_structure(&convert(Format::Wikitext, Format::Html, &recovered)),
            expected
        );
    }
}

#[test]
fn basic_links_keep_source_labels_and_normalize_wiki_namespace_targets() {
    let wiki = "See [[Main_Page]], [[Talk:main page|discussion]], [[Main Page|A &lt; B]], and [https://example.org/ A &#91;B&#93;].";
    let gfm = "See [Main_Page](./Main_Page), [discussion](./Talk:Main_page), [A &lt; B](./Main_Page), and [A \\[B\\]](https://example.org/).";
    let html = "<p>See <a href=\"./Main_Page\">Main_Page</a>, <a href=\"./Talk:Main_page\">discussion</a>, <a href=\"./Main_Page\">A &lt; B</a>, and <a href=\"https://example.org/\">A [B]</a>.</p>";

    for convert in [api, cli] {
        assert_document_conversions(convert, wiki, gfm, html);
    }
}

#[test]
fn wikitext_plain_words_do_not_gain_links_or_swallow_markup_around_entities() {
    let wiki = "abchttps://example.org/ & [[Main Page]] &amp; text.";
    let html =
        "<p>abchttps://example.org/ &amp; <a href=\"./Main_Page\">Main Page</a> &amp; text.</p>";
    for convert in [api, cli] {
        let actual = convert(Format::Wikitext, Format::Html, wiki);
        assert_eq!(document_structure(&actual), document_structure(html));
    }
}

#[test]
fn api_and_cli_preserve_list_type_item_order_and_paragraph_boundaries() {
    let wiki = "Before.\n* Apple\n* Pear\n# First\n# Second\nAfter.";
    let gfm = "Before.\n\n- Apple\n- Pear\n\n1. First\n2. Second\n\nAfter.";
    let html = "<p>Before.</p><ul><li>Apple</li><li>Pear</li></ul><ol><li>First</li><li>Second</li></ol><p>After.</p>";

    for convert in [api, cli] {
        assert_document_conversions(convert, wiki, gfm, html);
    }
}

#[test]
fn adjacent_lists_of_the_same_type_stay_separate() {
    let wiki = "* One\n\n* Two\n# Three\n\n# Four";
    let gfm = "- One\n\n* Two\n\n1. Three\n\n1) Four";
    let html =
        "<ul><li>One</li></ul><ul><li>Two</li></ul><ol><li>Three</li></ol><ol><li>Four</li></ol>";

    for convert in [api, cli] {
        assert_document_conversions(convert, wiki, gfm, html);
    }
}

const MILESTONE_WIKI: &str = include_str!("fixtures/milestone.wiki");
const MILESTONE_GFM: &str = include_str!("fixtures/milestone.md");
const MILESTONE_HTML: &str = include_str!("fixtures/milestone.html");

#[test]
fn api_and_cli_convert_complete_basic_documents() {
    for convert in [api, cli] {
        assert_document_conversions(convert, MILESTONE_WIKI, MILESTONE_GFM, MILESTONE_HTML);
    }
}

#[test]
fn complete_document_cli_file_stdin_and_output_streams() {
    use Format::{Gfm, Html, Wikitext};

    let fixtures = [
        (Wikitext, Gfm, "milestone.wiki", MILESTONE_WIKI),
        (Wikitext, Html, "milestone.wiki", MILESTONE_WIKI),
        (Gfm, Wikitext, "milestone.md", MILESTONE_GFM),
        (Html, Wikitext, "milestone.html", MILESTONE_HTML),
    ];
    for (source, target, name, content) in fixtures {
        let path = Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("tests/fixtures")
            .join(name);
        let file = cli_command(source, target)
            .arg(path)
            .stdin(Stdio::null())
            .output()
            .expect("read complete document from disk");
        let stdin = output_with_stdin(cli_command(source, target).arg("-"), content);
        assert_eq!(stdout(file), stdout(stdin));
    }

    let output = cli_output(Wikitext, Html, "{{Unsupported template}}");
    assert!(!output.status.success());
    assert!(output.stdout.is_empty());
    assert!(String::from_utf8_lossy(&output.stderr).contains("unsupported wikitext syntax"));
}

#[test]
#[ignore = "requires the pinned MediaWiki release and PHP; see docs/compatibility.md"]
fn parsoid_confirms_complete_document_round_trips() {
    let render = |wikitext: &str| {
        let php = std::env::var_os("RWIKITEXT_PHP").unwrap_or_else(|| "php".into());
        let mut command = Command::new(php);
        command
            .arg(Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/render-parsoid.php"))
            .arg(mediawiki_release())
            .args([
                CONTEXT.page_title,
                CONTEXT.site_language,
                CONTEXT.reference_time,
            ]);
        stdout(output_with_stdin(&mut command, wikitext))
    };

    for convert in [api, cli] {
        assert_conversions_with_renderer(
            convert,
            MILESTONE_WIKI,
            MILESTONE_GFM,
            MILESTONE_HTML,
            render,
        );
    }
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
    assert_eq!(
        document_structure(&stdout(output)),
        document_structure(HTML)
    );
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
    let release = mediawiki_release();
    let cases = [
        ("tests/parser/parserTests.txt", "Blank input"),
        ("tests/parser/parserTests.txt", "Simple paragraph"),
        (
            "vendor/wikimedia/parsoid/tests/parser/pWrapping.txt",
            "No block tags",
        ),
        (
            "vendor/wikimedia/parsoid/tests/parser/headings.txt",
            "Basic section headings",
        ),
        (
            "tests/parser/parserTests.txt",
            "Multiple spaces in titles should normalize to a single underscore",
        ),
        (
            "vendor/wikimedia/parsoid/tests/parser/extLinks.txt",
            "External links: specified text",
        ),
        (
            "vendor/wikimedia/parsoid/tests/parser/extLinks.txt",
            "External links: non-bracketed",
        ),
        (
            "vendor/wikimedia/parsoid/tests/parser/extLinks.txt",
            "External links: Free with trailing punctuation",
        ),
        ("tests/parser/parserTests.txt", "Simple list"),
        ("tests/parser/parserTests.txt", "Common list"),
        ("tests/parser/parserTests.txt", "Numbered list"),
    ];
    for (path, title) in cases {
        let source = fs::read_to_string(release.join(path)).expect("read pinned parser cases");
        let (wikitext, expected) = parser_case(&source, title);
        assert_eq!(
            document_structure(&api(Format::Wikitext, Format::Html, &wikitext)),
            document_structure(&expected),
            "{path}: {title}"
        );
        println!("{path}: {title}");
    }
}

fn mediawiki_release() -> std::path::PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("target/upstream/mediawiki-1.46.2")
}
