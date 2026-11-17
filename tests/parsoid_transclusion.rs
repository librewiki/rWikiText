use rwikitext::{convert_with_options, ConversionOptions, ConversionRequest, Format, WikiContext};
use scraper::{Html, Selector};
use serde_json::Value;
use std::io::Write;
use std::process::{Command, Stdio};

const WIKI: &str = "Before {{Message|a=b|  first  |name=Jin}} after.";

struct LocalWiki;

impl rwikitext::WikiData for LocalWiki {
    fn template_text(&self, title: &str) -> Result<Option<String>, rwikitext::LookupError> {
        Ok(match title {
            "Template:Message" => Some("'''{{{name}}}''' ({{{1}}}, {{{a}}})".into()),
            "Template:1x" => Some("{{{1}}}".into()),
            "Template:Foo" => Some("FOO".into()),
            "Template:Foo–bar" => Some("foo".into()),
            _ => None,
        })
    }
}

fn convert(source: Format, target: Format, input: &str) -> rwikitext::ConversionResult {
    convert_with_options(
        ConversionRequest {
            source,
            target,
            input,
            context: &WikiContext::default(),
        },
        ConversionOptions {
            wiki_data: Some(&LocalWiki),
            ..ConversionOptions::default()
        },
    )
    .unwrap()
}

#[test]
fn unset_parameters_in_a_piped_link_keep_the_table_cell_literal() {
    let result = convert(
        Format::Wikitext,
        Format::Html,
        "{|\n|[[{{{1}}}|{{{2}}}]]\n|}",
    );
    assert!(result.diagnostics.is_empty(), "{:?}", result.diagnostics);
    let document = Html::parse_fragment(&result.content);
    let cells = Selector::parse("table > tbody > tr > td").unwrap();
    assert_eq!(
        document
            .select(&cells)
            .map(|cell| cell.text().collect::<String>())
            .collect::<Vec<_>>(),
        ["[[{{{1}}}|{{{2}}}]]"]
    );
    assert!(document
        .select(&Selector::parse("a").unwrap())
        .next()
        .is_none());
}

#[test]
fn a_newline_inside_a_template_name_leaves_the_call_literal() {
    let input = "{{ech\no|foo}}";
    let result = convert(Format::Wikitext, Format::Html, input);
    assert!(result.diagnostics.is_empty(), "{:?}", result.diagnostics);
    let document = Html::parse_fragment(&result.content);
    assert_eq!(document.root_element().text().collect::<String>(), input);
}

#[test]
fn comments_within_template_names_survive_the_html_round_trip() {
    let input = "{{1<!--X-->x|foo}}";
    let mut wiki = input.to_owned();
    for _ in 0..2 {
        let result = convert(Format::Wikitext, Format::Html, &wiki);
        assert!(result.diagnostics.is_empty(), "{:?}", result.diagnostics);
        let document = Html::parse_fragment(&result.content);
        let group = document
            .select(&Selector::parse("[typeof='mw:Transclusion']").unwrap())
            .next()
            .unwrap();
        assert_eq!(group.text().collect::<String>(), "foo");
        let metadata: Value = serde_json::from_str(group.attr("data-mw").unwrap()).unwrap();
        let target = &metadata["parts"][0]["template"]["target"];
        assert_eq!(target["href"], "./Template:1x");
        assert_eq!(target["wt"], "1<!--X-->x");

        let recovered = convert(Format::Html, Format::Wikitext, &result.content);
        assert!(
            recovered.diagnostics.is_empty(),
            "{:?}",
            recovered.diagnostics
        );
        wiki = recovered.content;
    }
}

#[test]
fn comment_only_lines_in_template_output_do_not_create_blocks() {
    let input = "{{1x|foo\n<!--should be ignored-->\n <!--should be ignored as well-->\nbar}}";
    let result = convert(Format::Wikitext, Format::Html, input);
    assert!(result.diagnostics.is_empty(), "{:?}", result.diagnostics);
    let document = Html::parse_fragment(&result.content);
    let paragraphs = Selector::parse("p").unwrap();
    let paragraph_text = document
        .select(&paragraphs)
        .map(|p| {
            let text: String = p.text().collect();
            text.split_whitespace().collect::<Vec<_>>().join(" ")
        })
        .collect::<Vec<_>>();
    assert_eq!(paragraph_text, ["foo bar"]);
    assert!(document
        .select(&Selector::parse("pre").unwrap())
        .next()
        .is_none());

    let recovered = convert(Format::Html, Format::Wikitext, &result.content);
    assert!(
        recovered.diagnostics.is_empty(),
        "{:?}",
        recovered.diagnostics
    );
    assert_eq!(recovered.content, input);
}

#[test]
fn parser_function_names_with_comments_round_trip() {
    let input = "{{#i<!--note-->f:1|yes|no}}";
    let html = convert(Format::Wikitext, Format::Html, input);
    assert!(html.diagnostics.is_empty(), "{:?}", html.diagnostics);
    let document = Html::parse_fragment(&html.content);
    assert_eq!(document.root_element().text().collect::<String>(), "yes");
    let recovered = convert(Format::Html, Format::Wikitext, &html.content);
    assert!(
        recovered.diagnostics.is_empty(),
        "{:?}",
        recovered.diagnostics
    );
    assert_eq!(recovered.content, input);
}

#[test]
fn template_lookup_decodes_entities_and_uses_the_title_without_the_fragment() {
    for (source, href, expected) in [
        ("Foo#bar", "./Template:Foo#bar", "FOO"),
        ("Foo&ndash;bar", "./Template:Foo%E2%80%93bar", "foo"),
    ] {
        let input = format!("{{{{{source}}}}}");
        let result = convert(Format::Wikitext, Format::Html, &input);
        assert!(
            result.diagnostics.is_empty(),
            "{source}: {:?}",
            result.diagnostics
        );
        let document = Html::parse_fragment(&result.content);
        let group = document
            .select(&Selector::parse("[typeof='mw:Transclusion']").unwrap())
            .next()
            .unwrap();
        assert_eq!(group.text().collect::<String>(), expected);
        let metadata: Value = serde_json::from_str(group.attr("data-mw").unwrap()).unwrap();
        let target = &metadata["parts"][0]["template"]["target"];
        assert_eq!(target["href"], href);
        assert_eq!(target["wt"], source);

        let recovered = convert(Format::Html, Format::Wikitext, &result.content);
        assert!(
            recovered.diagnostics.is_empty(),
            "{:?}",
            recovered.diagnostics
        );
        assert_eq!(recovered.content, input);

        if source == "Foo&ndash;bar" {
            let upstream_href = result.content.replace(href, "./Template:Foo–bar");
            let recovered = convert(Format::Html, Format::Wikitext, &upstream_href);
            assert!(
                recovered.diagnostics.is_empty(),
                "{:?}",
                recovered.diagnostics
            );
            assert_eq!(recovered.content, input);
        }
    }
}

#[test]
fn html_preserves_template_arguments_and_rendered_content() {
    use Format::{Html as HtmlFormat, Wikitext};
    let html = convert(Wikitext, HtmlFormat, WIKI);
    let document = Html::parse_fragment(&html.content);
    let wrapper = document
        .select(&Selector::parse("p > [typeof='mw:Transclusion']").unwrap())
        .next()
        .expect("transclusion inside the paragraph");
    let group =
        Selector::parse(&format!("p > [about='{}']", wrapper.attr("about").unwrap())).unwrap();
    assert_eq!(
        document
            .select(&group)
            .flat_map(|node| node.text())
            .collect::<String>(),
        "Jin (  first  , b)"
    );
    let data: Value = serde_json::from_str(wrapper.attr("data-mw").unwrap()).unwrap();
    let call = &data["parts"][0]["template"];
    assert_eq!(call["target"]["href"], "./Template:Message");
    assert_eq!(call["params"]["1"]["wt"], "  first  ");
    assert_eq!(call["params"]["name"]["wt"], "Jin");
    let recovered = convert(HtmlFormat, Wikitext, &html.content);
    assert!(
        recovered.diagnostics.is_empty(),
        "{:?}",
        recovered.diagnostics
    );
    assert_eq!(
        convert(Wikitext, HtmlFormat, &recovered.content).content,
        html.content
    );
}

#[test]
fn complete_nowiki_and_comments_stay_within_their_parameter() {
    let input = "{{Message|<nowiki>|</nowiki>|name=Jin|a=b<!--|-->}}";
    let html = convert(Format::Wikitext, Format::Html, input);
    assert!(html.diagnostics.is_empty(), "{:?}", html.diagnostics);
    let recovered = convert(Format::Html, Format::Wikitext, &html.content);
    assert!(
        recovered.diagnostics.is_empty(),
        "{:?}",
        recovered.diagnostics
    );
    assert_eq!(recovered.content, input);
}

#[test]
fn invalid_metadata_cannot_inject_wikitext() {
    for metadata in [
        r#"{not json}"#,
        r#"{"parts":[{"template":{"target":{"wt":"Message","href":"./Template:Message"},"params":{"1":{"wt":"value}}[[Category:Injected]]{{Message"}},"i":0}}]}"#,
        r#"{"parts":[{"template":{"target":{"wt":"Message","href":"javascript:alert(1)"},"params":{},"i":0}}]}"#,
        r#"{"parts":[{"template":{"target":{"wt":"Message#injected","href":"./Template:Message%23injected"},"params":{},"i":0}}]}"#,
        r#"{"parts":[{"template":{"target":{"wt":"Message#injected","href":"./Template:Message%2523injected"},"params":{},"i":0}}]}"#,
        r#"{"parts":[{"template":{"target":{"wt":"Message<!--","href":"./Template:Message"},"params":{"1":{"wt":"-->Suffix"}},"i":0}}]}"#,
        r#"{"parts":[{"template":{"target":{"wt":"Message","href":"./Template:Message"},"params":{"1":{"wt":"value<!--"},"2":{"wt":"-->other"}},"i":0}}]}"#,
        r#"{"parts":[{"template":{"target":{"wt":"Message","href":"./Template:Message"},"params":{"1":{"wt":"value<nowiki>"},"2":{"wt":"other</nowiki>"}},"i":0}}]}"#,
        r#"{"parts":[{"template":{"target":{"wt":"Message","href":"./Template:Message"},"params":{"x":{"wt":"value","key":{"wt":"x<!--"}}},"i":0}}]}"#,
        r#"{"parts":[{"template":{"target":{"wt":"Message","href":"./Template:Message"},"params":{"x":{"wt":"value-->","key":{"wt":"x<!--"}}},"i":0}}]}"#,
        r#"{"parts":[{"template":{"target":{"wt":"Message","href":"./Template:Message"},"params":{"x":{"wt":"-->wrong=payload","key":{"wt":"x<!--"}}},"i":0}}]}"#,
        r#"{"parts":[{"template":{"target":{"wt":"Message","href":"./Template:Message"},"params":{"x":{"wt":"value","key":{"wt":"x<!--"}},"y":{"wt":"-->wrong"}},"i":0}}]}"#,
    ] {
        let input = format!("<p>Safe <span about=\"#mwt1\" typeof=\"mw:Transclusion\" data-mw='{}'>visible</span>.</p>", metadata);
        let result = convert(Format::Html, Format::Wikitext, &input);
        let rendered = convert(Format::Wikitext, Format::Html, &result.content);
        assert_eq!(
            Html::parse_fragment(&rendered.content)
                .root_element()
                .text()
                .collect::<String>(),
            "Safe visible."
        );
        assert!(!result.content.contains("{{Message"));
        assert!(result
            .diagnostics
            .iter()
            .any(|diagnostic| diagnostic.kind == rwikitext::DiagnosticKind::ConversionLoss));
    }
    assert_eq!(
        convert(
            Format::Html,
            Format::Wikitext,
            "<p>General <strong>HTML</strong>.</p>"
        )
        .content,
        "General '''HTML'''."
    );
    let malformed_parameter_info = r##"<span about="#mwt1" typeof="mw:Transclusion" data-parsoid='{"pi":["bad"]}' data-mw='{"parts":[{"template":{"target":{"wt":"Message","href":"./Template:Message"},"params":{},"i":0}}]}'>visible</span>"##;
    let result = convert(Format::Html, Format::Wikitext, malformed_parameter_info);
    assert!(result.content.contains("visible"));
    assert!(!result.content.contains("{{"));
    assert!(!result.diagnostics.is_empty());
}

#[test]
fn unset_parameters_render_literally_and_keep_the_invocation() {
    let input = "{{Message|name=Jin|x}}";
    let html = convert(Format::Wikitext, Format::Html, input);
    assert!(html.diagnostics.is_empty(), "{:?}", html.diagnostics);
    assert!(html.content.contains("{{{a}}}"));
    assert!(html.content.contains(">Jin</b>"));
    assert!(!html.content.contains("data-rwikitext-token"));
    assert_eq!(
        convert(Format::Html, Format::Wikitext, &html.content).content,
        input
    );
}

#[test]
fn repeated_explicit_arguments_keep_the_positional_argument_whitespace() {
    use Format::{Html, Wikitext};
    let first = convert(
        Wikitext,
        Html,
        "{{Message|initial|1=override|  unused  |a=b|name=Jin}}",
    );
    let recovered = convert(Html, Wikitext, &first.content);
    assert!(
        recovered.content.contains("||1=override|  unused  |"),
        "{}",
        recovered.content
    );
    let second = convert(Wikitext, Html, &recovered.content);
    let html_text = |fragment: &str| {
        scraper::Html::parse_fragment(fragment)
            .root_element()
            .text()
            .collect::<String>()
    };
    assert_eq!(html_text(&first.content), html_text(&second.content));
}

#[test]
#[ignore = "requires the verified MediaWiki archive"]
fn pinned_parsoid_template_dom_and_metadata() {
    struct Fixtures;
    impl rwikitext::WikiData for Fixtures {
        fn template_text(&self, title: &str) -> Result<Option<String>, rwikitext::LookupError> {
            Ok(match title {
                "Template:1x" => Some("{{{1}}}".into()),
                "Template:Inner_list" => Some("* item 1".into()),
                _ => None,
            })
        }
    }
    let source = std::fs::read_to_string(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/target/upstream/mediawiki-1.46.2/tests/parser/parserTests.txt"
    ))
    .unwrap()
    .replace("\r\n", "\n");
    let run = |source, target, input: &str| {
        convert_with_options(
            ConversionRequest {
                source,
                target,
                input,
                context: &WikiContext::default(),
            },
            ConversionOptions {
                wiki_data: Some(&Fixtures),
                ..Default::default()
            },
        )
        .unwrap()
    };
    let expected_groups = |html: &str| {
        let document = Html::parse_fragment(html);
        document
            .select(&Selector::parse("[data-mw][typeof~='mw:Transclusion']").unwrap())
            .map(|element| {
                let metadata: Value =
                    serde_json::from_str(element.attr("data-mw").unwrap()).unwrap();
                (
                    element.value().name().to_owned(),
                    element.text().collect::<String>(),
                    element.attr("href").map(str::to_owned),
                    metadata,
                )
            })
            .collect::<Vec<_>>()
    };
    for title in ["Templates with templated name", "Nonexistent template"] {
        let (_, case) = source.split_once(&format!("!! test\n{title}\n")).unwrap();
        let (_, case) = case.split_once("!! wikitext\n").unwrap();
        let (wiki, _) = case.split_once("\n!! html").unwrap();
        let (_, case) = case.split_once("!! html/parsoid\n").unwrap();
        let (expected, _) = case.split_once("\n!! end").unwrap();
        let actual = run(Format::Wikitext, Format::Html, wiki);
        assert!(
            actual.diagnostics.is_empty(),
            "{title}: {:?}",
            actual.diagnostics
        );
        assert_eq!(
            expected_groups(&actual.content),
            expected_groups(expected),
            "{title}"
        );
        let recovered = run(Format::Html, Format::Wikitext, expected);
        assert!(
            recovered.diagnostics.is_empty(),
            "{title}: {:?}",
            recovered.diagnostics
        );
        assert_eq!(
            expected_groups(&run(Format::Wikitext, Format::Html, &recovered.content).content),
            expected_groups(expected),
            "{title}: {:?}",
            recovered.content
        );
    }
}

fn cli(source: &str, target: &str, input: &str) -> String {
    let mut child = Command::new(env!("CARGO_BIN_EXE_rwikitext"))
        .args([
            source,
            target,
            "--wiki-data",
            concat!(env!("CARGO_MANIFEST_DIR"), "/tests/fixtures/wiki-data.json"),
        ])
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .unwrap();
    child
        .stdin
        .take()
        .unwrap()
        .write_all(input.as_bytes())
        .unwrap();
    let result = child.wait_with_output().unwrap();
    assert!(
        result.status.success(),
        "{}",
        String::from_utf8_lossy(&result.stderr)
    );
    assert!(
        result.stderr.is_empty(),
        "{}",
        String::from_utf8_lossy(&result.stderr)
    );
    String::from_utf8(result.stdout).unwrap()
}

#[test]
fn cli_round_trips_transclusion_with_local_data() {
    let html = cli("wikitext", "html", WIKI);
    assert!(html.contains("data-mw"));
    let recovered = cli("html", "wikitext", &html);
    assert!(recovered.contains("{{Message|"), "{recovered}");
    assert_eq!(cli("wikitext", "html", &recovered), html);
}
