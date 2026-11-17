use rwikitext::{
    convert_with_options, ConversionError, ConversionOptions, ConversionRequest, ConversionResult,
    DiagnosticKind, FileInfo, Format, LookupError, WikiContext, WikiData,
};
use scraper::Html;
use serde_json::Value;
use std::collections::HashMap;
use std::path::Path;

#[path = "support/document.rs"]
mod document;

#[path = "support/parser_case_audit.rs"]
mod parser_case_audit;

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
            let normalized_ns = match ns.trim().replace(' ', "_").to_ascii_lowercase().as_str() {
                "media" => "Media".into(),
                "special" => "Special".into(),
                "talk" => "Talk".into(),
                "user" => "User".into(),
                "user_talk" => "User_talk".into(),
                "project" => "Project".into(),
                "project_talk" => "Project_talk".into(),
                "mediawiki" => "MediaWiki".into(),
                "mediawiki_talk" => "MediaWiki_talk".into(),
                "template" => "Template".into(),
                "template_talk" => "Template_talk".into(),
                "help" => "Help".into(),
                "help_talk" => "Help_talk".into(),
                "file" | "image" => "File".into(),
                "file_talk" | "image_talk" => "File_talk".into(),
                "category" => "Category".into(),
                "category_talk" => "Category_talk".into(),
                _ => ns.trim().replace(' ', "_"),
            };
            (format!("{normalized_ns}:"), title.trim())
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
    render_wikitext(
        section(case, "wikitext").expect("upstream wikitext input"),
        &context,
        wiki,
    )
    .unwrap_or_else(|e| panic!("convert upstream case {title:?}: {e}"))
}

fn render_wikitext(
    input: &str,
    context: &WikiContext<'_>,
    wiki: &Fixture<'_>,
) -> Result<ConversionResult, ConversionError> {
    convert_with_options(
        ConversionRequest {
            source: Format::Wikitext,
            target: Format::Html,
            input,
            context,
        },
        ConversionOptions {
            wiki_data: Some(wiki),
            ..Default::default()
        },
    )
}

fn expected_html<'a>(case: &[(&str, &'a str)]) -> &'a str {
    section(case, "html/parsoid+standalone")
        .or_else(|| section(case, "html/parsoid"))
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
            "Templates: Recognize targets when newlines and comments don't split the target",
            "Templates: Handle comments in the target",
            "Templates: Handle empty comment-and-ws-only lines correctly",
            "Transclusion with a (stripped) fragment",
            "Template titles with entities in them",
        ],
    );
}

#[test]
#[ignore = "requires the verified MediaWiki archive; see docs/compatibility.md"]
fn unusual_parameter_keys_and_brace_precedence() {
    check_cases(
        PARSOID_CASES,
        &[
            "Templates: Other wikitext in parameter names (T69657)",
            "Templates: Preserve blank parameter names",
            "Templates: Preserve blank parameter names in other positions",
            "Template parameter named \"constructor\"",
        ],
    );
    check_cases(
        "tests/parser/preprocessor.txt",
        &[
            "Preprocessor precedence 1: link is rightmost opening",
            "Preprocessor precedence 5: tplarg takes precedence over template",
            "Preprocessor precedence 9: groups of braces",
            "Preprocessor precedence 11: found during visual diff testing",
        ],
    );
    let source = read_fixture("tests/parser/preprocessor.txt");
    let wiki = Fixture::parse(&source);
    let title = "Preprocessor precedence 6: broken link is rightmost opening";
    let case = wiki.case(title);
    let actual = rendered(case, title, &wiki);
    assert_eq!(
        document_structure(&actual.content),
        document_structure(expected_html(case))
    );
    let input = section(case, "wikitext").unwrap();
    let losses: Vec<_> = actual
        .diagnostics
        .iter()
        .map(|diagnostic| {
            assert_eq!(diagnostic.kind, DiagnosticKind::ConversionLoss);
            &input[diagnostic.span.clone()]
        })
        .collect();
    assert_eq!(losses, ["{{1x|[[Foo}}", "{{1x|[[Foo|bar|bat=baz}}"]);
    check_cases(
        CORE_CASES,
        &[
            "Include directive string gluing",
            "1. Include directive in template target",
            "2. Include directive in template target",
            "3. Include directive in template target",
            "4. Include directive in template target",
        ],
    );
}

#[test]
#[ignore = "requires the verified MediaWiki archive; see docs/compatibility.md"]
fn template_expansion_can_start_its_own_block() {
    check_cases(
        CORE_CASES,
        &["T2529: Template with table, not included at beginning of line"],
    );
    let source = read_fixture(CORE_CASES);
    let wiki = Fixture::parse(&source);
    let title = "Implicit newline insertion/sol when expanding templates (T241150/T14974/T2529)";
    let case = wiki.case(title);
    let actual = rendered(case, title, &wiki);
    assert_eq!(
        document_structure(&actual.content),
        document_structure(expected_html(case))
    );
    let input = section(case, "wikitext").unwrap();
    let losses: Vec<_> = actual
        .diagnostics
        .iter()
        .map(|diagnostic| {
            assert_eq!(diagnostic.kind, DiagnosticKind::ConversionLoss);
            &input[diagnostic.span.clone()]
        })
        .collect();
    assert_eq!(losses, ["{{1x|#foo}}", "{{1x|#foo}}"]);
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
fn core_tag_function_handles_nowiki_and_pre() {
    check_cases(
        "tests/parser/extTags.txt",
        &[
            "Extension tags in extension content: <nowiki>",
            "Extension tags in extension content: <pre>",
            "Extension tags in extension attributes: <pre>",
            "Extension tags in extension attributes: bogus tag name",
            "Extension tags in extension name",
            "Doubly-nested tags",
        ],
    );
}

#[test]
#[ignore = "requires the verified MediaWiki archive; see docs/compatibility.md"]
fn wiki_context_controls_language_sensitive_functions() {
    check_cases(
        MAGIC_WORDS,
        &[
            "Magic Word: {{TALKSPACE}}",
            "Magic Word: {{TALKSPACE}}, same namespace",
            "Magic Word: {{TALKSPACE}}, main namespace",
            "Magic Word: {{TALKSPACEE}}",
            "Magic Word: {{SUBJECTSPACE}}",
            "Magic Word: {{SUBJECTSPACE}}, same namespace",
            "Magic Word: {{SUBJECTSPACE}}, main namespace",
            "Magic Word: {{SUBJECTSPACEE}}",
            "Magic Word: {{NAMESPACE}}",
            "Magic Word: {{NAMESPACEE}}",
            "Magic Word: {{NAMESPACENUMBER}}",
            "Magic Word: {{SUBPAGENAME}}",
            "Magic Word: {{SUBPAGENAMEE}}",
            "Magic Word: {{ROOTPAGENAME}}",
            "Magic Word: {{ROOTPAGENAMEE}}",
            "Magic Word: {{BASEPAGENAME}}",
            "Magic Word: {{BASEPAGENAMEE}}",
            "Magic Word: {{TALKPAGENAME}}",
            "Magic Word: {{TALKPAGENAMEE}}",
            "Magic Word: {{SUBJECTPAGENAME}}",
            "Magic Word: {{SUBJECTPAGENAMEE}}",
            "Magic Word: {{PAGENAME}}",
            "Magic Word: {{PAGENAME}} with metacharacters",
            "Magic Word: {{PAGENAME}} with metacharacters (T28781)",
            "Magic Word: {{PAGENAMEE}}",
            "Magic Word: {{PAGENAMEE}} with metacharacters (T28781)",
            "Magic Word: {{FULLPAGENAME}}",
            "Magic Word: {{FULLPAGENAMEE}}",
            "Magic Word: {{PAGELANGUAGE}}",
            "Magic Word: {{PAGELANGUAGE}} on a page with no explicitly set language",
            "Case-sensitive magic words, when cased differently, should just be template transclusions",
        ],
    );
    check_cases(
        CORE_CASES,
        &[
            "T33490 Turkish: ucfirst 'blah'",
            "T33490 Turkish: lcfirst 'BLAH'",
            "Namespace 1 {{ns:1}}",
            "Namespace 1 {{ns:01}}",
            "Namespace 0 {{ns:0}} (T6783)",
            "Namespace 0 {{ns:00}} (T6783)",
            "Namespace -1 {{ns:-1}}",
            "Namespace User {{ns:User}}",
            "Namespace User talk {{ns:User_talk}}",
            "Namespace User talk {{ns:uSeR tAlK}}",
            "Namespace File {{ns:File}}",
            "Namespace File {{ns:Image}}",
            "Urlencode",
            "Strip marker in urlencode",
            "Strip marker in lc",
            "Strip marker in uc",
            "anchorencode",
            "anchorencode trims spaces",
            "anchorencode deals with links",
            "anchorencode deals with templates",
            "Strip marker in anchorencode",
            "Namespaced link must have a title",
            "Namespaced link must have a title (bad fragment version)",
            "Nowikis can be considered pwrap optional based on content",
            "Entities inside <nowiki>",
            "Link with space in namespace",
            "Templates: Parameters substituted at the top-level",
            "Parsoid: Serialize link to category page with colon escape",
            "Fuzz testing: URL adjacent extension (no space, dirty; nowiki)",
            "WTS of autolinks with nowikis (round-trip)",
        ],
    );
    check_cases(
        "tests/parser/definitionLists.txt",
        &[
            "Definition list with URL link",
            "Definition list with bracketed URL link",
            "Definition list with news link containing colon",
            "Malformed definition list with colon",
            "Definition lists: colon in external link text",
            "Definition lists: colon in HTML attribute",
            "Second colon on line: No templates involved",
        ],
    );
    check_cases(
        "tests/parser/redirects.txt",
        &["Redirect wikitext become ol list items when they come from a template"],
    );
    check_cases(
        "tests/parser/pst.txt",
        &["Parsoid: Recognize nowiki with trailing space in tags"],
    );
    let source = read_fixture(CORE_CASES);
    let wiki = Fixture::parse(&source);
    let title = "Non extlink in wikilink";
    let case = wiki.case(title);
    let actual = rendered(case, title, &wiki);
    assert_eq!(
        document_structure(&actual.content),
        document_structure(expected_html(case))
    );
    let input = section(case, "wikitext").unwrap();
    let losses: Vec<_> = actual
        .diagnostics
        .iter()
        .map(|diagnostic| {
            assert_eq!(diagnostic.kind, DiagnosticKind::ConversionLoss);
            &input[diagnostic.span.clone()]
        })
        .collect();
    assert_eq!(losses, ["{{1x|123}}"]);
    check_cases(
        "tests/parser/comments.txt",
        &[
            "Comment on its own line post-expand",
            "Comment on its own line post-expand with non-significant whitespace",
        ],
    );
    check_cases(
        "tests/parser/quotes.txt",
        &[
            "Italics and bold: 4-quote opening sequence: (4,4) w/ nowiki",
            "Italics and bold: multiple quote sequences: (2,4,2+3) w/ nowiki",
            "Italics and bold: multiple quote sequences: (2,4,3+2) w/ nowiki",
            "Italics and bold: multiple quote sequences: (2,4,4+2) w/ nowiki",
            "Italics and bold: other quote tests: (3,2,3+2+2,2)",
        ],
    );
}

#[test]
#[ignore = "requires the verified MediaWiki archive; see docs/compatibility.md"]
fn reference_time_controls_utc_weekday_and_week() {
    check_cases(
        MAGIC_WORDS,
        &[
            "Magic Words LOCAL (UTC)",
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
