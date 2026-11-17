use rwikitext::{convert, ConversionRequest, Format, WikiContext};

#[test]
fn category_page_metadata_survives_html_round_trip() {
    let context = WikiContext::default();
    let first = convert(ConversionRequest {
        source: Format::Wikitext,
        target: Format::Html,
        input: "Text\n[[Category:Foo|Bar]]",
        context: &context,
    })
    .unwrap();
    assert_eq!(first.metadata.categories, vec!["Foo"]);
    assert!(first.content.contains("mw:PageProp/Category"));
    let second = convert(ConversionRequest {
        source: Format::Html,
        target: Format::Wikitext,
        input: &first.content,
        context: &context,
    })
    .unwrap();
    assert_eq!(second.metadata, first.metadata);
    assert!(second.content.contains("[[Category:Foo|Bar]]"));
}

#[test]
fn page_metadata_loss_is_located_and_strict_mode_rejects_it() {
    use rwikitext::{convert_with_mode, ConversionMode, DiagnosticKind};
    let context = WikiContext::default();
    let input = "Body\n[[Category:Foo|Bar]]";
    let request = || ConversionRequest {
        source: Format::Wikitext,
        target: Format::Gfm,
        input,
        context: &context,
    };
    let result = convert(request()).unwrap();
    assert_eq!(result.content, "Body");
    assert_eq!(result.metadata.categories, vec!["Foo"]);
    let diagnostic = result
        .diagnostics
        .iter()
        .find(|d| d.kind == DiagnosticKind::ConversionLoss)
        .unwrap();
    assert_eq!(&input[diagnostic.span.clone()], "[[Category:Foo|Bar]]");
    assert!(!convert_with_mode(request(), ConversionMode::Strict)
        .unwrap_err()
        .diagnostics()
        .is_empty());
}

#[test]
fn redirect_and_display_title_round_trip_through_parsoid_html() {
    let context = WikiContext {
        page_title: "Example",
        ..WikiContext::default()
    };
    let input = "#REDIRECT [[Main Page]]\n{{DISPLAYTITLE:Example}}";
    let first = convert(ConversionRequest {
        source: Format::Wikitext,
        target: Format::Html,
        input,
        context: &context,
    })
    .unwrap();
    assert_eq!(first.metadata.redirect_target.as_deref(), Some("Main Page"));
    assert_eq!(first.metadata.display_title.as_deref(), Some("Example"));
    let second = convert(ConversionRequest {
        source: Format::Html,
        target: Format::Wikitext,
        input: &first.content,
        context: &context,
    })
    .unwrap();
    assert_eq!(second.metadata, first.metadata);
    assert!(second.content.starts_with("#REDIRECT [[Main Page]]"));
}

#[test]
fn ordinary_and_unsafe_links_are_not_trusted_as_page_properties() {
    let context = WikiContext::default();
    for input in [
        "<link rel=\"mw:PageProp/Category\" href=\"https://example.org/Category:Foo\">",
        "<link rel=\"mw:PageProp/Category\" href=\"./Category:Foo\" typeof=\"mw:ExpandedAttrs\">",
    ] {
        if let Ok(result) = convert(ConversionRequest {
            source: Format::Html,
            target: Format::Wikitext,
            input,
            context: &context,
        }) {
            assert!(result.metadata.categories.is_empty());
            assert!(!result.diagnostics.is_empty());
        }
    }
}

// MediaWiki 1.46.2 parserTests.txt: "Simple category with whitespace as sort key".
#[test]
fn official_simple_category_with_whitespace_sort_key() {
    let context = WikiContext::default();
    let wiki = convert(ConversionRequest {
        source: Format::Wikitext,
        target: Format::Html,
        input: "[[Category:Foo| ]]",
        context: &context,
    })
    .unwrap();
    assert!(wiki.content.contains("href=\"./Category:Foo#%20\""));
    let html = "<link rel=\"mw:PageProp/Category\" href=\"./Category:Foo#%20\" data-parsoid='{\"stx\":\"simple\"}'>";
    let parsed = convert(ConversionRequest {
        source: Format::Html,
        target: Format::Wikitext,
        input: html,
        context: &context,
    })
    .unwrap();
    assert_eq!(parsed.metadata, wiki.metadata);
    assert!(parsed.content.contains("[[Category:Foo| ]]"));
}

#[test]
fn cli_reports_metadata_loss_and_strict_mode_rejects_it() {
    use std::io::Write;
    use std::process::{Command, Stdio};
    let run = |strict: bool| {
        let mut cmd = Command::new(env!("CARGO_BIN_EXE_rwikitext"));
        cmd.args(["wikitext", "gfm"]);
        if strict {
            cmd.arg("--strict");
        }
        let mut child = cmd
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .unwrap();
        child
            .stdin
            .take()
            .unwrap()
            .write_all(b"Body\n[[Category:Foo]]")
            .unwrap();
        child.wait_with_output().unwrap()
    };
    let best = run(false);
    assert!(best.status.success());
    assert_eq!(String::from_utf8(best.stdout).unwrap(), "Body");
    let stderr = String::from_utf8(best.stderr).unwrap();
    assert!(
        stderr.contains("category") && stderr.contains("5..21"),
        "{stderr}"
    );
    let strict = run(true);
    assert!(!strict.status.success());
    assert!(strict.stdout.is_empty());
    assert!(String::from_utf8(strict.stderr)
        .unwrap()
        .contains("category"));
}

// MediaWiki 1.46.2 redirects.txt: "1. Simple redirect to page".
#[test]
fn official_simple_redirect() {
    let context = WikiContext::default();
    let result = convert(ConversionRequest {
        source: Format::Wikitext,
        target: Format::Html,
        input: "#REDIRECT [[Main Page]]",
        context: &context,
    })
    .unwrap();
    assert_eq!(
        result.metadata.redirect_target.as_deref(),
        Some("Main Page")
    );
    assert!(result.content.contains("href=\"./Main_Page\""));
    let imported = convert(ConversionRequest {
        source: Format::Html,
        target: Format::Wikitext,
        input: "<link rel=\"mw:PageProp/redirect\" href=\"./Main_Page\"/>",
        context: &context,
    })
    .unwrap();
    assert_eq!(imported.metadata, result.metadata);
}

// MediaWiki 1.46.2 parserTests.txt emits this transclusion for DISPLAYTITLE.
#[test]
fn official_plain_display_title_transclusion() {
    let context = WikiContext {
        page_title: "Whatever",
        ..WikiContext::default()
    };
    let html = "<span about=\"#mwt1\" typeof=\"mw:Transclusion\" data-mw='{\"parts\":[{\"template\":{\"target\":{\"wt\":\"DISPLAYTITLE:whatever\",\"function\":\"displaytitle\"},\"params\":{},\"i\":0}}]}'></span>";
    let imported = convert(ConversionRequest {
        source: Format::Html,
        target: Format::Wikitext,
        input: html,
        context: &context,
    })
    .unwrap();
    assert_eq!(imported.metadata.display_title.as_deref(), Some("whatever"));
    assert!(imported.content.contains("{{DISPLAYTITLE:whatever}}"));
}

#[test]
fn metadata_inside_inert_html_is_never_trusted() {
    let context = WikiContext::default();
    let property = "<link rel=\"mw:PageProp/Category\" href=\"./Category:Fake\">";
    for wrapper in [
        "script",
        "style",
        "textarea",
        "title",
        "xmp",
        "iframe",
        "noembed",
        "noframes",
        "plaintext",
        "template",
        "svg",
        "math",
        "select",
        "noscript",
    ] {
        let input = format!("<{wrapper}>{property}</{wrapper}>");
        if let Ok(result) = convert(ConversionRequest {
            source: Format::Html,
            target: Format::Wikitext,
            input: &input,
            context: &context,
        }) {
            assert!(
                result.metadata.categories.is_empty(),
                "trusted metadata inside {wrapper}"
            );
        }
    }
    let nested =
        format!("<template><!-- </template> --><template></template>{property}</template>");
    if let Ok(result) = convert(ConversionRequest {
        source: Format::Html,
        target: Format::Wikitext,
        input: &nested,
        context: &context,
    }) {
        assert!(
            result.metadata.categories.is_empty(),
            "trusted metadata inside nested template"
        );
    }
}

#[test]
fn wikitext_html_attribute_is_not_page_metadata() {
    let context = WikiContext::default();
    let input = "<span title=\"[[Category:Fake]]\">text</span>";
    let result = convert(ConversionRequest {
        source: Format::Wikitext,
        target: Format::Html,
        input,
        context: &context,
    })
    .unwrap();
    assert!(result.metadata.categories.is_empty());
    assert!(result.content.contains("[[Category:Fake]]"));
}

// MediaWiki 1.46.2 redirects.txt: "Optional colon in #REDIRECT".
#[test]
fn official_redirect_optional_colon() {
    let context = WikiContext::default();
    let result = convert(ConversionRequest {
        source: Format::Wikitext,
        target: Format::Html,
        input: "#REDIRECT:[[Main Page]]",
        context: &context,
    })
    .unwrap();
    assert_eq!(
        result.metadata.redirect_target.as_deref(),
        Some("Main Page")
    );
    assert!(result.content.contains("href=\"./Main_Page\""));
}

// MediaWiki 1.46.2 parserTests.txt: T28547, accepted inline color.
#[test]
fn official_styled_display_title_round_trips() {
    let context = WikiContext {
        page_title: "Screen",
        ..WikiContext::default()
    };
    let input = "{{DISPLAYTITLE:<span style=\"color: red;\">s</span>creen}}";
    let first = convert(ConversionRequest {
        source: Format::Wikitext,
        target: Format::Html,
        input,
        context: &context,
    })
    .unwrap();
    assert_eq!(
        first.metadata.display_title.as_deref(),
        Some("<span style=\"color: red;\">s</span>creen")
    );
    let second = convert(ConversionRequest {
        source: Format::Html,
        target: Format::Wikitext,
        input: &first.content,
        context: &context,
    })
    .unwrap();
    assert_eq!(second.metadata, first.metadata);
    assert!(second.content.contains(input));
}

#[test]
fn display_title_markup_must_not_hide_content_or_inject_wikitext() {
    let context = WikiContext {
        page_title: "Screen",
        ..WikiContext::default()
    };
    for title in [
        "<span style=\"display: none;\">s</span>creen",
        "<span style=\"color: transparent\">s</span>creen",
        "<span style=\"color:red\" style=\"display:none\">s</span>creen",
        "<span style=\"color:red\">s</span>creen}}{{Danger}}",
    ] {
        let json = serde_json::json!({"parts":[{"template":{"target":{"wt":format!("DISPLAYTITLE:{title}"),"function":"displaytitle"},"params":{},"i":0}}]}).to_string();
        let html = format!(
            "<span typeof=\"mw:Transclusion\" data-mw=\"{}\"></span>",
            html_escape::encode_double_quoted_attribute(&json)
        );
        if let Ok(result) = convert(ConversionRequest {
            source: Format::Html,
            target: Format::Wikitext,
            input: &html,
            context: &context,
        }) {
            assert!(result.metadata.display_title.is_none(), "trusted {title}");
        }
    }
}
