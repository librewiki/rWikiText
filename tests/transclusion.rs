use rwikitext::{
    convert_with_options, ConversionOptions, ConversionRequest, DiagnosticKind, ExpansionLimits,
    Format, LookupError, WikiContext, WikiData,
};
use std::cell::RefCell;

#[derive(Default)]
struct Data {
    calls: RefCell<Vec<String>>,
}
impl WikiData for Data {
    fn template_text(&self, title: &str) -> Result<Option<String>, LookupError> {
        self.calls.borrow_mut().push(title.to_owned());
        match title {
            "Template:Outer" => Ok(Some("[{{{1|default}}}/{{{name|fallback}}}] {{Inner|{{{1}}}}}".into())),
            "Template:Inner" => Ok(Some("{{{1}}}".into())),
            "Template:Inclusion" => Ok(Some("outside<noinclude>hidden</noinclude><onlyinclude>inside<includeonly>included</includeonly></onlyinclude>tail".into())),
            "Template:Loop" => Ok(Some("{{Loop}}".into())),
            "Template:Big" => Ok(Some("Ω界".into())),
            "Template:One" => Ok(Some("1".into())),
            "Template:1x" => Ok(Some("{{{1}}}".into())),
            "Template:Table" => Ok(Some("{|\n| 1 || 2\n|-\n| 3 || 4\n|}".into())),
            "Template:Mixed_name" => Ok(Some("normalized".into())),
            "Template:Unused" => Ok(Some("good".into())),
            "Template:Fail" => Err(LookupError::new("offline")),
            _ => Ok(None),
        }
    }
}
fn run(
    input: &str,
    data: &Data,
    limits: ExpansionLimits,
) -> Result<rwikitext::ConversionResult, rwikitext::ConversionError> {
    let context = WikiContext::default();
    convert_with_options(
        ConversionRequest {
            source: Format::Wikitext,
            target: Format::Html,
            input,
            context: &context,
        },
        ConversionOptions {
            wiki_data: Some(data),
            expansion_limits: limits,
            ..Default::default()
        },
    )
}
fn body(html: &str) -> String {
    scraper::Html::parse_fragment(html)
        .root_element()
        .text()
        .collect()
}

#[test]
fn adjacent_template_expansions_can_start_separate_list_items() {
    let result = run(
        "{{1x|#foo}}{{1x|#foo}}",
        &Data::default(),
        ExpansionLimits::default(),
    )
    .unwrap();
    let document = scraper::Html::parse_fragment(&result.content);
    let items = scraper::Selector::parse("ol > li").unwrap();
    assert_eq!(
        document
            .select(&items)
            .map(|item| item.text().collect::<String>())
            .collect::<Vec<_>>(),
        ["foo", "foo"]
    );
}

#[test]
fn a_template_can_start_a_table_after_inline_text() {
    let result = run(
        "foo {{table}}",
        &Data::default(),
        ExpansionLimits::default(),
    )
    .unwrap();
    assert!(result.diagnostics.is_empty(), "{:?}", result.diagnostics);
    let document = scraper::Html::parse_fragment(&result.content);
    let paragraph = scraper::Selector::parse("p").unwrap();
    assert_eq!(
        document
            .select(&paragraph)
            .next()
            .unwrap()
            .text()
            .collect::<String>()
            .trim(),
        "foo"
    );
    let table = scraper::Selector::parse("table td").unwrap();
    assert_eq!(
        document
            .select(&table)
            .map(|cell| cell.text().collect::<String>())
            .collect::<Vec<_>>(),
        ["1", "2", "3", "4"]
    );
    let transclusion =
        scraper::Selector::parse("table[typeof='mw:Transclusion'][data-mw]").unwrap();
    assert!(
        document.select(&transclusion).next().is_some(),
        "{}",
        result.content
    );
}

#[test]
fn a_parser_function_in_a_link_target_does_not_start_a_block() {
    let input = "[[Foo{{lc:#Section}}|target]]";
    let result = run(input, &Data::default(), ExpansionLimits::default()).unwrap();
    let document = scraper::Html::parse_fragment(&result.content);
    let links = scraper::Selector::parse("a[href='./Foo?action=edit&redlink=1#section']").unwrap();
    assert_eq!(
        document
            .select(&links)
            .map(|link| link.text().collect::<String>())
            .collect::<Vec<_>>(),
        ["target"]
    );
    let [loss] = result.diagnostics.as_slice() else {
        panic!(
            "expected a loss for the partial link, got {:?}",
            result.diagnostics
        )
    };
    assert_eq!(loss.kind, DiagnosticKind::ConversionLoss);
    assert_eq!(&input[loss.span.clone()], "{{lc:#Section}}");
}

#[test]
fn a_parser_function_in_a_bracketed_external_link_keeps_the_label() {
    let input = "[https://example.org {{lc:#Label}}]";
    let result = run(input, &Data::default(), ExpansionLimits::default()).unwrap();
    let document = scraper::Html::parse_fragment(&result.content);
    let links = scraper::Selector::parse("a[href='https://example.org']").unwrap();
    assert_eq!(
        document
            .select(&links)
            .map(|link| link.text().collect::<String>())
            .collect::<Vec<_>>(),
        ["#label"]
    );
}

#[test]
fn a_less_than_sign_in_plain_text_does_not_prevent_a_template_list() {
    let result = run(
        "2 < 3 {{1x|*Text}}",
        &Data::default(),
        ExpansionLimits::default(),
    )
    .unwrap();
    let document = scraper::Html::parse_fragment(&result.content);
    let item = scraper::Selector::parse("ul > li").unwrap();
    assert_eq!(
        document
            .select(&item)
            .next()
            .unwrap()
            .text()
            .collect::<String>(),
        "Text"
    );
}

#[test]
fn nested_arguments_defaults_and_inclusion() {
    let data = Data::default();
    let result = run(
        "{{Outer|first|name= value }} {{Outer}} {{Inclusion}}",
        &data,
        ExpansionLimits::default(),
    )
    .unwrap();
    let text = body(&result.content);
    assert!(text.contains("[first/value] first"), "{text}");
    assert!(text.contains("[default/fallback]"), "{text}");
    assert!(text.contains("insideincluded"), "{text}");
    assert!(!text.contains("outside"), "{text}");
    assert_eq!(
        data.calls.borrow().as_slice(),
        [
            "Template:Outer",
            "Template:Inner",
            "Template:Outer",
            "Template:Inner",
            "Template:Inclusion"
        ]
    );
}
#[test]
fn unused_template_argument_is_lazy_and_literals_do_not_expand() {
    let data = Data::default();
    let result = run(
        "{{Unused|{{Fail}}}} <nowiki>{{Fail}}</nowiki> <!-- {{Fail}} -->",
        &data,
        ExpansionLimits::default(),
    )
    .unwrap();
    assert!(body(&result.content).contains("good"));
    assert_eq!(data.calls.borrow().as_slice(), ["Template:Unused"]);
}
#[test]
fn lookup_errors_carry_the_invocation_span() {
    let data = Data::default();
    let err = run("before {{Fail}} after", &data, ExpansionLimits::default()).unwrap_err();
    assert!(err
        .diagnostics()
        .iter()
        .any(|d| d.kind == DiagnosticKind::WikiLookup && d.span == (7..15)));
}
#[test]
fn independent_expansion_limits_fail() {
    let data = Data::default();
    let base = ExpansionLimits::default();
    for (input, limits, invocation, reason) in [
        (
            "{{Loop}}",
            ExpansionLimits {
                max_depth: 2,
                ..base
            },
            "{{Loop}}",
            "depth",
        ),
        (
            "{{One}}{{One}}{{One}}",
            ExpansionLimits {
                max_expansions: 2,
                ..base
            },
            "{{One}}",
            "count",
        ),
        (
            "{{Big}}",
            ExpansionLimits {
                max_output_bytes: 4,
                ..base
            },
            "{{Big}}",
            "output byte",
        ),
    ] {
        let err = run(input, &data, limits).unwrap_err();
        let [diagnostic] = err.diagnostics() else {
            panic!("expected one expansion limit diagnostic, got {err:?}")
        };
        assert_eq!(diagnostic.kind, DiagnosticKind::ExpansionLimit);
        assert_eq!(
            diagnostic.span,
            input.rfind(invocation).unwrap()..input.len()
        );
        assert!(diagnostic.message.contains(reason), "{diagnostic}");
    }

    let result = run(
        "{{Big}}",
        &data,
        ExpansionLimits {
            max_depth: 1,
            max_expansions: 1,
            max_output_bytes: 5,
        },
    )
    .unwrap();
    assert!(result.diagnostics.is_empty());
    assert_eq!(body(&result.content), "Ω界");
}

#[test]
fn malformed_template_fallback_stops_before_the_next_argument_lookup() {
    let data = Data::default();
    let input = "{{[]|{{Big}}|{{Big}}|{{Big}}|{{Big}}|{{Big}}|{{Fail}}}}";
    let error = run(
        input,
        &data,
        ExpansionLimits {
            max_output_bytes: 40,
            ..ExpansionLimits::default()
        },
    )
    .unwrap_err();
    let [diagnostic] = error.diagnostics() else {
        panic!("expected one expansion limit diagnostic, got {error:?}")
    };
    assert_eq!(diagnostic.kind, DiagnosticKind::ExpansionLimit);
    assert_eq!(&input[diagnostic.span.clone()], input);
    assert!(!data
        .calls
        .borrow()
        .iter()
        .any(|title| title == "Template:Fail"));
}

#[test]
fn pre_tag_attributes_stop_expanding_when_the_combined_output_is_too_large() {
    let data = Data::default();
    let input =
        "{{#tag:pre|text|title={{Big}}|title={{Big}}|title={{Big}}|title={{Big}}|title={{Fail}}}}";
    let error = run(
        input,
        &data,
        ExpansionLimits {
            max_output_bytes: 48,
            ..ExpansionLimits::default()
        },
    )
    .unwrap_err();
    let [diagnostic] = error.diagnostics() else {
        panic!("expected one expansion limit diagnostic, got {error:?}")
    };
    assert_eq!(diagnostic.kind, DiagnosticKind::ExpansionLimit);
    assert_eq!(&input[diagnostic.span.clone()], input);
    assert!(!data
        .calls
        .borrow()
        .iter()
        .any(|title| title == "Template:Fail"));
}

#[test]
fn a_missing_template_in_nowiki_uses_deterministic_literal_text() {
    let input = "{{#tag:nowiki|{{Absent}}}}";
    let result = run(input, &Data::default(), ExpansionLimits::default()).unwrap();
    assert_eq!(body(&result.content), "[[:Template:Absent]]");
    assert!(result.diagnostics.iter().all(|loss| {
        loss.kind == DiagnosticKind::ConversionLoss && &input[loss.span.clone()] == input
    }));
}

#[test]
fn a_template_title_that_normalizes_to_empty_stays_literal() {
    let data = Data::default();
    let input = "{{:_}} / {{:_#Section}}";
    let result = run(input, &data, ExpansionLimits::default()).unwrap();
    assert!(result.diagnostics.is_empty(), "{:?}", result.diagnostics);
    assert_eq!(body(&result.content), input);
    assert!(data.calls.borrow().is_empty());
}

#[test]
fn count_bounds_repeated_parameter_expansion_with_empty_output() {
    struct Repeated;
    impl WikiData for Repeated {
        fn template_text(&self, title: &str) -> Result<Option<String>, LookupError> {
            let step: usize = title.strip_prefix("Template:T").unwrap().parse().unwrap();
            Ok(Some(if step == 20 {
                "{{{1}}}".into()
            } else {
                format!("{{{{T{}|{}{}{}}}}}", step + 1, "{{{1}}}", "{{{1}}}", "")
            }))
        }
    }
    let error = convert_with_options(
        ConversionRequest {
            source: Format::Wikitext,
            target: Format::Html,
            input: "{{T0|}}",
            context: &WikiContext::default(),
        },
        ConversionOptions {
            wiki_data: Some(&Repeated),
            expansion_limits: ExpansionLimits {
                max_expansions: 100,
                ..Default::default()
            },
            ..Default::default()
        },
    )
    .unwrap_err();
    assert!(error
        .diagnostics()
        .iter()
        .any(|d| d.kind == DiagnosticKind::ExpansionLimit && d.message.contains("count")));
}

#[test]
fn inactive_inclusion_cannot_add_page_metadata() {
    let result = run("<includeonly>[[Category:Hidden]]</includeonly><noinclude>[[Category:Visible]]</noinclude>body", &Data::default(), ExpansionLimits::default()).unwrap();
    assert_eq!(result.metadata.categories, ["Visible"]);
}
#[test]
fn missing_templates_render_as_red_links() {
    let data = Data::default();
    let result = run("{{Missing}}", &data, ExpansionLimits::default()).unwrap();
    let doc = scraper::Html::parse_fragment(&result.content);
    let a = doc
        .select(&scraper::Selector::parse("a").unwrap())
        .next()
        .expect("missing template link");
    assert!(a
        .value()
        .attr("class")
        .unwrap_or("")
        .split_whitespace()
        .any(|c| c == "new"));
    assert!(a.text().collect::<String>().contains("Template:Missing"));
}

#[test]
fn lookup_title_is_canonicalized() {
    let data = Data::default();
    let result = run("{{mixed name}}", &data, ExpansionLimits::default()).unwrap();
    assert!(body(&result.content).contains("normalized"));
    assert_eq!(data.calls.borrow().as_slice(), ["Template:Mixed_name"]);
}
#[test]
fn cli_uses_the_same_template_callback_data() {
    use std::io::Write;
    use std::process::{Command, Stdio};
    let mut child = Command::new(env!("CARGO_BIN_EXE_rwikitext"))
        .args([
            "wikitext",
            "html",
            "-",
            "--wiki-data",
            "tests/fixtures/wiki-data.json",
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
        .write_all(b"{{Outer|hello}}")
        .unwrap();
    let output = child.wait_with_output().unwrap();
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(body(&String::from_utf8(output.stdout).unwrap()).contains("hello"));
}

#[test]
#[ignore = "requires scripts/fetch-compatibility.py"]
fn pinned_mediawiki_template_cases() {
    let fixture =
        std::fs::read_to_string("target/upstream/mediawiki-1.46.2/tests/parser/parserTests.txt")
            .unwrap();
    struct FixtureData<'a>(&'a str);
    impl WikiData for FixtureData<'_> {
        fn template_text(&self, title: &str) -> Result<Option<String>, LookupError> {
            Ok(self.0.split("!! article\n").skip(1).find_map(|article| {
                let (name, body) = article.split_once("\n!! text\n")?;
                if name.replace(' ', "_").eq_ignore_ascii_case(title) {
                    Some(body.split("\n!! endarticle").next().unwrap().to_owned())
                } else {
                    None
                }
            }))
        }
    }
    let data = FixtureData(&fixture);
    for name in [
        "<includeonly> and <noinclude> being included",
        "<onlyinclude> being included",
        "<onlyinclude> and <includeonly> being included",
        "Simple template",
        "Template parameter",
        "Template unnamed parameter",
        "Template with default value",
        "Template with default value (value set)",
        "Template with complex template as argument",
        "Template redirect",
    ] {
        let header = format!("!! test\n{name}\n!! wikitext\n");
        let case = fixture
            .split(&header)
            .nth(1)
            .unwrap()
            .split("!! end")
            .next()
            .unwrap();
        let input = case.split("\n!! html").next().unwrap();
        let expected = case
            .split("\n!! html/php\n")
            .nth(1)
            .or_else(|| case.split("\n!! html\n").nth(1))
            .unwrap()
            .split("\n!! ")
            .next()
            .unwrap();
        let context = WikiContext::default();
        let result = convert_with_options(
            ConversionRequest {
                source: Format::Wikitext,
                target: Format::Html,
                input,
                context: &context,
            },
            ConversionOptions {
                wiki_data: Some(&data),
                ..Default::default()
            },
        )
        .unwrap();
        assert_eq!(
            body(&result.content).trim(),
            body(expected).trim(),
            "{name}"
        );
    }
}

#[test]
fn pathological_parameter_nesting_fails_without_exhausting_the_stack() {
    let data = Data::default();
    let mut input = "{{{missing|".repeat(200);
    input.push('x');
    input.push_str(&"}}}".repeat(200));
    let error = run(
        &input,
        &data,
        ExpansionLimits {
            max_depth: 1000,
            max_expansions: 1000,
            ..Default::default()
        },
    )
    .unwrap_err();
    assert!(error
        .diagnostics()
        .iter()
        .any(|d| d.kind == DiagnosticKind::ExpansionLimit));
}

#[test]
fn page_inclusion_controls_and_prevented_literals_do_not_lookup() {
    let data = Data::default();
    let output = run("a<noinclude>b</noinclude><includeonly>{{Fail}}</includeonly><onlyinclude>c</onlyinclude><pre class=\"x\">{{Fail}}</pre>", &data, ExpansionLimits::default()).unwrap();
    assert!(body(&output.content).contains("abc"), "{}", output.content);
    assert!(data.calls.borrow().is_empty());
}

#[test]
fn nested_missing_link_survives_case_function() {
    let result = run(
        "{{uc:{{Missing}}}}",
        &Data::default(),
        ExpansionLimits::default(),
    )
    .unwrap();
    let doc = scraper::Html::parse_fragment(&result.content);
    let link = doc
        .select(&scraper::Selector::parse("a.new").unwrap())
        .next()
        .expect("missing template link");
    assert!(link.text().collect::<String>().contains("Template:Missing"));
}

#[test]
fn self_closed_inclusion_controls_are_empty() {
    let data = Data::default();
    let output = run(
        "a<noinclude />b<includeonly/>c<onlyinclude />d",
        &data,
        ExpansionLimits::default(),
    )
    .unwrap();
    assert_eq!(body(&output.content), "abcd");
    assert!(output.diagnostics.is_empty(), "{:?}", output.diagnostics);
}
