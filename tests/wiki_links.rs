use rwikitext::{
    convert, convert_with_options, ConversionOptions, ConversionRequest, DiagnosticKind, FileInfo,
    Format, LookupError, WikiContext, WikiData,
};
use std::cell::RefCell;

#[derive(Default)]
struct Data {
    calls: RefCell<Vec<String>>,
}
impl WikiData for Data {
    fn page_exists(&self, title: &str) -> Result<Option<bool>, LookupError> {
        self.calls.borrow_mut().push(title.into());
        match title {
            "Missing_page" => Ok(Some(false)),
            "This_is_a_red_link" => Ok(None),
            "Broken" => Err(LookupError::new("offline")),
            _ => Ok(Some(true)),
        }
    }
    fn file_info(&self, title: &str) -> Result<Option<FileInfo>, LookupError> {
        self.calls.borrow_mut().push(title.into());
        match title {
            "File:Example_image.png" => Ok(Some(FileInfo {
                url: "https://example.org/image.png".into(),
                width: Some(300),
                height: Some(200),
            })),
            "File:Foobar.jpg" => Ok(Some(FileInfo {
                url: "http://example.com/images/3/3a/Foobar.jpg".into(),
                width: Some(1941),
                height: Some(220),
            })),
            "File:Unsafe.png" => Ok(Some(FileInfo {
                url: "javascript:alert(1)".into(),
                width: None,
                height: None,
            })),
            "File:Broken.png" => Err(LookupError::new("offline")),
            _ => Ok(None),
        }
    }
}

#[test]
#[ignore = "requires the verified MediaWiki archive"]
fn pinned_red_link_preserves_label_and_destination() {
    let fixtures = std::fs::read_to_string("target/upstream/mediawiki-1.46.2/vendor/wikimedia/parsoid/tests/parser/langParserTests.txt").unwrap();
    let case = fixtures
        .split_once("Don't crash if there are no possible variants of a redlink\n")
        .unwrap()
        .1;
    let (wiki, expected) = case
        .split_once("!! wikitext\n")
        .unwrap()
        .1
        .split_once("\n!! html/parsoid+langconv\n")
        .unwrap();
    let expected = expected.split_once("\n!! end").unwrap().0;
    let actual = run(wiki, Format::Html, &Data::default()).unwrap();
    let link = |html: &str| {
        let document = scraper::Html::parse_fragment(html);
        let anchor = document
            .select(&scraper::Selector::parse("a.new").unwrap())
            .next()
            .unwrap();
        (
            anchor.attr("href").map(str::to_owned),
            anchor.attr("rel").map(str::to_owned),
            anchor.text().collect::<String>(),
        )
    };
    assert_eq!(link(&actual.content), link(expected));
}
fn run(
    input: &str,
    target: Format,
    data: &Data,
) -> Result<rwikitext::ConversionResult, rwikitext::ConversionError> {
    let context = WikiContext::default();
    convert_with_options(
        ConversionRequest {
            source: Format::Wikitext,
            target,
            input,
            context: &context,
        },
        ConversionOptions {
            wiki_data: Some(data),
            ..Default::default()
        },
    )
}
#[test]
fn links_lookup_in_lists_and_tables_preserves_labels_and_errors() {
    let data = Data::default();
    let result = run(
        "* [[Existing]]\n* [[Missing page|Label]]\n{|\n| [[Missing page#Part|Cell]]\n|}",
        Format::Html,
        &data,
    )
    .unwrap();
    let html = scraper::Html::parse_fragment(&result.content);
    let red = scraper::Selector::parse("a.new").unwrap();
    let links: Vec<_> = html.select(&red).collect();
    assert_eq!(links.len(), 2, "{}", result.content);
    assert_eq!(
        links[0].attr("href"),
        Some("./Missing_page?action=edit&redlink=1")
    );
    assert_eq!(links[0].text().collect::<String>(), "Label");
    assert!(
        data.calls
            .borrow()
            .iter()
            .filter(|s| *s == "Missing_page")
            .count()
            == 2
    );
    let error = run("[[Broken]]", Format::Html, &data).unwrap_err();
    assert!(error
        .diagnostics()
        .iter()
        .any(|d| d.kind == DiagnosticKind::WikiLookup));
}

#[test]
fn attributes_are_literal_and_colon_escaped_pages_are_looked_up() {
    let data = Data::default();
    let literal = r#"<span title="[[Broken]]">content</span>"#;
    assert!(run(literal, Format::Html, &data)
        .unwrap()
        .content
        .contains(literal));
    assert!(data.calls.borrow().is_empty());
    assert!(run("[[:Broken|Label]]", Format::Html, &data).is_err());
}

#[test]
fn importing_media_preserves_adjacent_content_and_diagnoses_url_mismatch() {
    let malformed = "<span typeof='mw:File'><img resource='./File:Example_image.png' src='https://example.org/image.png'><strong>unrelated text</strong></span>";
    let mismatch = "<figure typeof='mw:File/Thumb'><img src='https://unrelated.example/image.png' resource='./File:Example_image.png' alt='A' width='100'><figcaption>Sample</figcaption></figure>";
    let import = |input| {
        convert_with_options(
            ConversionRequest {
                source: Format::Html,
                target: Format::Wikitext,
                input,
                context: &WikiContext::default(),
            },
            ConversionOptions {
                wiki_data: Some(&Data::default()),
                ..Default::default()
            },
        )
        .unwrap()
    };
    let result = import(malformed);
    assert!(result.content.contains("unrelated text"));
    assert!(!result.content.contains("[[File:"));
    assert!(!result.diagnostics.is_empty());
    let result = import(mismatch);
    assert!(result.content.contains("[[File:Example image.png"));
    assert!(result
        .diagnostics
        .iter()
        .any(|d| d.kind == DiagnosticKind::ConversionLoss && d.message.contains("media URL")));
    assert!(
        import(&mismatch.replace("https://unrelated.example", "https://example.org"))
            .diagnostics
            .is_empty()
    );
}

#[test]
fn height_only_images_scale_and_missing_image_options_report_loss() {
    let data = Data::default();
    let result = run(
        "[[File:Example image.png|thumb|x80px|caption]]",
        Format::Html,
        &data,
    )
    .unwrap();
    assert!(result.content.contains("width=\"120\" height=\"80\""));
    let result = run(
        "[[File:Missing.jpg|thumb|alt=sample|link=Page]]",
        Format::Html,
        &data,
    )
    .unwrap();
    assert!(result
        .diagnostics
        .iter()
        .any(|d| d.kind == DiagnosticKind::ConversionLoss));
}
#[test]
fn media_round_trip_preserves_target_alt_size_and_caption() {
    let data = Data::default();
    let wiki = "[[File:Example image.png|thumb|120px|alt=Descriptive text|A caption]]";
    let html = run(wiki, Format::Html, &data).unwrap();
    assert!(
        html.content.contains("https://example.org/image.png"),
        "{}",
        html.content
    );
    assert!(html.content.contains("A caption"), "{}", html.content);
    let context = WikiContext::default();
    let back = convert(ConversionRequest {
        source: Format::Html,
        target: Format::Wikitext,
        input: &html.content,
        context: &context,
    })
    .unwrap();
    for value in [
        "File:Example image.png",
        "alt=Descriptive text",
        "120px",
        "A caption",
    ] {
        assert!(back.content.contains(value), "{}", back.content);
    }
    assert!(data
        .calls
        .borrow()
        .contains(&"File:Example_image.png".to_owned()));
}
#[test]
fn missing_file_uses_broken_media_and_errors_are_propagated() {
    let data = Data::default();
    let html = run("[[File:Missing.jpg|thumb|Caption]]", Format::Html, &data).unwrap();
    assert!(
        html.content.contains("mw:Error mw:File/Thumb"),
        "{}",
        html.content
    );
    assert!(html.content.contains("mw-broken-media"), "{}", html.content);
    assert!(run("[[File:Broken.png]]", Format::Html, &data).is_err());
}

// The case and expected DOM features are from parserTests.txt in MediaWiki 1.46.2.
#[test]
fn official_file_alignment_size_and_caption() {
    let data = Data::default();
    let html = run(
        "[[File:Foobar.jpg|right|200x200px|Caption]]",
        Format::Html,
        &data,
    )
    .unwrap();
    let doc = scraper::Html::parse_fragment(&html.content);
    let figure = doc
        .select(&scraper::Selector::parse("figure.mw-halign-right").unwrap())
        .next()
        .expect(&html.content);
    assert_eq!(figure.attr("typeof"), Some("mw:File"));
    let image = figure
        .select(&scraper::Selector::parse("img").unwrap())
        .next()
        .unwrap();
    assert_eq!(image.attr("resource"), Some("./File:Foobar.jpg"));
    assert_eq!(image.attr("width"), Some("200"));
    assert_eq!(image.attr("height"), Some("23"));
    assert_eq!(
        figure
            .select(&scraper::Selector::parse("figcaption").unwrap())
            .next()
            .unwrap()
            .text()
            .collect::<String>(),
        "Caption"
    );
    let context = WikiContext::default();
    let back = convert(ConversionRequest {
        source: Format::Html,
        target: Format::Wikitext,
        input: &html.content,
        context: &context,
    })
    .unwrap();
    assert!(back.content.contains("|right"), "{}", back.content);
    assert!(back.content.contains("Caption"), "{}", back.content);
}

#[test]
fn caption_with_nested_link_and_formatting_round_trips() {
    let data = Data::default();
    let html = run(
        "[[File:Example image.png|thumb|See [[Existing|this page]] and ''detail'']]",
        Format::Html,
        &data,
    )
    .unwrap();
    let doc = scraper::Html::parse_fragment(&html.content);
    let caption = doc
        .select(&scraper::Selector::parse("figcaption").unwrap())
        .next()
        .expect(&html.content);
    let anchor = caption
        .select(&scraper::Selector::parse("a").unwrap())
        .next()
        .unwrap();
    assert_eq!(anchor.attr("href"), Some("./Existing"));
    assert!(caption
        .select(&scraper::Selector::parse("i").unwrap())
        .next()
        .is_some());
    let context = WikiContext::default();
    let back = convert(ConversionRequest {
        source: Format::Html,
        target: Format::Wikitext,
        input: &html.content,
        context: &context,
    })
    .unwrap();
    assert!(
        back.content.contains("[[Existing|this page]]"),
        "{}",
        back.content
    );
    assert!(back.content.contains("<i>detail</i>"), "{}", back.content);
}

#[test]
fn nested_caption_lookups_and_unsafe_urls_are_handled() {
    let data = Data::default();
    let wiki = "[[File:Example image.png|thumb|See [[Missing page|missing]]]]";
    let html = run(wiki, Format::Html, &data).unwrap();
    assert!(
        html.content.contains("?action=edit&amp;redlink=1"),
        "{}",
        html.content
    );
    assert!(data.calls.borrow().contains(&"Missing_page".to_owned()));
    let wiki = "[[File:Example image.png|thumb|See [[Broken]]]]";
    let error = run(wiki, Format::Html, &data).unwrap_err();
    let start = wiki.find("[[Broken]]").unwrap();
    assert!(
        error
            .diagnostics()
            .iter()
            .any(|d| d.kind == DiagnosticKind::WikiLookup && d.span == (start..start + 10)),
        "{:?}",
        error.diagnostics()
    );
    let unsafe_result = run("[[File:Unsafe.png]]", Format::Html, &data).unwrap();
    assert!(!unsafe_result.content.contains("javascript:"));
    assert!(unsafe_result
        .diagnostics
        .iter()
        .any(|d| d.kind == DiagnosticKind::ConversionLoss));
}

#[test]
fn cli_local_file_and_page_results() {
    use std::io::Write;
    use std::process::{Command, Stdio};
    let mut child = Command::new(env!("CARGO_BIN_EXE_rwikitext"))
        .args([
            "wikitext",
            "html",
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
        .write_all(b"[[Missing|label]] [[File:Example.png|alt=example]]")
        .unwrap();
    let output = child.wait_with_output().unwrap();
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    let html = String::from_utf8(output.stdout).unwrap();
    assert!(html.contains("?action=edit&amp;redlink=1"), "{html}");
    assert!(html.contains("https://example.org/example.png"), "{html}");
}

#[test]
fn direct_media_link_round_trips_with_resource() {
    let data = Data::default();
    let html = run("[[Media:Example image.png|image]]", Format::Html, &data).unwrap();
    assert!(
        html.content.contains("https://example.org/image.png"),
        "{}",
        html.content
    );
    assert!(
        html.content
            .contains("resource=\"./Media:Example_image.png\""),
        "{}",
        html.content
    );
    let context = WikiContext::default();
    let back = convert(ConversionRequest {
        source: Format::Html,
        target: Format::Wikitext,
        input: &html.content,
        context: &context,
    })
    .unwrap();
    assert!(
        back.content.contains("[[Media:Example image.png|image]]"),
        "{}",
        back.content
    );
}

#[test]
fn missing_link_round_trips_with_fragment_and_label() {
    let data = Data::default();
    let html = run("[[Missing page#Section|label]]", Format::Html, &data).unwrap();
    let context = WikiContext::default();
    let back = convert(ConversionRequest {
        source: Format::Html,
        target: Format::Wikitext,
        input: &html.content,
        context: &context,
    })
    .unwrap();
    assert!(
        back.content.contains("[[Missing page#Section|label]]"),
        "{}",
        back.content
    );
    let gfm = run("[[Missing page#Section|label]]", Format::Gfm, &data).unwrap();
    assert!(
        gfm.content.contains("Missing_page#Section"),
        "{}",
        gfm.content
    );
}

#[test]
fn wiki_callbacks_run_on_calling_thread() {
    struct ThreadData(std::thread::ThreadId);
    impl WikiData for ThreadData {
        fn page_exists(&self, _: &str) -> Result<Option<bool>, LookupError> {
            assert_eq!(std::thread::current().id(), self.0);
            Ok(Some(true))
        }
        fn file_info(&self, _: &str) -> Result<Option<FileInfo>, LookupError> {
            assert_eq!(std::thread::current().id(), self.0);
            Ok(None)
        }
    }
    let data = ThreadData(std::thread::current().id());
    let context = WikiContext::default();
    convert_with_options(
        ConversionRequest {
            source: Format::Wikitext,
            target: Format::Html,
            input: "[[Main Page]] [[File:Missing.jpg]]",
            context: &context,
        },
        ConversionOptions {
            wiki_data: Some(&data),
            ..Default::default()
        },
    )
    .unwrap();
}

#[test]
fn media_survives_gfm_round_trip() {
    let data = Data::default();
    let gfm = run(
        "[[File:Example image.png|thumb|alt=example|Caption]]",
        Format::Gfm,
        &data,
    )
    .unwrap();
    let context = WikiContext::default();
    let back = convert(ConversionRequest {
        source: Format::Gfm,
        target: Format::Wikitext,
        input: &gfm.content,
        context: &context,
    })
    .unwrap();
    assert!(
        back.content.contains("File:Example image.png"),
        "{} -> {}",
        gfm.content,
        back.content
    );
    assert!(
        back.content.contains("Caption"),
        "{} -> {}",
        gfm.content,
        back.content
    );
}

// Inline caption fixture from parserTests.txt in the pinned MediaWiki release.
#[test]
fn official_inline_file_caption_supplies_alt() {
    let data = Data::default();
    let html = run("[[File:Foobar.jpg|« test »]]", Format::Html, &data).unwrap();
    let doc = scraper::Html::parse_fragment(&html.content);
    let image = doc
        .select(&scraper::Selector::parse("img").unwrap())
        .next()
        .expect(&html.content);
    assert_eq!(image.attr("alt"), Some("« test »"));
    assert_eq!(image.attr("resource"), Some("./File:Foobar.jpg"));
    assert!(html
        .diagnostics
        .iter()
        .any(|d| d.kind == DiagnosticKind::ConversionLoss));
}
