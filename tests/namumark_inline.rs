use rwikitext::{convert, ConversionRequest, Format, WikiContext};

fn namumark_convert(target: Format, input: &str) -> rwikitext::ConversionResult {
    let context = WikiContext::default();
    convert(ConversionRequest {
        source: Format::NamuMark,
        target,
        input,
        context: &context,
    })
    .unwrap()
}

#[test]
fn namumark_combined_emphasis_closes_in_closer_order() {
    use rwikitext::{convert_with_mode, ConversionMode};
    let context = WikiContext::default();
    let cases = [
        ("'''''x''' y''", Format::Html, "<p><i><b>x</b> y</i></p>"),
        ("'''''x''' y''", Format::Wikitext, "'''''x''' y''"),
        ("'''''x''' y''", Format::Gfm, "***x** y*"),
        ("'''''x'' y'''", Format::Html, "<p><b><i>x</i> y</b></p>"),
        ("'''''x'' y'''", Format::Wikitext, "'''''x'' y'''"),
        ("'''''x'' y'''", Format::Gfm, "***x* y**"),
    ];
    for (input, target, expected) in cases {
        let request = ConversionRequest {
            source: Format::NamuMark,
            target,
            input,
            context: &context,
        };
        let result = convert(request).unwrap();
        assert_eq!(result.content, expected, "target {target:?} input {input}");
        assert!(
            result.diagnostics.is_empty(),
            "target {target:?} input {input}: {:?}",
            result.diagnostics
        );
        assert!(
            convert_with_mode(request, ConversionMode::Strict).is_ok(),
            "target {target:?} input {input}"
        );
    }
}

#[test]
fn namumark_issue_examples_convert_without_diagnostics() {
    let context = WikiContext::default();
    for (input, expected) in [
        (
            "'''''bold-italic''' italic''",
            "<p><i><b>bold-italic</b> italic</i></p>",
        ),
        (
            "'''''bold-italic'' bold'''",
            "<p><b><i>bold-italic</i> bold</b></p>",
        ),
    ] {
        let result = convert(ConversionRequest {
            source: Format::NamuMark,
            target: Format::Html,
            input,
            context: &context,
        })
        .unwrap();
        assert_eq!(result.content, expected, "input {input}");
        assert!(
            result.diagnostics.is_empty(),
            "input {input}: {:?}",
            result.diagnostics
        );
    }
}

#[test]
fn namumark_nests_emphasis_inside_single_markers() {
    use rwikitext::{convert_with_mode, ConversionMode};
    let context = WikiContext::default();
    let cases = [
        (
            "'''x ''y'' z'''",
            Format::Html,
            "<p><b>x <i>y</i> z</b></p>",
        ),
        ("'''x ''y'' z'''", Format::Wikitext, "'''x ''y'' z'''"),
        ("'''x ''y'' z'''", Format::Gfm, "**x *y* z**"),
        (
            "''x '''y''' z''",
            Format::Html,
            "<p><i>x <b>y</b> z</i></p>",
        ),
        ("''x '''y''' z''", Format::Wikitext, "''x '''y''' z''"),
        ("''x '''y''' z''", Format::Gfm, "*x **y** z*"),
    ];
    for (input, target, expected) in cases {
        let request = ConversionRequest {
            source: Format::NamuMark,
            target,
            input,
            context: &context,
        };
        let result = convert(request).unwrap();
        assert_eq!(result.content, expected, "target {target:?} input {input}");
        assert!(
            result.diagnostics.is_empty(),
            "target {target:?} input {input}: {:?}",
            result.diagnostics
        );
        assert!(
            convert_with_mode(request, ConversionMode::Strict).is_ok(),
            "target {target:?} input {input}"
        );
    }
}

#[test]
fn namumark_symmetric_combined_emphasis_is_italic_outside_bold() {
    use rwikitext::{convert_with_mode, ConversionMode};
    let context = WikiContext::default();
    let cases = [
        ("'''''x'''''", Format::Html, "<p><i><b>x</b></i></p>"),
        ("'''''x'''''", Format::Wikitext, "'''''x'''''"),
        ("'''''x'''''", Format::Gfm, "***x***"),
    ];
    for (input, target, expected) in cases {
        let request = ConversionRequest {
            source: Format::NamuMark,
            target,
            input,
            context: &context,
        };
        let result = convert(request).unwrap();
        assert_eq!(result.content, expected, "target {target:?} input {input}");
        assert!(result.diagnostics.is_empty(), "target {target:?}");
        assert!(
            convert_with_mode(request, ConversionMode::Strict).is_ok(),
            "target {target:?}"
        );
    }
}

#[test]
fn namumark_symmetric_combined_emphasis_keeps_nested_short_markers() {
    use rwikitext::{convert_with_mode, ConversionMode};
    let context = WikiContext::default();
    let cases = [
        (
            "'''''bold-italic with '''strong''' inside'''''",
            "<p><i><b>bold-italic with <b>strong</b> inside</b></i></p>",
            "'''''bold&#45;italic with '''strong''' inside'''''",
            "***bold\\-italic with **strong** inside***",
        ),
        (
            "'''''bold-italic with ''italic'' inside'''''",
            "<p><i><b>bold-italic with <i>italic</i> inside</b></i></p>",
            "'''''bold&#45;italic with ''italic'' inside'''''",
            "***bold\\-italic with *italic* inside***",
        ),
        (
            "'''''a [[Orchard|''x'']] b'''''",
            "<p><i><b>a <a rel=\"mw:WikiLink\" href=\"./Orchard\"><i>x</i></a> b</b></i></p>",
            "'''''a [[Orchard|''x'']] b'''''",
            "***a [*x*](<./Orchard>) b***",
        ),
        (
            "'''''a''b''c'''''",
            "<p><i><b>a<i>b</i>c</b></i></p>",
            "'''''a''b''c'''''",
            "***a*b*c***",
        ),
    ];
    for (input, html, wikitext, gfm) in cases {
        for (target, expected) in [
            (Format::Html, html),
            (Format::Wikitext, wikitext),
            (Format::Gfm, gfm),
        ] {
            let request = ConversionRequest {
                source: Format::NamuMark,
                target,
                input,
                context: &context,
            };
            let result = convert(request).unwrap();
            assert_eq!(result.content, expected, "target {target:?} input {input}");
            assert!(
                result.diagnostics.is_empty(),
                "target {target:?} input {input}: {:?}",
                result.diagnostics
            );
            assert!(
                convert_with_mode(request, ConversionMode::Strict).is_ok(),
                "target {target:?} input {input}"
            );
        }
    }
}

#[test]
fn namumark_combined_emphasis_falls_back_without_symmetric_closer() {
    use rwikitext::{convert_with_mode, ConversionMode};
    let context = WikiContext::default();
    let cases = [
        (
            "'''''x''' y'' and ''z''",
            "<p><i><b>x</b> y</i> and <i>z</i></p>",
            "'''''x''' y'' and ''z''",
            "***x** y* and *z*",
        ),
        (
            "'''''x'' y''' and '''z'''",
            "<p><b><i>x</i> y</b> and <b>z</b></p>",
            "'''''x'' y''' and '''z'''",
            "***x* y** and **z**",
        ),
    ];
    for (input, html, wikitext, gfm) in cases {
        for (target, expected) in [
            (Format::Html, html),
            (Format::Wikitext, wikitext),
            (Format::Gfm, gfm),
        ] {
            let request = ConversionRequest {
                source: Format::NamuMark,
                target,
                input,
                context: &context,
            };
            let result = convert(request).unwrap();
            assert_eq!(result.content, expected, "target {target:?} input {input}");
            assert!(
                result.diagnostics.is_empty(),
                "target {target:?} input {input}: {:?}",
                result.diagnostics
            );
            assert!(
                convert_with_mode(request, ConversionMode::Strict).is_ok(),
                "target {target:?} input {input}"
            );
        }
    }
}

#[test]
fn namumark_unclosed_combined_emphasis_is_one_located_loss() {
    use rwikitext::{convert_with_mode, ConversionMode, DiagnosticKind};
    let context = WikiContext::default();
    let input = "'''''a''' b";
    let request = ConversionRequest {
        source: Format::NamuMark,
        target: Format::Html,
        input,
        context: &context,
    };
    let result = convert(request).unwrap();
    assert_eq!(result.diagnostics.len(), 1, "{:?}", result.diagnostics);
    assert_eq!(result.diagnostics[0].kind, DiagnosticKind::ConversionLoss);
    assert_eq!(result.diagnostics[0].span, 0..11);
    let html = scraper::Html::parse_fragment(&result.content);
    assert_eq!(html.root_element().text().collect::<String>(), input);
    assert!(convert_with_mode(request, ConversionMode::Strict).is_err());
}

#[test]
fn namumark_unclosed_combined_emphasis_keeps_utf8_and_crlf_offsets() {
    use rwikitext::{convert_with_mode, ConversionMode, DiagnosticKind};
    let context = WikiContext::default();
    let input = "가'''배'''다\r\n'''''a''' b";
    let request = ConversionRequest {
        source: Format::NamuMark,
        target: Format::Html,
        input,
        context: &context,
    };
    let result = convert(request).unwrap();
    assert_eq!(result.diagnostics.len(), 1, "{:?}", result.diagnostics);
    assert_eq!(result.diagnostics[0].kind, DiagnosticKind::ConversionLoss);
    assert_eq!(result.diagnostics[0].span, 17..28);
    assert_eq!(result.content, "<p>가<b>배</b>다<br>'''''a''' b</p>");
    assert!(convert_with_mode(request, ConversionMode::Strict).is_err());
}

#[test]
fn namumark_escaped_apostrophes_stay_literal() {
    let result = namumark_convert(Format::Html, r"\'\'x\'\'");
    assert_eq!(result.content, "<p>''x''</p>");
    assert!(result.diagnostics.is_empty(), "{:?}", result.diagnostics);
}

#[test]
fn namumark_escaped_apostrophe_inside_emphasis_body() {
    use rwikitext::{convert_with_mode, ConversionMode};
    let result = namumark_convert(Format::Html, r"''x \'' y''");
    assert_eq!(result.content, "<p><i>x '' y</i></p>");
    assert!(result.diagnostics.is_empty(), "{:?}", result.diagnostics);
    let context = WikiContext::default();
    let request = ConversionRequest {
        source: Format::NamuMark,
        target: Format::Html,
        input: r"''x \'' y''",
        context: &context,
    };
    assert!(convert_with_mode(request, ConversionMode::Strict).is_ok());
}

#[test]
fn namumark_ordinary_brackets_remain_literal_text() {
    use rwikitext::{convert_with_mode, ConversionMode};
    let context = WikiContext::default();
    for input in [
        "[1]",
        "[sic]",
        "[note]",
        "[TODO(fix)]",
        "[anchor]",
        "[include]",
    ] {
        for target in [Format::Html, Format::Wikitext, Format::Gfm] {
            let request = ConversionRequest {
                source: Format::NamuMark,
                target,
                input,
                context: &context,
            };
            let result = convert(request).unwrap();
            assert!(
                result.diagnostics.is_empty(),
                "target {target:?} input {input}: {:?}",
                result.diagnostics
            );
            assert!(
                convert_with_mode(request, ConversionMode::Strict).is_ok(),
                "target {target:?} input {input}"
            );
        }
    }
    let wikitext = namumark_convert(Format::Wikitext, "[1]");
    assert_eq!(wikitext.content, "&#91;1&#93;");
    assert!(wikitext.diagnostics.is_empty());
    let gfm = namumark_convert(Format::Gfm, "[1]");
    assert_eq!(gfm.content, "\\[1\\]");
    assert!(gfm.diagnostics.is_empty());
    let html = namumark_convert(Format::Html, "[1]");
    assert_eq!(html.content, "<p>[1]</p>");
    assert!(html.diagnostics.is_empty());
}

#[test]
fn namumark_bracketed_prose_keeps_inner_emphasis_active() {
    use rwikitext::{convert_with_mode, ConversionMode};
    let context = WikiContext::default();
    let cases = [
        ("[note ''x'']", Format::Html, "<p>[note <i>x</i>]</p>"),
        ("[note ''x'']", Format::Wikitext, "&#91;note ''x''&#93;"),
        ("[note ''x'']", Format::Gfm, "\\[note *x*\\]"),
    ];
    for (input, target, expected) in cases {
        let request = ConversionRequest {
            source: Format::NamuMark,
            target,
            input,
            context: &context,
        };
        let result = convert(request).unwrap();
        assert_eq!(result.content, expected, "target {target:?} input {input}");
        assert!(
            result.diagnostics.is_empty(),
            "target {target:?} input {input}: {:?}",
            result.diagnostics
        );
        assert!(
            convert_with_mode(request, ConversionMode::Strict).is_ok(),
            "target {target:?} input {input}"
        );
    }
    let result = namumark_convert(Format::Html, r"\[[Orchard]]");
    assert_eq!(result.content, "<p>[[Orchard]]</p>");
    assert!(result.diagnostics.is_empty(), "{:?}", result.diagnostics);
}

#[test]
fn namumark_italic_closer_takes_leading_two_of_five_apostrophes() {
    use rwikitext::{convert_with_mode, ConversionMode};
    let context = WikiContext::default();
    let cases = [
        (
            "''a'''''b'''",
            "<p><i>a</i><b>b</b></p>",
            "''a'''''b'''",
            "*a***b**",
        ),
        (
            "''a'''''b'''''c''",
            "<p><i>a</i><b>b</b><i>c</i></p>",
            "''a'''''b'''''c''",
            "*a***b***c*",
        ),
        (
            "See ''italic'''''bold''' here.",
            "<p>See <i>italic</i><b>bold</b> here.</p>",
            "See ''italic'''''bold''' here.",
            "See *italic***bold** here\\.",
        ),
        (
            "[[Orchard|''a'''''b''']]",
            "<p><a rel=\"mw:WikiLink\" href=\"./Orchard\"><i>a</i><b>b</b></a></p>",
            "[[Orchard|''a'''''b''']]",
            "[*a***b**](<./Orchard>)",
        ),
    ];
    for (input, html, wikitext, gfm) in cases {
        for (target, expected) in [
            (Format::Html, html),
            (Format::Wikitext, wikitext),
            (Format::Gfm, gfm),
        ] {
            let request = ConversionRequest {
                source: Format::NamuMark,
                target,
                input,
                context: &context,
            };
            let result = convert(request).unwrap();
            assert_eq!(result.content, expected, "target {target:?} input {input}");
            assert!(
                result.diagnostics.is_empty(),
                "target {target:?} input {input}: {:?}",
                result.diagnostics
            );
            assert!(
                convert_with_mode(request, ConversionMode::Strict).is_ok(),
                "target {target:?} input {input}"
            );
        }
    }
}

#[test]
fn namumark_shared_five_run_closes_italic_and_bold_pair() {
    use rwikitext::{convert_with_mode, ConversionMode};
    let context = WikiContext::default();
    // A five-or-more closer run donates its leading apostrophes to the inner
    // opener, so one shared run closes both the inner and outer style.
    let cases = [
        (
            "plain''italic'''bold-italic'''''plain",
            "<p>plain<i>italic<b>bold-italic</b></i>plain</p>",
            "plain''italic'''bold&#45;italic'''''plain",
            "plain*italic**bold\\-italic***plain",
        ),
        (
            "plain'''bold''bold-italic'''''plain",
            "<p>plain<b>bold<i>bold-italic</i></b>plain</p>",
            "plain'''bold''bold&#45;italic'''''plain",
            "plain**bold*bold\\-italic***plain",
        ),
        (
            "''a'''b'''''",
            "<p><i>a<b>b</b></i></p>",
            "''a'''b'''''",
            "*a**b***",
        ),
        (
            "'''a''b'''''",
            "<p><b>a<i>b</i></b></p>",
            "'''a''b'''''",
            "**a*b***",
        ),
    ];
    for (input, html, wikitext, gfm) in cases {
        for (target, expected) in [
            (Format::Html, html),
            (Format::Wikitext, wikitext),
            (Format::Gfm, gfm),
        ] {
            let request = ConversionRequest {
                source: Format::NamuMark,
                target,
                input,
                context: &context,
            };
            let result = convert(request).unwrap();
            assert_eq!(result.content, expected, "target {target:?} input {input}");
            assert!(
                result.diagnostics.is_empty(),
                "target {target:?} input {input}: {:?}",
                result.diagnostics
            );
            assert!(
                convert_with_mode(request, ConversionMode::Strict).is_ok(),
                "target {target:?} input {input}"
            );
        }
    }
}

#[test]
fn namumark_italic_closer_still_skips_three_and_four_apostrophes() {
    use rwikitext::{convert_with_mode, ConversionMode, DiagnosticKind};
    let context = WikiContext::default();

    // Three- and four-apostrophe runs never close italic.
    for (input, html, wikitext, gfm) in [(
        "''x '''y''' z''",
        "<p><i>x <b>y</b> z</i></p>",
        "''x '''y''' z''",
        "*x **y** z*",
    )] {
        for (target, expected) in [
            (Format::Html, html),
            (Format::Wikitext, wikitext),
            (Format::Gfm, gfm),
        ] {
            let request = ConversionRequest {
                source: Format::NamuMark,
                target,
                input,
                context: &context,
            };
            let result = convert(request).unwrap();
            assert_eq!(result.content, expected, "target {target:?} input {input}");
            assert!(
                result.diagnostics.is_empty(),
                "target {target:?} input {input}: {:?}",
                result.diagnostics
            );
            assert!(
                convert_with_mode(request, ConversionMode::Strict).is_ok(),
                "target {target:?} input {input}"
            );
        }
    }

    // A six-apostrophe run donates two to close italic; the remaining four
    // stay literal as one located loss.
    let input = "''a''''''";
    let request = ConversionRequest {
        source: Format::NamuMark,
        target: Format::Html,
        input,
        context: &context,
    };
    let result = convert(request).unwrap();
    assert_eq!(result.content, "<p><i>a</i>''''</p>", "input {input}");
    assert_eq!(result.diagnostics.len(), 1, "{:?}", result.diagnostics);
    assert_eq!(
        result.diagnostics[0].kind,
        DiagnosticKind::ConversionLoss,
        "input {input}"
    );
    assert_eq!(result.diagnostics[0].span, 5..9, "input {input}");
    assert!(convert_with_mode(request, ConversionMode::Strict).is_err());

    // A four-apostrophe run stays bold-plus-literal policy: the inner run is
    // one located loss inside the italic.
    let input = "''x''''y''";
    let request = ConversionRequest {
        source: Format::NamuMark,
        target: Format::Html,
        input,
        context: &context,
    };
    let result = convert(request).unwrap();
    assert_eq!(result.content, "<p><i>x''''y</i></p>", "input {input}");
    assert_eq!(result.diagnostics.len(), 1, "{:?}", result.diagnostics);
    assert_eq!(
        result.diagnostics[0].kind,
        DiagnosticKind::ConversionLoss,
        "input {input}"
    );
    assert!(
        result.diagnostics[0].span.start <= input.len()
            && result.diagnostics[0].span.end <= input.len()
            && result.diagnostics[0].span.start < result.diagnostics[0].span.end,
        "span {:?} input {input}",
        result.diagnostics[0].span
    );
    assert!(convert_with_mode(request, ConversionMode::Strict).is_err());
}

/// Count apostrophes in converted output, normalizing the escape forms the
/// Wikitext and Gfm writers use, so virtual closer bytes cannot hide there.
fn count_apostrophes(target: Format, text: &str) -> usize {
    let normalized = if target == Format::Wikitext {
        text.replace("&#39;", "'")
    } else if target == Format::Gfm {
        text.replace("\\'", "'")
    } else {
        text.to_owned()
    };
    normalized.matches('\'').count()
}

#[test]
fn namumark_shared_closer_candidates_never_leak_virtual_bytes() {
    use rwikitext::DiagnosticKind;
    let context = WikiContext::default();
    for input in ["''a{{{'''x'''''c", "''a[[x'''y'''''c", "''a'''b\\'''''c"] {
        let source_apostrophes = input.matches('\'').count();
        for target in [Format::Html, Format::Wikitext, Format::Gfm] {
            let request = ConversionRequest {
                source: Format::NamuMark,
                target,
                input,
                context: &context,
            };
            let result = convert(request).unwrap();
            assert!(
                result
                    .diagnostics
                    .iter()
                    .all(|d| d.span.end <= input.len() && d.span.start <= d.span.end),
                "target {target:?} input {input}: {:?}",
                result.diagnostics
            );
            assert!(
                result
                    .diagnostics
                    .iter()
                    .any(|d| d.kind == DiagnosticKind::ConversionLoss),
                "target {target:?} input {input} should still diagnose",
            );
            let extracted = if target == Format::Html {
                scraper::Html::parse_fragment(&result.content)
                    .root_element()
                    .text()
                    .collect::<String>()
            } else {
                result.content.clone()
            };
            let output_apostrophes = count_apostrophes(target, &extracted);
            assert!(
                output_apostrophes <= source_apostrophes,
                "target {target:?} input {input}: {output_apostrophes} output \
                 apostrophes exceed {source_apostrophes} source apostrophes in \
                 {extracted:?}",
            );
        }
    }
}
