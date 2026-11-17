use std::io::Write;
use std::process::{Command, Output, Stdio};

use pulldown_cmark::{html, Options, Parser};
use rwikitext::{
    convert_with_options, ConversionMode, ConversionOptions, ConversionRequest, DiagnosticKind,
    Format, LookupError, PageMetadata, WikiContext, WikiData,
};
use scraper::{Html, Selector};
use serde_json::Value;

#[path = "support/document.rs"]
mod document;
#[path = "support/parsoid.rs"]
mod parsoid;

use document::document_structure as body;
use Format::{Gfm, Html as HtmlFormat, Wikitext};

const CONTEXT: WikiContext<'static> = WikiContext {
    page_title: "Field notes",
    site_language: "en",
    reference_time: "2024-03-09T05:06:07Z",
};
const WIKI_DATA: &str = concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/tests/fixtures/release-wiki-data.json"
);

struct Fixture {
    name: &'static str,
    wiki: &'static str,
    markdown: &'static str,
    categories: &'static [(&'static str, Option<&'static str>)],
    redirect: Option<&'static str>,
    losses: &'static [(&'static str, &'static str)],
}

const FIXTURES: &[Fixture] = &[
    Fixture {
        name: "article",
        wiki: include_str!("fixtures/release-article.wiki"),
        markdown: include_str!("fixtures/release-article.md"),
        categories: &[("Field work", Some("Notes & maps")), ("Guides", None)],
        redirect: None,
        losses: &[
            ("[[Category:Field work|Notes & maps]]", "category"),
            ("[[Category:Guides]]", "category"),
        ],
    },
    Fixture {
        name: "redirect",
        wiki: include_str!("fixtures/release-redirect.wiki"),
        markdown: "",
        categories: &[("Field work", Some("Redirects"))],
        redirect: Some("Main Page"),
        losses: &[
            ("#REDIRECT: [[Main Page]]", "redirect"),
            ("[[Category:Field work|Redirects]]", "category"),
        ],
    },
];

impl Fixture {
    fn metadata(&self) -> PageMetadata {
        PageMetadata {
            categories: self
                .categories
                .iter()
                .map(|(name, _)| (*name).into())
                .collect(),
            category_sort_keys: self
                .categories
                .iter()
                .map(|(_, key)| key.map(str::to_owned))
                .collect(),
            redirect_target: self.redirect.map(str::to_owned),
            display_title: None,
        }
    }
}

struct LocalWiki(Value);

impl WikiData for LocalWiki {
    fn template_text(&self, title: &str) -> Result<Option<String>, LookupError> {
        Ok(self.0["templates"][title].as_str().map(str::to_owned))
    }

    fn page_exists(&self, title: &str) -> Result<Option<bool>, LookupError> {
        Ok(self.0["pages"][title].as_bool())
    }
}

fn request(source: Format, target: Format, input: &str) -> ConversionRequest<'_> {
    ConversionRequest {
        source,
        target,
        input,
        context: &CONTEXT,
    }
}

fn with_stdin(command: &mut Command, input: &str) -> Output {
    let mut child = command
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("start renderer or converter");
    child
        .stdin
        .take()
        .unwrap()
        .write_all(input.as_bytes())
        .unwrap();
    child.wait_with_output().unwrap()
}

fn cli(source: &str, target: &str, input: &str, strict: bool) -> Output {
    let mut command = Command::new(env!("CARGO_BIN_EXE_rwikitext"));
    command
        .args([source, target, "--wiki-data", WIKI_DATA])
        .args(["--page-title", CONTEXT.page_title])
        .args(["--site-language", CONTEXT.site_language])
        .args(["--reference-time", CONTEXT.reference_time]);
    if strict {
        command.arg("--strict");
    }
    with_stdin(&mut command, input)
}

fn rendered(output: Output) -> String {
    assert!(output.status.success(), "{:?}", output);
    assert!(
        output.stderr.is_empty(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    String::from_utf8(output.stdout).unwrap()
}

fn render_parsoid(wiki: &str) -> String {
    rendered(with_stdin(&mut parsoid::parsoid_command(&CONTEXT), wiki))
}

fn render_gfm(markdown: &str) -> String {
    let mut output = String::new();
    let options = Options::ENABLE_TABLES;
    html::push_html(&mut output, Parser::new_ext(markdown, options));
    output
}

fn decode_url_component(input: &str) -> String {
    let mut bytes = Vec::new();
    let mut remainder = input.as_bytes();
    while let Some((&byte, tail)) = remainder.split_first() {
        if byte == b'%' {
            let hex = std::str::from_utf8(&tail[..2]).unwrap();
            bytes.push(u8::from_str_radix(hex, 16).unwrap());
            remainder = &tail[2..];
        } else {
            bytes.push(byte);
            remainder = tail;
        }
    }
    String::from_utf8(bytes).unwrap()
}

fn html_metadata(html: &str) -> PageMetadata {
    let document = Html::parse_fragment(html);
    let properties = Selector::parse("link[rel^='mw:PageProp/']").unwrap();
    let mut result = PageMetadata::default();
    for link in document.select(&properties) {
        let rel = link.attr("rel").unwrap();
        let href = link.attr("href").unwrap();
        match rel {
            "mw:PageProp/Category" => {
                let title = href.strip_prefix("./Category:").unwrap();
                let (name, key) = title.split_once('#').map_or((title, None), |(name, key)| {
                    (name, Some(decode_url_component(key)))
                });
                result
                    .categories
                    .push(decode_url_component(name).replace('_', " "));
                result.category_sort_keys.push(key);
            }
            "mw:PageProp/redirect" => {
                let title = href.strip_prefix("./").unwrap();
                assert!(result
                    .redirect_target
                    .replace(decode_url_component(title).replace('_', " "))
                    .is_none());
            }
            other => panic!("unexpected page property {other}"),
        }
    }
    result
}

fn template_calls(html: &str) -> Vec<(String, Value)> {
    let document = Html::parse_fragment(html);
    let transclusions = Selector::parse("[typeof~='mw:Transclusion'][data-mw]").unwrap();
    let mut calls = Vec::new();
    for element in document.select(&transclusions) {
        let data: Value = serde_json::from_str(element.attr("data-mw").unwrap()).unwrap();
        for part in data["parts"].as_array().unwrap() {
            if let Some(call) = part.get("template") {
                calls.push((
                    call["target"]["href"].as_str().unwrap().into(),
                    call["params"].clone(),
                ));
            }
        }
    }
    calls
}

fn heading_anchors(html: &str) -> Vec<String> {
    let document = Html::parse_fragment(html);
    document
        .select(&Selector::parse("h2[id], h3[id]").unwrap())
        .map(|element| element.attr("id").unwrap().to_owned())
        .collect()
}

#[test]
fn api_and_cli_report_meaning_lost_from_gfm_bodies() {
    let data =
        LocalWiki(serde_json::from_str(include_str!("fixtures/release-wiki-data.json")).unwrap());
    let options = ConversionOptions {
        wiki_data: Some(&data),
        ..ConversionOptions::default()
    };
    for fixture in FIXTURES {
        let convert = |mode| {
            convert_with_options(
                request(Wikitext, Gfm, fixture.wiki),
                ConversionOptions { mode, ..options },
            )
        };
        let result = convert(ConversionMode::BestEffort).unwrap();
        assert_eq!(result.metadata, fixture.metadata(), "{}", fixture.name);
        assert_eq!(
            result.diagnostics.len(),
            fixture.losses.len(),
            "{:?}",
            result.diagnostics
        );
        for (diagnostic, (source, kind)) in result.diagnostics.iter().zip(fixture.losses) {
            assert_eq!(diagnostic.kind, DiagnosticKind::ConversionLoss);
            assert_eq!(&fixture.wiki[diagnostic.span.clone()], *source);
            assert!(
                diagnostic.message.contains(&format!("loses the {kind}")),
                "{diagnostic}"
            );
        }
        assert_eq!(
            convert(ConversionMode::Strict).unwrap_err().diagnostics(),
            result.diagnostics
        );

        let expected = body(&render_gfm(fixture.markdown));
        assert_eq!(
            body(&render_gfm(&result.content)),
            expected,
            "API {}",
            fixture.name
        );

        for strict in [false, true] {
            let output = cli("wikitext", "gfm", fixture.wiki, strict);
            assert_eq!(output.status.success(), !strict);
            let stdout = String::from_utf8(output.stdout).unwrap();
            let stderr = String::from_utf8(output.stderr).unwrap();
            if strict {
                assert!(stdout.is_empty());
            } else {
                assert_eq!(body(&render_gfm(&stdout)), expected, "CLI {}", fixture.name);
            }
            let lines: Vec<_> = stderr.lines().collect();
            assert_eq!(lines.len(), fixture.losses.len(), "{stderr}");
            for (line, (source, kind)) in lines.iter().zip(fixture.losses) {
                let start = fixture.wiki.find(source).unwrap();
                let end = start + source.len();
                assert!(
                    line.contains(&format!("conversion loss at bytes {start}..{end}:")),
                    "{line}"
                );
                assert!(line.contains(&format!("loses the {kind}")), "{line}");
            }
        }
    }
}

#[test]
#[ignore = "requires the pinned MediaWiki release and PHP; see docs/compatibility.md"]
fn pinned_parsoid_confirms_release_document_round_trips() {
    let data =
        LocalWiki(serde_json::from_str(include_str!("fixtures/release-wiki-data.json")).unwrap());
    let api = |source, target, input: &str| {
        convert_with_options(
            request(source, target, input),
            ConversionOptions {
                wiki_data: Some(&data),
                ..ConversionOptions::default()
            },
        )
        .unwrap()
    };

    for fixture in FIXTURES {
        let original = render_parsoid(fixture.wiki);
        let expected_body = body(&render_gfm(fixture.markdown));
        assert_eq!(body(&original), expected_body, "reference {}", fixture.name);
        assert_eq!(
            html_metadata(&original),
            fixture.metadata(),
            "reference {}",
            fixture.name
        );
        let expected_calls = template_calls(&original);
        let expected_anchors = heading_anchors(&original);

        let check_html = |html: &str, route: &str| {
            assert_eq!(body(html), expected_body, "{route} {}", fixture.name);
            assert_eq!(
                html_metadata(html),
                fixture.metadata(),
                "{route} {}",
                fixture.name
            );
            assert_eq!(
                template_calls(html),
                expected_calls,
                "{route} {}",
                fixture.name
            );
            assert_eq!(
                heading_anchors(html),
                expected_anchors,
                "{route} {}",
                fixture.name
            );
        };
        let check_recovered_gfm = |wiki: &str, route: &str| {
            let html = render_parsoid(wiki);
            assert_eq!(
                body(&html),
                expected_body,
                "{route} {}\n{wiki}",
                fixture.name
            );
            assert_eq!(
                html_metadata(&html),
                PageMetadata::default(),
                "{route} {}",
                fixture.name
            );
            assert!(template_calls(&html).is_empty(), "{route} {}", fixture.name);
            assert_eq!(
                heading_anchors(&html),
                expected_anchors,
                "{route} {}",
                fixture.name
            );
        };

        let html = api(Wikitext, HtmlFormat, fixture.wiki);
        assert_eq!(html.metadata, fixture.metadata());
        assert!(html.diagnostics.is_empty(), "{:?}", html.diagnostics);
        check_html(&html.content, "API HTML export");
        let recovered = api(HtmlFormat, Wikitext, &html.content);
        assert_eq!(recovered.metadata, fixture.metadata());
        assert!(
            recovered.diagnostics.is_empty(),
            "{:?}",
            recovered.diagnostics
        );
        check_html(&render_parsoid(&recovered.content), "API HTML round trip");

        let cli_html = rendered(cli("wikitext", "html", fixture.wiki, false));
        check_html(&cli_html, "CLI HTML export");
        let recovered = rendered(cli("html", "wikitext", &cli_html, false));
        check_html(&render_parsoid(&recovered), "CLI HTML round trip");

        let markdown = api(Wikitext, Gfm, fixture.wiki).content;
        let recovered = api(Gfm, Wikitext, &markdown);
        assert_eq!(recovered.metadata, PageMetadata::default());
        assert!(
            recovered.diagnostics.is_empty(),
            "{:?}",
            recovered.diagnostics
        );
        check_recovered_gfm(&recovered.content, "API GFM round trip");

        let markdown = cli("wikitext", "gfm", fixture.wiki, false);
        assert!(markdown.status.success(), "{markdown:?}");
        let markdown = String::from_utf8(markdown.stdout).unwrap();
        let recovered = rendered(cli("gfm", "wikitext", &markdown, false));
        check_recovered_gfm(&recovered, "CLI GFM round trip");
    }
}
