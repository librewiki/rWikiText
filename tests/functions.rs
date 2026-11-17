use rwikitext::{
    convert_with_options, ConversionOptions, ConversionRequest, DiagnosticKind, Format,
    LookupError, WikiContext, WikiData,
};
struct Data;
impl WikiData for Data {
    fn page_exists(&self, title: &str) -> Result<Option<bool>, LookupError> {
        match title {
            "Present" | "ParserFunctions_page" => Ok(Some(true)),
            "Failure" => Err(LookupError::new("offline")),
            _ => Ok(None),
        }
    }

    fn file_info(&self, title: &str) -> Result<Option<rwikitext::FileInfo>, LookupError> {
        if title == "File:Failure.jpg" {
            return Err(LookupError::new("file offline"));
        }
        Ok((title == "File:Foobar.jpg").then(|| rwikitext::FileInfo {
            url: "https://example.org/Foobar.jpg".into(),
            width: None,
            height: None,
        }))
    }
}
fn run(input: &str) -> Result<String, rwikitext::ConversionError> {
    let context = WikiContext {
        page_title: "User:Example/Sub",
        site_language: "fr",
        reference_time: "2024-02-03T04:05:06Z",
    };
    convert_with_options(
        ConversionRequest {
            source: Format::Wikitext,
            target: Format::Html,
            input,
            context: &context,
        },
        ConversionOptions {
            wiki_data: Some(&Data),
            ..Default::default()
        },
    )
    .map(|r| {
        scraper::Html::parse_fragment(&r.content)
            .root_element()
            .text()
            .collect::<String>()
    })
}
#[test]
fn context_and_core_functions() {
    let output = run("{{PAGENAME}}; {{FULLPAGENAME}}; {{NAMESPACE}}; {{SUBPAGENAME}}; {{CONTENTLANGUAGE}}; {{CURRENTTIMESTAMP}}; {{uc:hello}}; {{padleft:7|3|0}}").unwrap();
    assert!(
        output.contains("Example/Sub; User:Example/Sub; User; Sub; fr; 20240203040506; HELLO; 007"),
        "{output}"
    );
}

#[test]
fn title_magic_word_requires_colon_for_explicit_title() {
    struct Templates;
    impl WikiData for Templates {
        fn template_text(&self, title: &str) -> Result<Option<String>, LookupError> {
            Ok((title == "Template:PAGENAME").then(|| "template".into()))
        }
    }
    let result = convert_with_options(
        ConversionRequest {
            source: Format::Wikitext,
            target: Format::Html,
            input: "{{PAGENAME|foo}} / {{PAGENAME:foo}} / {{PAGENAME}}",
            context: &WikiContext {
                page_title: "Current",
                ..WikiContext::default()
            },
        },
        ConversionOptions {
            wiki_data: Some(&Templates),
            ..Default::default()
        },
    )
    .unwrap();
    let text = scraper::Html::parse_fragment(&result.content)
        .root_element()
        .text()
        .collect::<String>();
    assert_eq!(text, "template / Foo / Current");
}

#[test]
fn inline_equals_keeps_metadata_in_preformatted_text_and_table_cells() {
    for input in [
        " {|\nnormal {{=}} value",
        "<pre>\n{|\n</pre>\n{{=}} stays literal here",
        "{|\n| data {{=}} value\n|}",
    ] {
        let result = convert_with_options(
            ConversionRequest {
                source: Format::Wikitext,
                target: Format::Html,
                input,
                context: &WikiContext::default(),
            },
            ConversionOptions::default(),
        )
        .unwrap();
        assert!(
            result.diagnostics.is_empty(),
            "{input}: {:?}",
            result.diagnostics
        );
        assert!(
            result.content.contains("mw:Transclusion"),
            "{}",
            result.content
        );
    }
}
#[test]
fn branches_are_lazy_and_nested() {
    let output = run("{{#if:{{#ifeq:01|1|yes|no}}|{{#ifexist:Present|ok|wrong}}|{{#ifexist:Failure|bad|bad}}}} {{#switch:b|a|b=hit|#default=miss}}").unwrap();
    assert!(output.contains("ok hit"), "{output}");
    let error = run("{{#ifexist:Failure|yes|no}}").unwrap_err();
    assert!(error
        .diagnostics()
        .iter()
        .any(|d| d.kind == DiagnosticKind::WikiLookup));
    assert!(run("{{#ifexist:Absent|yes|no}}").unwrap().contains("no"));
    assert!(run("{{#switch:foo|#default|bar=first|baz=second}}")
        .unwrap()
        .contains("first"));
}

#[test]
fn conditional_file_lookup_error_points_to_the_invocation() {
    let call = "{{#ifexist:Media:Failure.jpg|found|missing}}";
    let input = format!("앞 {call}");
    let error = run(&input).unwrap_err();
    let [diagnostic] = error.diagnostics() else {
        panic!("expected one wiki lookup error, got {error:?}")
    };
    assert_eq!(diagnostic.kind, DiagnosticKind::WikiLookup);
    assert_eq!(&input[diagnostic.span.clone()], call);
    assert!(diagnostic.message.contains("File:Failure.jpg"));
    assert!(diagnostic.message.contains("file offline"));
}

#[test]
fn utc_weekday_and_iso_week_follow_the_calendar_year_boundary() {
    for (reference_time, expected) in [
        ("2021-01-01T00:00:00Z", "5 53"),
        ("2021-01-04T00:00:00Z", "1 1"),
        ("2023-01-01T00:00:00Z", "0 52"),
        ("2000-02-29T00:00:00Z", "2 9"),
    ] {
        let result = convert_with_options(
            ConversionRequest {
                source: Format::Wikitext,
                target: Format::Gfm,
                input: "{{CURRENTDOW}} {{CURRENTWEEK}}",
                context: &WikiContext {
                    reference_time,
                    ..WikiContext::default()
                },
            },
            ConversionOptions::default(),
        )
        .unwrap();
        assert!(result.diagnostics.is_empty(), "{:?}", result.diagnostics);
        assert_eq!(result.content, expected, "{reference_time}");
    }
}

#[test]
#[ignore = "requires scripts/fetch-compatibility.py"]
fn pinned_magic_word_cases() {
    let source =
        std::fs::read_to_string("target/upstream/mediawiki-1.46.2/tests/parser/magicWords.txt")
            .unwrap();
    for name in [
        "CURRENTDAY",
        "CURRENTDAY2",
        "CURRENTMONTH",
        "CURRENTMONTH1",
        "CURRENTTIME",
        "CURRENTHOUR",
        "CURRENTYEAR",
        "CURRENTTIMESTAMP",
        "FULLPAGENAME",
        "PAGENAME",
        "SUBPAGENAME",
        "BASEPAGENAME",
        "ROOTPAGENAME",
    ] {
        let title = format!("!! test\nMagic Word: {{{{{name}}}}}\n");
        let case = source
            .split(&title)
            .nth(1)
            .expect(name)
            .split("!! end")
            .next()
            .unwrap();
        let page_title = case
            .split("title=[[")
            .nth(1)
            .map(|s| s.split("]]").next().unwrap())
            .unwrap_or("Parser test");
        let input = case
            .split("!! wikitext\n")
            .nth(1)
            .unwrap()
            .split("!! html/php\n")
            .next()
            .unwrap()
            .trim();
        let expected = case
            .split("!! html/php\n<p>")
            .nth(1)
            .unwrap()
            .split('\n')
            .next()
            .unwrap();
        let context = WikiContext {
            page_title,
            site_language: "en",
            reference_time: "1970-01-01T00:02:03Z",
        };
        let result = convert_with_options(
            ConversionRequest {
                source: Format::Wikitext,
                target: Format::Html,
                input,
                context: &context,
            },
            ConversionOptions {
                wiki_data: Some(&Data),
                ..Default::default()
            },
        )
        .unwrap();
        let document = scraper::Html::parse_fragment(&result.content);
        assert_eq!(
            document.root_element().text().collect::<String>().trim(),
            expected,
            "{name}"
        );
    }
}

#[test]
#[ignore = "requires scripts/fetch-compatibility.py"]
fn pinned_ifeq_case() {
    let source = std::fs::read_to_string("target/upstream/mediawiki-1.46.2/extensions/ParserFunctions/tests/parser/funcsParserTests.txt").unwrap();
    let case = source
        .split("!! test\n#ifeq\n")
        .nth(1)
        .unwrap()
        .split("!! end")
        .next()
        .unwrap();
    let input = case
        .split("!! wikitext\n")
        .nth(1)
        .unwrap()
        .split("!! html\n")
        .next()
        .unwrap()
        .trim();
    let expected_html = case
        .split("!! html\n")
        .nth(1)
        .unwrap()
        .split("\n!!end")
        .next()
        .unwrap();
    let expected_doc = scraper::Html::parse_fragment(expected_html);
    let expected = expected_doc.root_element().text().collect::<String>();
    let actual = run(input).unwrap();
    assert_eq!(
        actual.split_whitespace().collect::<Vec<_>>(),
        expected.split_whitespace().collect::<Vec<_>>()
    );
}

#[test]
fn context_words_do_not_require_a_provider() {
    let context = WikiContext {
        page_title: "Example",
        site_language: "en",
        reference_time: "2024-02-03T04:05:06Z",
    };
    let result = rwikitext::convert(ConversionRequest {
        source: Format::Wikitext,
        target: Format::Html,
        input: "{{PAGENAME}} {{CURRENTYEAR}}",
        context: &context,
    })
    .unwrap();
    let doc = scraper::Html::parse_fragment(&result.content);
    assert!(doc
        .root_element()
        .text()
        .collect::<String>()
        .contains("Example 2024"));
}

#[test]
fn page_title_is_text_not_wikitext() {
    let context = WikiContext {
        page_title: "User:''Example'' & Co",
        site_language: "en",
        reference_time: "2024-02-03T04:05:06Z",
    };
    let result = rwikitext::convert(ConversionRequest {
        source: Format::Wikitext,
        target: Format::Html,
        input: "{{FULLPAGENAME}}",
        context: &context,
    })
    .unwrap();
    let doc = scraper::Html::parse_fragment(&result.content);
    assert_eq!(
        doc.root_element().text().collect::<String>(),
        "User:''Example'' & Co"
    );
    assert!(!result.content.contains("<i>"));
}

#[test]
#[ignore = "requires scripts/fetch-compatibility.py"]
fn pinned_conditional_cases() {
    let source = std::fs::read_to_string("target/upstream/mediawiki-1.46.2/extensions/ParserFunctions/tests/parser/funcsParserTests.txt").unwrap();
    for name in [
        "#if",
        "#switch",
        "{{#ifexist}}",
        "#switch #default should match next and not last named parameter",
    ] {
        let title = format!("!! test\n{name}\n");
        let case = source
            .split(&title)
            .nth(1)
            .unwrap()
            .split("!! end")
            .next()
            .unwrap();
        let input = case
            .split("!! wikitext\n")
            .nth(1)
            .unwrap()
            .split("!! html\n")
            .next()
            .unwrap()
            .trim();
        let expected_html = case
            .split("!! html\n")
            .nth(1)
            .unwrap()
            .split("\n!!end")
            .next()
            .unwrap();
        let expected_doc = scraper::Html::parse_fragment(expected_html);
        let expected = expected_doc.root_element().text().collect::<String>();
        let actual = run(input).unwrap();
        assert_eq!(
            actual.split_whitespace().collect::<Vec<_>>(),
            expected.split_whitespace().collect::<Vec<_>>(),
            "{name}"
        );
    }
}

#[test]
#[ignore = "requires scripts/fetch-compatibility.py"]
fn pinned_string_functions_and_metadata() {
    let source =
        std::fs::read_to_string("target/upstream/mediawiki-1.46.2/tests/parser/parserTests.txt")
            .unwrap();
    for (case, language) in [
        ("Padleft and padright (default 0-padding)", "en"),
        ("Padleft and padright (partial fill)", "en"),
        ("Padleft and padright as substr", "en"),
        ("Strip marker in padleft", "en"),
        ("Strip marker in padright", "en"),
        ("T33490 Turkish: ucfirst 'ix'", "tr"),
    ] {
        let body = source
            .split_once(&format!("\n{case}\n"))
            .unwrap()
            .1
            .split_once("!! end")
            .unwrap()
            .0;
        let test = body.split_once("!! wikitext\n").unwrap().1;
        let (input, _) = test.split_once("\n!! html").unwrap();
        let expected = test
            .split_once("!! html/parsoid\n")
            .or_else(|| test.split_once("!! html\n"))
            .unwrap()
            .1;
        let context = WikiContext {
            site_language: language,
            ..WikiContext::default()
        };
        let result = rwikitext::convert(ConversionRequest {
            source: Format::Wikitext,
            target: Format::Html,
            input,
            context: &context,
        })
        .unwrap();
        let actual = scraper::Html::parse_fragment(&result.content);
        let expected = scraper::Html::parse_fragment(expected);
        assert_eq!(
            actual.root_element().text().collect::<String>().trim(),
            expected.root_element().text().collect::<String>().trim(),
            "{case}"
        );
        let data_mw = |document: &scraper::Html| {
            document
                .select(&scraper::Selector::parse("[data-mw]").unwrap())
                .map(|el| {
                    serde_json::from_str::<serde_json::Value>(el.attr("data-mw").unwrap()).unwrap()
                })
                .collect::<Vec<_>>()
        };
        if !data_mw(&expected).is_empty() {
            assert_eq!(data_mw(&actual), data_mw(&expected), "{case}");
        }
    }
}

#[test]
fn local_cli_repeats_context_dependent_functions() {
    use std::{
        io::Write,
        process::{Command, Stdio},
    };
    let execute = |input: &str| {
        let mut child = Command::new(env!("CARGO_BIN_EXE_rwikitext"))
            .args([
                "wikitext",
                "html",
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
        child.wait_with_output().unwrap()
    };
    let input = "{{PAGENAME}} {{CONTENTLANGUAGE}} {{CURRENTTIMESTAMP}} {{#ifexist:Main Page|{{uc:yes}}|no}}";
    let first = execute(input);
    assert!(first.status.success());
    assert_eq!(first.stdout, execute(input).stdout);
    let content = String::from_utf8(first.stdout).unwrap();
    assert_eq!(
        scraper::Html::parse_fragment(&content)
            .root_element()
            .text()
            .collect::<String>(),
        "Example/Subpage en 20240309050607 YES"
    );
    let error = execute("{{#ifexist:Lookup error|yes|no}}");
    assert!(!error.status.success());
    assert!(error.stdout.is_empty());
    assert!(String::from_utf8_lossy(&error.stderr).contains("page unavailable"));
}
