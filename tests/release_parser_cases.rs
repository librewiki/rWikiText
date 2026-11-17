use rwikitext::{
    convert_with_options, ConversionOptions, ConversionRequest, ConversionResult, DiagnosticKind,
    FileInfo, Format, LookupError, WikiContext, WikiData,
};
use scraper::Html;
use serde_json::Value;
use std::collections::HashMap;
use std::path::Path;

#[path = "support/document.rs"]
mod document;

use document::document_structure;

const RELEASE: &str = concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/target/upstream/mediawiki-1.46.2"
);
const CONDITIONALS: &str = "extensions/ParserFunctions/tests/parser/funcsParserTests.txt";
const CORE_CASES: &str = "tests/parser/parserTests.txt";
const MAGIC_WORDS: &str = "tests/parser/magicWords.txt";
const PARSOID_CASES: &str = "vendor/wikimedia/parsoid/tests/parser/parserTests.txt";
const PARSER_FUNCTION_METADATA: &str =
    "vendor/wikimedia/parsoid/tests/parser/v3ParserFunctions.txt";

struct Fixture<'a> {
    sections: Vec<(&'a str, &'a str)>,
    articles: HashMap<String, &'a str>,
}

impl<'a> Fixture<'a> {
    fn parse(source: &'a str) -> Self {
        let mut sections = Vec::new();
        let mut section = None;
        let mut offset = 0;
        for line in source.split_inclusive('\n') {
            if let Some(marker) = line.strip_prefix("!!") {
                if let Some((name, start)) = section.replace((marker.trim(), offset + line.len())) {
                    let content = &source[start..offset];
                    sections.push((name, content.strip_suffix('\n').unwrap_or(content)));
                }
            }
            offset += line.len();
        }
        if let Some((name, start)) = section {
            sections.push((name, &source[start..]));
        }

        let articles = sections
            .windows(2)
            .filter_map(|pair| match pair {
                [("article", title), ("text", body)] => Some((article_title(title), *body)),
                _ => None,
            })
            .collect();
        Self { sections, articles }
    }

    fn case(&self, title: &str) -> &[(&'a str, &'a str)] {
        let start = self
            .sections
            .iter()
            .position(|&(name, value)| name == "test" && value.trim() == title)
            .unwrap_or_else(|| panic!("missing upstream case {title:?}"));
        let end = self.sections[start + 1..]
            .iter()
            .position(|&(name, _)| name == "end")
            .expect("upstream test has a closing marker");
        &self.sections[start + 1..start + 1 + end]
    }
}

fn article_title(source: &str) -> String {
    let (namespace, title) = source
        .trim()
        .split_once(':')
        .map_or((String::new(), source.trim()), |(ns, title)| {
            (format!("{ns}:"), title.trim())
        });
    let mut letters = title.chars();
    let initial = letters.next().into_iter().flat_map(char::to_uppercase);
    namespace + &initial.chain(letters).collect::<String>().replace(' ', "_")
}

impl WikiData for Fixture<'_> {
    fn template_text(&self, title: &str) -> Result<Option<String>, LookupError> {
        Ok(self.articles.get(title).map(|&body| body.to_owned()))
    }

    fn page_exists(&self, title: &str) -> Result<Option<bool>, LookupError> {
        Ok(self.articles.contains_key(title).then_some(true))
    }

    fn file_info(&self, title: &str) -> Result<Option<FileInfo>, LookupError> {
        // The ParserFunctions suite resolves redirect targets before testing file existence.
        let redirects_to_fixture_file =
            self.articles.get(title) == Some(&"#REDIRECT[[File:Foobar.jpg]]");
        Ok(
            (title == "File:Foobar.jpg" || redirects_to_fixture_file).then(|| FileInfo {
                url: "https://example.org/Foobar.jpg".into(),
                width: None,
                height: None,
            }),
        )
    }
}

fn section<'a>(case: &[(&str, &'a str)], name: &str) -> Option<&'a str> {
    case.iter()
        .find_map(|&(key, value)| (key == name).then_some(value))
}

fn read_fixture(path: &str) -> String {
    std::fs::read_to_string(Path::new(RELEASE).join(path))
        .unwrap_or_else(|e| panic!("read upstream fixture {path}: {e}"))
        .replace("\r\n", "\n")
}

fn rendered(case: &[(&str, &str)], title: &str, wiki: &Fixture<'_>) -> ConversionResult {
    let options = section(case, "options").unwrap_or_default();
    let context = WikiContext {
        page_title: options
            .split_once("title=[[")
            .and_then(|(_, rest)| rest.split_once("]]"))
            .map_or("Parser test", |(title, _)| title),
        site_language: options
            .lines()
            .find_map(|line| line.strip_prefix("language="))
            .unwrap_or("en"),
        reference_time: "1970-01-01T00:02:03Z",
    };
    convert_with_options(
        ConversionRequest {
            source: Format::Wikitext,
            target: Format::Html,
            input: section(case, "wikitext").expect("upstream wikitext input"),
            context: &context,
        },
        ConversionOptions {
            wiki_data: Some(wiki),
            ..Default::default()
        },
    )
    .unwrap_or_else(|e| panic!("convert upstream case {title:?}: {e}"))
}

fn expected_html<'a>(case: &[(&str, &'a str)]) -> &'a str {
    section(case, "html/parsoid")
        .or_else(|| section(case, "html/parsoid+integrated"))
        .or_else(|| section(case, "html"))
        .or_else(|| section(case, "html/*"))
        .or_else(|| section(case, "html/php"))
        .expect("upstream rendered HTML")
}

fn transclusion_metadata(html: &str) -> Vec<(String, Value)> {
    let document = Html::parse_fragment(html);
    let selector = scraper::Selector::parse("[typeof~='mw:Transclusion']").unwrap();
    document
        .select(&selector)
        .map(|node| {
            (
                node.attr("typeof").unwrap().to_owned(),
                serde_json::from_str(node.attr("data-mw").unwrap()).unwrap(),
            )
        })
        .collect()
}

fn check_cases(path: &str, titles: &[&str]) {
    let source = read_fixture(path);
    let wiki = Fixture::parse(&source);
    for title in titles {
        let case = wiki.case(title);
        let actual = rendered(case, title, &wiki);
        assert_eq!(
            document_structure(&actual.content),
            document_structure(expected_html(case)),
            "{path}: {title}\n{}",
            actual.content
        );
        assert!(
            actual.diagnostics.is_empty(),
            "{path}: {title}: {:?}",
            actual.diagnostics
        );
    }
}

#[test]
#[ignore = "requires the verified MediaWiki archive; see docs/compatibility.md"]
fn conditional_comparison_decodes_entities_once() {
    check_cases(CONDITIONALS, &["#ifeq entities"]);
}

#[test]
#[ignore = "requires the verified MediaWiki archive; see docs/compatibility.md"]
fn switch_uses_exact_default_labels_and_mediawiki_equality() {
    check_cases(
        CONDITIONALS,
        &[
            "Preserve tags in #switch default value",
            "#switch should not match #default as a prefix",
            "#switch Test php truthy",
        ],
    );
}

#[test]
#[ignore = "requires the verified MediaWiki archive; see docs/compatibility.md"]
fn conditional_arguments_expand_templates_and_trim_branches() {
    check_cases(
        CONDITIONALS,
        &["Templates: Parser functions don't strip whitespace from positional parameters"],
    );
}

#[test]
#[ignore = "requires the verified MediaWiki archive; see docs/compatibility.md"]
fn ifexist_uses_caller_resolved_file_data_for_media_redirects() {
    check_cases(CONDITIONALS, &["{{#ifexist:}} media redirect (bug 32031)"]);
}

#[test]
#[ignore = "requires the verified MediaWiki archive; see docs/compatibility.md"]
fn switch_defaults_inside_literal_angle_brackets_preserve_text_without_loss() {
    check_cases(
        CONDITIONALS,
        &["Bug 19093: Default values don't fall through in switch"],
    );
}

#[test]
#[ignore = "requires the verified MediaWiki archive; see docs/compatibility.md"]
fn templates_expand_arguments_and_resolve_names() {
    check_cases(
        PARSOID_CASES,
        &[
            "Template with explicit namespace",
            "Template with template name as unnamed argument",
            "Template with template name as argument",
            "Template with template name as argument (2)",
            "Template with argument in separate line",
            "Template with complex arguments",
            "Template passing argument to another template",
            "Templates: Strip leading and trailing whitespace from named-param values",
        ],
    );
}

#[test]
#[ignore = "requires the verified MediaWiki archive; see docs/compatibility.md"]
fn template_positional_whitespace_controls_blocks() {
    let source = read_fixture(PARSOID_CASES);
    let wiki = Fixture::parse(&source);
    let title = "Templates: Don't strip whitespace from positional-param values";
    let case = wiki.case(title);
    let actual = rendered(case, title, &wiki);
    let expected = expected_html(case);
    assert_eq!(
        document_structure(&actual.content),
        document_structure(expected)
    );
    let preformatted = |html: &str| {
        Html::parse_fragment(html)
            .select(&scraper::Selector::parse("pre").unwrap())
            .map(|pre| pre.text().collect::<String>())
            .collect::<Vec<_>>()
    };
    assert_eq!(preformatted(&actual.content), preformatted(expected));
    let input = section(case, "wikitext").unwrap();
    assert_eq!(actual.diagnostics.len(), 1);
    assert_eq!(actual.diagnostics[0].kind, DiagnosticKind::ConversionLoss);
    assert_eq!(&input[actual.diagnostics[0].span.clone()], "{{1x|\n }}");
}

#[test]
#[ignore = "requires the verified MediaWiki archive; see docs/compatibility.md"]
fn templates_expand_named_parameters_with_comments() {
    let source = read_fixture(PARSOID_CASES);
    let wiki = Fixture::parse(&source);
    let title = "Templates: Handle comments in parameter names (T69657)";
    let case = wiki.case(title);
    let expected = expected_html(case);
    let actual = rendered(case, title, &wiki);
    assert!(actual.diagnostics.is_empty(), "{:?}", actual.diagnostics);
    let parameters = |html: &str| {
        transclusion_metadata(html)
            .into_iter()
            .map(|(_, data)| data["parts"][0]["template"]["params"].clone())
            .collect::<Vec<_>>()
    };
    assert_eq!(
        document_structure(&actual.content),
        document_structure(expected)
    );
    assert_eq!(parameters(&actual.content), parameters(expected));

    let options = ConversionOptions {
        wiki_data: Some(&wiki),
        ..Default::default()
    };
    let context = WikiContext::default();
    let recovered = convert_with_options(
        ConversionRequest {
            source: Format::Html,
            target: Format::Wikitext,
            input: expected,
            context: &context,
        },
        options,
    )
    .unwrap();
    assert!(
        recovered.diagnostics.is_empty(),
        "{:?}",
        recovered.diagnostics
    );
    let rerendered = convert_with_options(
        ConversionRequest {
            source: Format::Wikitext,
            target: Format::Html,
            input: &recovered.content,
            context: &context,
        },
        options,
    )
    .unwrap();
    assert!(
        rerendered.diagnostics.is_empty(),
        "{:?}",
        rerendered.diagnostics
    );
    assert_eq!(
        document_structure(&rerendered.content),
        document_structure(expected)
    );
    assert_eq!(parameters(&rerendered.content), parameters(expected));
}

#[test]
#[ignore = "requires the verified MediaWiki archive; see docs/compatibility.md"]
fn equals_escape_and_explicit_template_name_are_distinct() {
    check_cases(
        "tests/parser/pfeqParserTests.txt",
        &[
            "Basic usage of {{=}}",
            "Using {{Template:=}} doesn't invoke the parser function",
        ],
    );
}

#[test]
#[ignore = "requires the verified MediaWiki archive; see docs/compatibility.md"]
fn wiki_context_controls_language_sensitive_functions() {
    check_cases(
        MAGIC_WORDS,
        &[
            "Magic Word: {{NAMESPACE}}",
            "Magic Word: {{PAGELANGUAGE}}",
            "Magic Word: {{PAGELANGUAGE}} on a page with no explicitly set language",
        ],
    );
    check_cases(
        CORE_CASES,
        &[
            "T33490 Turkish: ucfirst 'blah'",
            "T33490 Turkish: lcfirst 'BLAH'",
        ],
    );
}

#[test]
#[ignore = "requires the verified MediaWiki archive; see docs/compatibility.md"]
fn reference_time_controls_utc_weekday_and_week() {
    check_cases(
        MAGIC_WORDS,
        &[
            "Magic Word: {{CURRENTDOW}}",
            "Magic Word: {{CURRENTWEEK}} (T6594)",
        ],
    );
}

#[test]
#[ignore = "requires the verified MediaWiki archive; see docs/compatibility.md"]
fn parser_function_metadata_uses_pinned_parsoid_html_version() {
    let source = read_fixture(PARSER_FUNCTION_METADATA);
    let wiki = Fixture::parse(&source);
    let title = "When ParsoidExperimentalParserFunctionOutput is disabled, Parsoid should generate v2.x HTML for parser functions";
    let case = wiki.case(title);
    let actual = rendered(case, title, &wiki);
    let expected = section(case, "html/parsoid").unwrap();
    assert!(actual.diagnostics.is_empty());
    assert_eq!(
        document_structure(&actual.content),
        document_structure(expected)
    );
    assert_eq!(
        transclusion_metadata(&actual.content),
        transclusion_metadata(expected)
    );
}

#[test]
fn escaped_delimiters_preserve_blocks_and_report_invocation_loss() {
    let table = "{|\n| a {{!}}{{!}} b\n|}";
    for (input, expected, invocations) in [
        ("={{=}} Title ==", "<h2>Title</h2>", &["{{=}}"][..]),
        (
            table,
            "<table><tr><td>a</td><td>b</td></tr></table>",
            &["{{!}}", "{{!}}"][..],
        ),
    ] {
        let result = rwikitext::convert(ConversionRequest {
            source: Format::Wikitext,
            target: Format::Html,
            input,
            context: &WikiContext::default(),
        })
        .unwrap();
        assert_eq!(
            document_structure(&result.content),
            document_structure(expected)
        );
        let reported = result
            .diagnostics
            .iter()
            .map(|diagnostic| {
                assert_eq!(diagnostic.kind, DiagnosticKind::ConversionLoss);
                &input[diagnostic.span.clone()]
            })
            .collect::<Vec<_>>();
        assert_eq!(reported, invocations);
    }
}
