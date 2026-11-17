use rwikitext::{convert, ConversionRequest, Format, WikiContext};

fn convert_text(source: Format, target: Format, input: &str) -> String {
    convert(ConversionRequest {
        source,
        target,
        input,
        context: &WikiContext::default(),
    })
    .unwrap()
    .content
}

#[test]
fn table_with_header_and_cells_uses_gfm_table() {
    let wiki = "{|\n! Name !! Value\n|-\n| Alpha || Beta\n|}";
    let gfm = convert_text(Format::Wikitext, Format::Gfm, wiki);
    assert_eq!(gfm, "| Name | Value |\n| --- | --- |\n| Alpha | Beta |");
    let result = convert_text(Format::Gfm, Format::Wikitext, &gfm);
    let html = convert_text(Format::Wikitext, Format::Html, &result);
    assert!(html.contains("<th>Name</th><th>Value</th>"), "{html}");
    assert!(html.contains("<td>Alpha</td><td>Beta</td>"), "{html}");
}

#[test]
fn merged_cells_use_permitted_html_and_survive_both_round_trips() {
    use Format::{Gfm, Html, Wikitext};
    let wiki = "{|\n! colspan=\"2\" | Heading\n|-\n| rowspan=\"2\" | A || B\n|-\n| C\n|}";
    for target in [Gfm, Html] {
        let intermediate = convert_text(Wikitext, target, wiki);
        assert!(intermediate.contains("<table>"), "{intermediate}");
        assert!(intermediate.contains("colspan=\"2\""), "{intermediate}");
        assert!(intermediate.contains("rowspan=\"2\""), "{intermediate}");
        let back = convert_text(target, Wikitext, &intermediate);
        let rendered = convert_text(Wikitext, Html, &back);
        assert!(
            rendered.contains("<th colspan=\"2\">Heading</th>"),
            "{rendered}"
        );
        assert!(rendered.contains("<td rowspan=\"2\">A</td>"), "{rendered}");
        assert!(rendered.contains("<td>B</td>"), "{rendered}");
        assert!(rendered.contains("<td>C</td>"), "{rendered}");
    }
}

#[test]
fn official_simple_table_and_cli_preserve_cell_order() {
    use std::io::Write;
    use std::process::{Command, Stdio};
    // MediaWiki 1.46.2 tests/parser/tables.txt, "Simple table".
    let wiki = "{|\n|1||2\n|-\n|3||4\n|}";
    let expected =
        "<table><tbody><tr><td>1</td><td>2</td></tr><tr><td>3</td><td>4</td></tr></tbody></table>";
    assert_eq!(convert_text(Format::Wikitext, Format::Html, wiki), expected);
    let mut process = Command::new(env!("CARGO_BIN_EXE_rwikitext"))
        .args(["wikitext", "html"])
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .spawn()
        .unwrap();
    process
        .stdin
        .take()
        .unwrap()
        .write_all(wiki.as_bytes())
        .unwrap();
    let output = process.wait_with_output().unwrap();
    assert!(output.status.success());
    assert_eq!(String::from_utf8(output.stdout).unwrap().trim(), expected);
}

#[test]
fn general_html_table_keeps_safe_attributes_through_wikitext() {
    use Format::{Html, Wikitext};
    let input = "<table class=\"wikitable\"><caption>Figures</caption><tr><th id=\"head\" scope=\"col\">Name</th><th scope=\"col\">Count</th></tr><tr><td headers=\"head\">A</td><td>2</td></tr></table>";
    let wiki = convert_text(Html, Wikitext, input);
    let html = convert_text(Wikitext, Html, &wiki);
    for expected in [
        "class=\"wikitable\"",
        "<caption>Figures</caption>",
        "scope=\"col\"",
        "id=\"head\"",
        "headers=\"head\"",
        ">A</td>",
    ] {
        assert!(html.contains(expected), "missing {expected} in {html}");
    }
}

#[test]
fn attributed_wiki_table_uses_html_fallback_and_keeps_content() {
    let wiki = "{| class=\"wikitable\"\n|+ style=\"color:red\" | Caption\n|- class=\"row\"\n! scope=\"col\" | Name !! scope=\"col\" | Value\n|-\n| A || B\n|}";
    let gfm = convert_text(Format::Wikitext, Format::Gfm, wiki);
    assert!(gfm.starts_with("<table class=\"wikitable\">"), "{gfm}");
    for value in [
        "<caption style=\"color:red\">Caption</caption>",
        "<tr class=\"row\">",
        "scope=\"col\"",
    ] {
        assert!(gfm.contains(value), "{gfm}");
    }
    let back = convert_text(Format::Gfm, Format::Wikitext, &gfm);
    let html = convert_text(Format::Wikitext, Format::Html, &back);
    assert!(html.contains("class=\"wikitable\""), "{html}");
    assert!(html.contains("scope=\"col\""), "{html}");
    assert!(html.contains(">Caption</caption>"), "{html}");
}

#[test]
fn footer_group_loss_is_reported_without_losing_cells() {
    use rwikitext::{convert_with_mode, ConversionMode, DiagnosticKind};
    let input = "<table><tr><td>Body</td></tr><tfoot><tr><td>Total</td></tr></tfoot></table>";
    let context = WikiContext::default();
    let request = || ConversionRequest {
        source: Format::Html,
        target: Format::Wikitext,
        input,
        context: &context,
    };
    let result = convert(request()).unwrap();
    assert!(result.content.contains("Body") && result.content.contains("Total"));
    assert!(
        result
            .diagnostics
            .iter()
            .any(|d| d.kind == DiagnosticKind::ConversionLoss && d.message.contains("footer")),
        "{:?}",
        result.diagnostics
    );
    assert!(!convert_with_mode(request(), ConversionMode::Strict)
        .unwrap_err()
        .diagnostics()
        .is_empty());
}

#[test]
fn official_rowspan_table_round_trips_through_cli() {
    use std::io::Write;
    use std::process::{Command, Stdio};
    // MediaWiki 1.46.2 tests/parser/tables.txt, "Table rowspan".
    let wiki = "{| border=1\n|Cell 1, row 1\n|rowspan=2|Cell 2, row 1 (and 2)\n|Cell 3, row 1\n|-\n|Cell 1, row 2\n|Cell 3, row 2\n|}";
    let run = |source: &str, target: &str, input: &str| {
        let mut child = Command::new(env!("CARGO_BIN_EXE_rwikitext"))
            .args([source, target])
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
        let output = child.wait_with_output().unwrap();
        assert!(
            output.status.success(),
            "{}",
            String::from_utf8_lossy(&output.stderr)
        );
        String::from_utf8(output.stdout).unwrap()
    };
    for target in ["gfm", "html"] {
        let middle = run("wikitext", target, wiki);
        assert!(middle.contains("rowspan=\"2\""), "{middle}");
        let back = run(target, "wikitext", &middle);
        let html = run("wikitext", "html", &back);
        for expected in ["rowspan=\"2\"", "Cell 2, row 1 (and 2)", "Cell 3, row 2"] {
            assert!(html.contains(expected), "{html}");
        }
    }
}

#[test]
fn native_gfm_table_keeps_links_and_literal_pipes_in_cells() {
    let wiki =
        "{|\n! [[Main Page|Page]] !! Value\n|-\n| A&#124;B || [https://example.org Link]\n|}";
    let gfm = convert_text(Format::Wikitext, Format::Gfm, wiki);
    assert!(gfm.starts_with('|'), "{gfm}");
    let back = convert_text(Format::Gfm, Format::Wikitext, &gfm);
    let html = convert_text(Format::Wikitext, Format::Html, &back);
    assert!(html.contains("href=\"./Main_Page\""), "{html}");
    assert!(html.contains("A|B"), "{html}");
    assert!(html.contains("href=\"https://example.org\""), "{html}");
}

#[test]
fn unsafe_table_attributes_are_removed_and_reported() {
    let input = "{| class=\"wikitable\" onclick=\"alert(1)\"\n| Safe\n|}";
    let context = WikiContext::default();
    let result = convert(ConversionRequest {
        source: Format::Wikitext,
        target: Format::Html,
        input,
        context: &context,
    })
    .unwrap();
    assert!(result.content.contains("Safe"));
    assert!(!result.content.contains("onclick"));
    assert!(!result.diagnostics.is_empty());
}

#[test]
fn official_nested_table_survives_both_round_trips() {
    let wiki = "{| border=1\n| &alpha;\n|\n{| bgcolor=#ABCDEF border=2\n|nested\n|-\n|table\n|}\n|the original table again\n|}";
    for target in [Format::Gfm, Format::Html] {
        let intermediate = convert_text(Format::Wikitext, target, wiki);
        assert!(
            intermediate.contains("nested") && intermediate.contains("the original table again"),
            "{intermediate}"
        );
        let back = convert_text(target, Format::Wikitext, &intermediate);
        let output = convert_text(Format::Wikitext, Format::Html, &back);
        assert_eq!(output.matches("<table").count(), 2, "{output}");
        assert!(
            output.contains("nested") && output.contains("the original table again"),
            "{output}"
        );
    }
}

#[test]
fn html_cell_with_list_needs_html_fallback() {
    let html = "<table><tr><th>Header</th></tr><tr><td><ul><li>First</li><li>Second</li></ul></td></tr></table>";
    let wiki = convert_text(Format::Html, Format::Wikitext, html);
    let gfm = convert_text(Format::Wikitext, Format::Gfm, &wiki);
    assert!(gfm.starts_with("<table"), "{gfm}");
    assert!(gfm.contains("First") && gfm.contains("Second"), "{gfm}");
}

#[test]
fn table_attribute_markup_is_escaped_in_wikitext() {
    let html = "<table class=\"foo|bar\"><tr><td title=\"{{hello}}|world\">Cell</td></tr></table>";
    let wiki = convert_text(Format::Html, Format::Wikitext, html);
    assert!(!wiki.contains("{{hello}}"), "{wiki}");
    let result = convert_text(Format::Wikitext, Format::Html, &wiki);
    assert!(result.contains("foo|bar"), "{result}");
    assert!(result.contains("{{hello}}|world"), "{result}");
    assert!(result.contains(">Cell</td>"), "{result}");
}

#[test]
fn nested_table_round_trips_through_cli() {
    use std::io::Write;
    use std::process::{Command, Stdio};
    let wiki = "{| border=1\n| &alpha;\n|\n{| bgcolor=#ABCDEF border=2\n|nested\n|-\n|table\n|}\n|the original table again\n|}";
    let run = |source: &str, target: &str, input: &str| {
        let mut child = Command::new(env!("CARGO_BIN_EXE_rwikitext"))
            .args([source, target])
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
        let output = child.wait_with_output().unwrap();
        assert!(
            output.status.success(),
            "{}",
            String::from_utf8_lossy(&output.stderr)
        );
        String::from_utf8(output.stdout).unwrap()
    };
    for target in ["gfm", "html"] {
        let middle = run("wikitext", target, wiki);
        let back = run(target, "wikitext", &middle);
        let html = run("wikitext", "html", &back);
        assert_eq!(html.matches("<table").count(), 2, "{html}");
        assert!(
            html.contains("nested") && html.contains("the original table again"),
            "{html}"
        );
    }
}

#[test]
fn table_sanitization_preserves_html_removal_kind() {
    use rwikitext::DiagnosticKind;
    let input = "{| onclick=\"alert(1)\"\n| Safe\n|}";
    let context = WikiContext::default();
    let result = convert(ConversionRequest {
        source: Format::Wikitext,
        target: Format::Html,
        input,
        context: &context,
    })
    .unwrap();
    assert!(
        result
            .diagnostics
            .iter()
            .any(|d| d.kind == DiagnosticKind::HtmlRemoval),
        "{:?}",
        result.diagnostics
    );
}
