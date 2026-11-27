#[path = "support/namumark.rs"]
mod namumark_support;

use namumark_support::{assert_clean_namumark, convert_clean_namumark};
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
    let context = WikiContext::default();
    let cases = [
        ("'''''x''' y''", Format::Html, "<p><em><strong>x</strong> y</em></p>"),
        ("'''''x''' y''", Format::Wikitext, "'''''x''' y''"),
        ("'''''x''' y''", Format::Gfm, "***x** y*"),
        ("'''''x'' y'''", Format::Html, "<p><strong><em>x</em> y</strong></p>"),
        ("'''''x'' y'''", Format::Wikitext, "'''''x'' y'''"),
        ("'''''x'' y'''", Format::Gfm, "***x* y**"),
    ];
    for (input, target, expected) in cases {
        assert_clean_namumark(&context, target, input, expected);
    }
}

#[test]
fn namumark_issue_examples_convert_without_diagnostics() {
    let context = WikiContext::default();
    for (input, expected) in [
        (
            "'''''bold-italic''' italic''",
            "<p><em><strong>bold-italic</strong> italic</em></p>",
        ),
        (
            "'''''bold-italic'' bold'''",
            "<p><strong><em>bold-italic</em> bold</strong></p>",
        ),
    ] {
        assert_clean_namumark(&context, Format::Html, input, expected);
    }
}

#[test]
fn namumark_nests_emphasis_inside_single_markers() {
    let context = WikiContext::default();
    let cases = [
        (
            "'''x ''y'' z'''",
            Format::Html,
            "<p><strong>x <em>y</em> z</strong></p>",
        ),
        ("'''x ''y'' z'''", Format::Wikitext, "'''x ''y'' z'''"),
        ("'''x ''y'' z'''", Format::Gfm, "**x *y* z**"),
        (
            "''x '''y''' z''",
            Format::Html,
            "<p><em>x <strong>y</strong> z</em></p>",
        ),
        ("''x '''y''' z''", Format::Wikitext, "''x '''y''' z''"),
        ("''x '''y''' z''", Format::Gfm, "*x **y** z*"),
    ];
    for (input, target, expected) in cases {
        assert_clean_namumark(&context, target, input, expected);
    }
}

#[test]
fn namumark_symmetric_combined_emphasis_is_italic_outside_bold() {
    let context = WikiContext::default();
    let cases = [
        ("'''''x'''''", Format::Html, "<p><em><strong>x</strong></em></p>"),
        ("'''''x'''''", Format::Wikitext, "'''''x'''''"),
        ("'''''x'''''", Format::Gfm, "***x***"),
    ];
    for (input, target, expected) in cases {
        assert_clean_namumark(&context, target, input, expected);
    }
}

#[test]
fn namumark_symmetric_combined_emphasis_keeps_nested_short_markers() {
    let context = WikiContext::default();
    let cases = [
        (
            "'''''bold-italic with '''strong''' inside'''''",
            "<p><em><strong>bold-italic with <strong>strong</strong> inside</strong></em></p>",
            "'''''bold&#45;italic with '''strong''' inside'''''",
            "***bold\\-italic with **strong** inside***",
        ),
        (
            "'''''bold-italic with ''italic'' inside'''''",
            "<p><em><strong>bold-italic with <em>italic</em> inside</strong></em></p>",
            "'''''bold&#45;italic with ''italic'' inside'''''",
            "***bold\\-italic with *italic* inside***",
        ),
        (
            "'''''a [[Orchard|''x'']] b'''''",
            "<p><em><strong>a <a rel=\"mw:WikiLink\" href=\"./Orchard\"><em>x</em></a> b</strong></em></p>",
            "'''''a [[Orchard|''x'']] b'''''",
            "***a [*x*](<./Orchard>) b***",
        ),
        (
            "'''''a''b''c'''''",
            "<p><em><strong>a<em>b</em>c</strong></em></p>",
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
            assert_clean_namumark(&context, target, input, expected);
        }
    }
}

#[test]
fn namumark_combined_emphasis_falls_back_without_symmetric_closer() {
    let context = WikiContext::default();
    let cases = [
        (
            "'''''x''' y'' and ''z''",
            "<p><em><strong>x</strong> y</em> and <em>z</em></p>",
            "'''''x''' y'' and ''z''",
            "***x** y* and *z*",
        ),
        (
            "'''''x'' y''' and '''z'''",
            "<p><strong><em>x</em> y</strong> and <strong>z</strong></p>",
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
            assert_clean_namumark(&context, target, input, expected);
        }
    }
}

#[test]
fn namumark_same_line_asymmetric_and_symmetric_combined_emphasis() {
    let context = WikiContext::default();
    let cases = [
        (
            "'''''a''' b'' and '''''c'''''",
            "<p><em><strong>a</strong> b</em> and <em><strong>c</strong></em></p>",
            "'''''a''' b'' and '''''c'''''",
            "***a** b* and ***c***",
        ),
        (
            "'''''a'' b''' and '''''c'''''",
            "<p><strong><em>a</em> b</strong> and <em><strong>c</strong></em></p>",
            "'''''a'' b''' and '''''c'''''",
            "***a* b** and ***c***",
        ),
        (
            "'''''a''''' and '''''b''' c''",
            "<p><em><strong>a</strong></em> and <em><strong>b</strong> c</em></p>",
            "'''''a''''' and '''''b''' c''",
            "***a*** and ***b** c*",
        ),
        (
            "'''''a''' b'' and '''''c'' d'''",
            "<p><em><strong>a</strong> b</em> and <strong><em>c</em> d</strong></p>",
            "'''''a''' b'' and '''''c'' d'''",
            "***a** b* and ***c* d**",
        ),
    ];
    for (input, html, wikitext, gfm) in cases {
        for (target, expected) in [
            (Format::Html, html),
            (Format::Wikitext, wikitext),
            (Format::Gfm, gfm),
        ] {
            assert_clean_namumark(&context, target, input, expected);
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
    assert_eq!(result.content, "<p>가<strong>배</strong>다<br>'''''a''' b</p>");
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
    let context = WikiContext::default();
    assert_clean_namumark(
        &context,
        Format::Html,
        r"''x \'' y''",
        "<p><em>x '' y</em></p>",
    );
}

#[test]
fn namumark_ordinary_brackets_remain_literal_text() {
    let context = WikiContext::default();
    for input in [
        "[1]",
        "[sic]",
        "[note]",
        "[TODO(fix)]",
        "[anchor]",
        "[include]",
        "[include(foo]",
        "[youtube(id]",
        "[br(1]",
    ] {
        for target in [Format::Html, Format::Wikitext, Format::Gfm] {
            convert_clean_namumark(&context, target, input);
        }
    }
    assert_clean_namumark(&context, Format::Wikitext, "[1]", "&#91;1&#93;");
    assert_clean_namumark(&context, Format::Gfm, "[1]", "\\[1\\]");
    assert_clean_namumark(&context, Format::Html, "[1]", "<p>[1]</p>");
}

#[test]
fn namumark_bracketed_prose_keeps_inner_emphasis_active() {
    let context = WikiContext::default();
    let cases = [
        ("[note ''x'']", Format::Html, "<p>[note <em>x</em>]</p>"),
        ("[note ''x'']", Format::Wikitext, "&#91;note ''x''&#93;"),
        ("[note ''x'']", Format::Gfm, "\\[note *x*\\]"),
    ];
    for (input, target, expected) in cases {
        assert_clean_namumark(&context, target, input, expected);
    }
    let result = namumark_convert(Format::Html, r"\[[Orchard]]");
    assert_eq!(result.content, "<p>[[Orchard]]</p>");
    assert!(result.diagnostics.is_empty(), "{:?}", result.diagnostics);
}

#[test]
fn namumark_italic_closer_takes_leading_two_of_five_apostrophes() {
    let context = WikiContext::default();
    let cases = [
        (
            "''a'''''b'''",
            "<p><em>a</em><strong>b</strong></p>",
            "''a'''''b'''",
            "*a***b**",
        ),
        (
            "''a'''''b'''''c''",
            "<p><em>a</em><strong>b</strong><em>c</em></p>",
            "''a'''''b'''''c''",
            "*a***b***c*",
        ),
        (
            "See ''italic'''''bold''' here.",
            "<p>See <em>italic</em><strong>bold</strong> here.</p>",
            "See ''italic'''''bold''' here.",
            "See *italic***bold** here\\.",
        ),
        (
            "[[Orchard|''a'''''b''']]",
            "<p><a rel=\"mw:WikiLink\" href=\"./Orchard\"><em>a</em><strong>b</strong></a></p>",
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
            assert_clean_namumark(&context, target, input, expected);
        }
    }
}

#[test]
fn namumark_shared_five_run_closes_italic_and_bold_pair() {
    let context = WikiContext::default();
    // A five-or-more closer run donates its leading apostrophes to the inner
    // opener, so one shared run closes both the inner and outer style.
    let cases = [
        (
            "plain''italic'''bold-italic'''''plain",
            "<p>plain<em>italic<strong>bold-italic</strong></em>plain</p>",
            "plain''italic'''bold&#45;italic'''''plain",
            "plain*italic**bold\\-italic***plain",
        ),
        (
            "plain'''bold''bold-italic'''''plain",
            "<p>plain<strong>bold<em>bold-italic</em></strong>plain</p>",
            "plain'''bold''bold&#45;italic'''''plain",
            "plain**bold*bold\\-italic***plain",
        ),
        (
            "''a'''b'''''",
            "<p><em>a<strong>b</strong></em></p>",
            "''a'''b'''''",
            "*a**b***",
        ),
        (
            "'''a''b'''''",
            "<p><strong>a<em>b</em></strong></p>",
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
            assert_clean_namumark(&context, target, input, expected);
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
        "<p><em>x <strong>y</strong> z</em></p>",
        "''x '''y''' z''",
        "*x **y** z*",
    )] {
        for (target, expected) in [
            (Format::Html, html),
            (Format::Wikitext, wikitext),
            (Format::Gfm, gfm),
        ] {
            assert_clean_namumark(&context, target, input, expected);
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
    assert_eq!(result.content, "<p><em>a</em>''''</p>", "input {input}");
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
    assert_eq!(result.content, "<p><em>x''''y</em></p>", "input {input}");
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

    // Four-apostrophe runs are bold plus one literal apostrophe on each side.
    let input = "''''x''''";
    for (target, expected) in [
        (Format::Html, "<p><strong>'x</strong>'</p>"),
        (Format::Wikitext, "'''&#39;x'''&#39;"),
        (Format::Gfm, "**\\'x**\\'"),
    ] {
        assert_clean_namumark(&context, target, input, expected);
    }
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
