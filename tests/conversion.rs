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
fn html_import_preserves_a_same_page_link_to_a_custom_heading_id() {
    let input = "<h2 id=\"legacy\">Heading</h2><p><a href=\"#legacy\">Jump</a></p>";
    for convert in [api, cli] {
        let wiki = convert(Format::Html, Format::Wikitext, input);
        let html = convert(Format::Wikitext, Format::Html, &wiki);
        let document = Html::parse_fragment(&html);
        for selector in ["h2[id='legacy']", "a[href='#legacy']"] {
            assert_eq!(
                document
                    .select(&scraper::Selector::parse(selector).unwrap())
                    .count(),
                1
            );
        }
    }
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
}

#[test]
fn api_and_cli_report_located_conversion_loss_and_support_strict_mode() {
    use rwikitext::{convert_with_mode, ConversionMode, DiagnosticKind};
    use Format::{Html, Wikitext};

    let input = "앞 {{Unsupported|{{Nested}}}} after";
    let request = |input| ConversionRequest {
        source: Wikitext,
        target: Html,
        input,
        context: &CONTEXT,
    };
    let best = convert(request(input)).unwrap();
    assert_eq!(best.content, "<p>앞  after</p>");
    let [loss] = best.diagnostics.as_slice() else {
        panic!("expected one loss")
    };
    assert_eq!(loss.kind, DiagnosticKind::ConversionLoss);
    assert_eq!(&input[loss.span.clone()], "{{Unsupported|{{Nested}}}}");
    assert!(loss.message.contains("transclusion"));
    let error = convert_with_mode(request(input), ConversionMode::Strict).unwrap_err();
    assert_eq!(error.diagnostics(), best.diagnostics);
    assert!(convert_with_mode(request("앞 after"), ConversionMode::Strict).is_ok());

    let best = cli_output(Wikitext, Html, input);
    assert!(best.status.success());
    assert_eq!(String::from_utf8_lossy(&best.stdout), "<p>앞  after</p>");
    assert!(String::from_utf8_lossy(&best.stderr).contains("transclusion"));
    let strict = output_with_stdin(cli_command(Wikitext, Html).arg("--strict"), input);
    assert!(!strict.status.success());
    assert!(strict.stdout.is_empty());
    assert_eq!(strict.stderr, best.stderr);
    let okay = output_with_stdin(cli_command(Wikitext, Html).arg("--strict"), "앞 after");
    assert_eq!(stdout(okay), "<p>앞 after</p>");
}

#[test]
fn html_policy_recovers_malformed_markup_and_reports_meaning_bearing_removals() {
    use rwikitext::{convert_with_mode, ConversionMode, DiagnosticKind};
    use Format::{Gfm, Html, Wikitext};

    let input = "<div class='panel'><p>Visit <a href='https://example.org'>safe</a>, <a href='jav&#x61;script:alert(1)'>unsafe</a><p><span lang=fr onclick='bad()'>salut</span><script>alert(1)</script></div>";
    for source in [Gfm, Html] {
        let request = ConversionRequest {
            source,
            target: Wikitext,
            input,
            context: &CONTEXT,
        };
        let result = convert(request).unwrap();
        assert_eq!(result.diagnostics.len(), 3, "{:?}", result.diagnostics);
        let loss = &result.diagnostics[0];
        assert_eq!(loss.kind, DiagnosticKind::ConversionLoss);
        assert_eq!(&input[loss.span.clone()], "href='jav&#x61;script:alert(1)'");
        assert_eq!(result.diagnostics[1].kind, DiagnosticKind::HtmlRemoval);
        assert_eq!(
            &input[result.diagnostics[1].span.clone()],
            "onclick='bad()'"
        );
        assert_eq!(
            convert_with_mode(request, ConversionMode::Strict)
                .unwrap_err()
                .diagnostics(),
            result.diagnostics
        );

        let output = cli_output(source, Wikitext, input);
        assert!(output.status.success());
        assert_eq!(String::from_utf8_lossy(&output.stdout), result.content);
        assert!(String::from_utf8_lossy(&output.stderr).contains("prohibited HTML attribute href"));
        let strict = output_with_stdin(cli_command(source, Wikitext).arg("--strict"), input);
        assert!(!strict.status.success());
        assert!(strict.stdout.is_empty());

        let html = api(Wikitext, Html, &result.content);
        let dom = scraper::Html::parse_fragment(&html);
        let select = |selector| {
            dom.select(&scraper::Selector::parse(selector).unwrap())
                .count()
        };
        assert_eq!(select("div.panel > p"), 2);
        assert_eq!(select("div.panel a[href='https://example.org']"), 1);
        assert_eq!(select("div.panel span[lang=fr]"), 1);
        assert_eq!(select("script, [onclick]"), 0);
        assert!(html.contains("unsafe"));
        assert!(!html.contains("alert(1)"));
    }
}

#[test]
fn wikitext_literal_html_preserves_allowed_attributes_and_renders_forbidden_tags_as_text() {
    use rwikitext::{convert_with_mode, ConversionMode, DiagnosticKind};
    use Format::{Html, Wikitext};
    let input = "Text <span class='label' style='color:red' onmouseover='bad()'>hello</span> <script>x</script>";
    let result = convert_with_mode(
        ConversionRequest {
            source: Wikitext,
            target: Html,
            input,
            context: &CONTEXT,
        },
        ConversionMode::Strict,
    )
    .unwrap();
    assert!(result
        .diagnostics
        .iter()
        .all(|d| d.kind == DiagnosticKind::HtmlRemoval));
    assert_eq!(result.diagnostics.len(), 3);
    let dom = scraper::Html::parse_fragment(&result.content);
    let selector = scraper::Selector::parse("span.label[style='color:red']").unwrap();
    assert_eq!(dom.select(&selector).next().unwrap().inner_html(), "hello");
    assert!(dom
        .root_element()
        .text()
        .collect::<String>()
        .contains("<script>x</script>"));
    let output = output_with_stdin(cli_command(Wikitext, Html).arg("--strict"), input);
    assert!(output.status.success());
    assert!(String::from_utf8_lossy(&output.stderr).contains("onmouseover"));
}

#[test]
fn repeated_losses_refer_to_distinct_original_byte_ranges() {
    use Format::{Gfm, Html, Wikitext};
    let wiki = "한글 {{Discard}}\r\n\r\none [[bad\r\n\r\none [[bad";
    let result = convert(ConversionRequest {
        source: Wikitext,
        target: Html,
        input: wiki,
        context: &CONTEXT,
    })
    .unwrap();
    assert_eq!(result.diagnostics.len(), 3);
    let ranges: Vec<_> = result.diagnostics.iter().map(|d| d.span.clone()).collect();
    assert_eq!(&wiki[ranges[0].clone()], "{{Discard}}");
    assert_eq!(&wiki[ranges[1].clone()], "one [[bad");
    assert_eq!(&wiki[ranges[2].clone()], "one [[bad");
    assert!(ranges[1].end < ranges[2].start);

    let markdown = "`onclick=\"x()\"`\n\n<span onclick=\"x()\">hi</span>\n\n[go](javascript:bad)";
    let result = convert(ConversionRequest {
        source: Gfm,
        target: Wikitext,
        input: markdown,
        context: &CONTEXT,
    })
    .unwrap();
    assert_eq!(result.diagnostics.len(), 2, "{:?}", result.diagnostics);
    assert_eq!(
        result.diagnostics[0].span.start,
        markdown.rfind("onclick").unwrap()
    );
    assert_eq!(
        &markdown[result.diagnostics[1].span.clone()],
        "[go](javascript:bad)"
    );
}

#[test]
fn imported_fragments_keep_safe_link_attributes_and_literal_wiki_markup() {
    use Format::{Html, Wikitext};
    let input = "<br><hr style='color:red; /* allowed */ border:0'><p><a href='https://example.org' title='{{Literal}}'><b>bold</b></a></p>";
    for convert in [api, cli] {
        let wiki = convert(Html, Wikitext, input);
        let recovered = convert(Wikitext, Html, &wiki);
        let dom = scraper::Html::parse_fragment(&recovered);
        for selector in [
            "br",
            "hr[style*='border:0']",
            "span[title='{{Literal}}'] a > b",
        ] {
            assert_eq!(
                dom.select(&scraper::Selector::parse(selector).unwrap())
                    .count(),
                1,
                "{recovered}"
            );
        }
    }
}

#[test]
fn literal_wiki_preformatted_content_is_not_expanded() {
    use Format::{Html, Wikitext};
    let actual = api(
        Wikitext,
        Html,
        "<pre>{{keep}} &amp; [[Main Page]] <b>literal</b></pre>",
    );
    let dom = scraper::Html::parse_fragment(&actual);
    let pre = dom
        .select(&scraper::Selector::parse("pre").unwrap())
        .next()
        .unwrap();
    assert_eq!(
        pre.text().collect::<String>(),
        "{{keep}} & [[Main Page]] <b>literal</b>"
    );
    assert_eq!(pre.child_elements().count(), 0);
}

#[test]
#[ignore = "requires the pinned MediaWiki source archive; see docs/compatibility.md"]
fn pinned_html_attribute_safety_cases() {
    use rwikitext::DiagnosticKind;
    use Format::{Html, Wikitext};

    let path = "vendor/wikimedia/parsoid/tests/parser/parserTests.txt";
    let source = fs::read_to_string(mediawiki_release().join(path)).unwrap();
    for (title, safe) in [
        ("T4304: HTML attribute safety (link)", true),
        ("MSIE CSS safety test: hex code", false),
    ] {
        let (input, expected) = parser_case(&source, title);
        let result = convert(ConversionRequest {
            source: Wikitext,
            target: Html,
            input: &input,
            context: &CONTEXT,
        })
        .unwrap();
        assert_eq!(
            document_structure(&result.content),
            document_structure(&expected)
        );
        let actual_dom = scraper::Html::parse_fragment(&result.content);
        let expected_dom = scraper::Html::parse_fragment(&expected);
        let selector = scraper::Selector::parse("div").unwrap();
        let actual = actual_dom.select(&selector).next().unwrap();
        let expected = expected_dom.select(&selector).next().unwrap();
        if safe {
            assert_eq!(actual.attr("title"), expected.attr("title"));
            assert!(result.diagnostics.is_empty());
        } else {
            assert_eq!(expected.attr("style"), Some("/* insecure input */"));
            assert_eq!(actual.attr("style"), None);
            assert_eq!(result.diagnostics[0].kind, DiagnosticKind::ConversionLoss);
        }
        println!("{path}: {title}");
    }
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
