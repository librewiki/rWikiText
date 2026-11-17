use scraper::{ElementRef, Html, Node};

pub fn document_structure(html: &str) -> String {
    let mut output = String::new();
    let document = Html::parse_fragment(html);
    let mut at_block_boundary = true;
    children(
        document.root_element(),
        &mut output,
        &mut at_block_boundary,
        false,
    );
    trim_space(&mut output);
    output
}

fn is_block(name: &str) -> bool {
    matches!(
        name,
        "caption"
            | "dd"
            | "div"
            | "dl"
            | "dt"
            | "h1"
            | "h2"
            | "h3"
            | "h4"
            | "h5"
            | "h6"
            | "hr"
            | "li"
            | "ol"
            | "p"
            | "pre"
            | "table"
            | "td"
            | "th"
            | "tr"
            | "ul"
    )
}

fn trim_space(output: &mut String) {
    output.truncate(output.trim_end_matches(' ').len());
}

fn children(parent: ElementRef<'_>, output: &mut String, at_block_boundary: &mut bool, pre: bool) {
    for child in parent.children() {
        match child.value() {
            Node::Text(text) if pre => output.push_str(&html_escape::encode_text(&**text)),
            Node::Text(text) => {
                for ch in html_escape::encode_text(&**text).chars() {
                    if ch.is_ascii_whitespace() {
                        if !*at_block_boundary && !output.ends_with(' ') {
                            output.push(' ');
                        }
                    } else {
                        output.push(ch);
                        *at_block_boundary = false;
                    }
                }
            }
            Node::Element(node) => {
                if node
                    .attr("rel")
                    .is_some_and(|rel| rel.starts_with("mw:PageProp/"))
                {
                    continue;
                }
                let element = ElementRef::wrap(child).unwrap();
                // The GFM renderer supplies a header group for a simple table.
                let table_group = matches!(node.name(), "thead" | "tbody");
                let wrapper = node.name() == "span"
                    && (node
                        .attr("about")
                        .is_some_and(|about| about.starts_with("#mwt"))
                        || node.attr("typeof").is_some_and(|types| {
                            types.split_whitespace().any(|kind| {
                                matches!(kind, "mw:Transclusion" | "mw:Nowiki" | "mw:Entity")
                            })
                        }))
                    && node.attrs().all(|(name, _)| {
                        matches!(name, "about" | "typeof" | "data-mw" | "data-parsoid")
                    });
                if table_group || wrapper {
                    children(element, output, at_block_boundary, pre);
                    continue;
                }
                let name = match node.name() {
                    "em" => "i",
                    "strong" => "b",
                    other => other,
                };
                let block = is_block(name);
                if block && !pre {
                    trim_space(output);
                }
                let attrs = match name {
                    "a" => &["href"][..],
                    "img" => &["src", "alt", "width", "height"],
                    "span" => &["title", "style", "lang", "dir", "class"],
                    "table" => &["class", "style"],
                    "td" | "th" => &["rowspan", "colspan", "style"],
                    "ol" => &["start"],
                    _ => &[],
                };
                output.push('<');
                output.push_str(name);
                for attr in attrs {
                    if let Some(value) = element.attr(attr) {
                        // The PHP fixtures use the test wiki's /wiki/ article path.
                        // Parsoid and rWikiText write the same destinations relative to the page.
                        let value = if *attr == "href" {
                            value
                                .strip_prefix("/wiki/")
                                .unwrap_or(value.trim_start_matches("./"))
                        } else {
                            value
                        };
                        let value = html_escape::encode_double_quoted_attribute(value);
                        output.push_str(&format!(" {attr}=\"{value}\""));
                    }
                }
                output.push('>');
                let in_pre = pre || name == "pre";
                let mut child_boundary = block || *at_block_boundary;
                children(element, output, &mut child_boundary, in_pre);
                if block && !in_pre {
                    trim_space(output);
                }
                output.push_str(&format!("</{name}>"));
                *at_block_boundary = block;
            }
            Node::Comment(_) => {}
            other => panic!("unexpected DOM node {other:?}"),
        }
    }
}
