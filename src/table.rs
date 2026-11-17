use crate::{
    inline::{self, Inline},
    parse_html_inlines, render_inline, ConversionError, Diagnostic, Format,
};
use scraper::{ElementRef, Node};

#[derive(Debug)]
struct Cell {
    header: bool,
    colspan: u32,
    rowspan: u32,
    content: Vec<Inline>,
    attrs: String,
}

#[derive(Debug)]
pub(crate) struct Table {
    caption: Option<Vec<Inline>>,
    rows: Vec<Vec<Cell>>,
    losses: Vec<Diagnostic>,
    attrs: String,
    row_attrs: Vec<String>,
    caption_attrs: String,
}

fn fail(message: &str) -> ConversionError {
    ConversionError::new(message)
}

// A cell separator inside a wikilink belongs to the link, not the table.
fn split_cells(input: &str, delimiter: &str) -> Vec<String> {
    let mut result = Vec::new();
    let mut depth = 0usize;
    let mut start = 0;
    let mut at = 0;
    while at < input.len() {
        let tail = &input[at..];
        if tail.starts_with("[[") {
            depth += 1;
            at += 2;
        } else if tail.starts_with("]]") && depth > 0 {
            depth -= 1;
            at += 2;
        } else if depth == 0 && tail.starts_with(delimiter) {
            result.push(input[start..at].to_string());
            at += delimiter.len();
            start = at;
        } else {
            at += tail.chars().next().unwrap().len_utf8();
        }
    }
    result.push(input[start..].to_string());
    result
}

fn clean_attrs(tag: &str, attrs: &str, losses: &mut Vec<Diagnostic>) -> String {
    let (safe, diagnostics) = crate::html_policy::sanitize_tag_attributes(tag, attrs);
    losses.extend(diagnostics);
    safe
}

fn html_attrs(element: ElementRef<'_>, excluded: &[&str]) -> String {
    element
        .value()
        .attrs()
        .filter(|(key, _)| !excluded.contains(key) && *key != "data-parsoid")
        .map(|(key, value)| {
            format!(
                " {key}=\"{}\"",
                html_escape::encode_double_quoted_attribute(value)
            )
        })
        .collect()
}

fn spans(
    attrs: &str,
    tag: &str,
    losses: &mut Vec<Diagnostic>,
) -> Result<(u32, u32, String), ConversionError> {
    let fragment =
        scraper::Html::parse_fragment(&format!("<table><tr><{tag} {attrs}></{tag}></tr></table>"));
    let selector = scraper::Selector::parse(tag).unwrap();
    let element = fragment
        .select(&selector)
        .next()
        .ok_or_else(|| fail("invalid table attributes"))?;
    let span = |name| -> Result<u32, ConversionError> {
        match element.value().attr(name) {
            Some(value) => value
                .parse::<u32>()
                .ok()
                .filter(|v| *v > 0)
                .ok_or_else(|| fail("invalid table span")),
            None => Ok(1),
        }
    };
    Ok((
        span("colspan")?,
        span("rowspan")?,
        clean_attrs(tag, &html_attrs(element, &["colspan", "rowspan"]), losses),
    ))
}

fn contains_block_html(content: &[Inline]) -> bool {
    content.iter().any(|part| match part {
        Inline::Html(html) => {
            let doc = scraper::Html::parse_fragment(html);
            doc.root_element()
                .children()
                .filter_map(ElementRef::wrap)
                .any(|element| crate::html_policy::is_block(element.value().name()))
        }
        Inline::Emphasis { children, .. } => contains_block_html(children),
        _ => false,
    })
}

fn render_cell_wiki(content: &[Inline]) -> Result<String, ConversionError> {
    let mut result = String::new();
    let mut last = 0;
    for (index, part) in content.iter().enumerate() {
        if let Inline::Html(html) = part {
            let doc = scraper::Html::parse_fragment(html);
            let mut elements = doc.root_element().children().filter_map(ElementRef::wrap);
            if let Some(element) = elements.next() {
                if element.value().name() == "table" && elements.next().is_none() {
                    result.push_str(&render_inline(&content[last..index], Format::Wikitext));
                    result.push('\n');
                    result.push_str(&Table::from_html(element)?.render(Format::Wikitext)?);
                    result.push('\n');
                    last = index + 1;
                }
            }
        }
    }
    result.push_str(&render_inline(&content[last..], Format::Wikitext));
    Ok(result)
}

impl Table {
    pub(crate) fn wiki_import_losses(&self) -> &[Diagnostic] {
        &self.losses
    }

    /// Report table semantics that are not modeled while retaining the cells.
    pub(crate) fn html_import_losses(element: ElementRef<'_>) -> Vec<String> {
        let mut losses = Vec::new();
        for node in element.descendants() {
            let Some(item) = ElementRef::wrap(node) else {
                continue;
            };
            let name = item.value().name();
            if !matches!(
                name,
                "table" | "thead" | "tbody" | "tfoot" | "tr" | "th" | "td" | "caption"
            ) {
                continue;
            }
            if name == "tfoot" {
                losses.push("table footer grouping cannot be preserved".to_string());
            }
            if matches!(name, "thead" | "tbody" | "tfoot") {
                for (key, _) in item.value().attrs() {
                    if key != "data-parsoid" {
                        losses.push(format!("table {name} attribute {key} cannot be preserved"));
                    }
                }
            }
        }
        losses.sort();
        losses.dedup();
        losses
    }
    pub(crate) fn parse_wiki(source: &str, language: &str) -> Result<Self, ConversionError> {
        let mut lines = source.lines();
        let first = lines.next().ok_or_else(|| fail("empty table"))?;
        let mut table = Self {
            caption: None,
            rows: Vec::new(),
            losses: Vec::new(),
            attrs: String::new(),
            row_attrs: Vec::new(),
            caption_attrs: String::new(),
        };
        table.attrs = clean_attrs(
            "table",
            first.strip_prefix("{|").unwrap_or("").trim(),
            &mut table.losses,
        );
        let mut current_attrs = String::new();
        let mut current: Vec<Cell> = Vec::new();
        let mut closed = false;
        while let Some(line) = lines.next() {
            if line.starts_with("{|") {
                let cell = current
                    .last_mut()
                    .ok_or_else(|| fail("nested table requires a parent cell"))?;
                let mut nested_lines = vec![line];
                let mut depth = 1usize;
                while depth > 0 {
                    let nested_line = lines.next().ok_or_else(|| fail("unclosed nested table"))?;
                    if nested_line.starts_with("{|") {
                        depth += 1;
                    }
                    if nested_line.trim() == "|}" {
                        depth -= 1;
                    }
                    nested_lines.push(nested_line);
                }
                let nested = Self::parse_wiki(&nested_lines.join("\n"), language)?;
                table.losses.extend(nested.losses.iter().cloned());
                cell.content
                    .push(Inline::Html(nested.render(Format::Html)?));
                continue;
            }
            if line.trim() == "|}" {
                closed = true;
                break;
            }
            if let Some(attributes) = line.strip_prefix("|-") {
                if !current.is_empty() {
                    table.rows.push(std::mem::take(&mut current));
                    table.row_attrs.push(std::mem::take(&mut current_attrs));
                }
                current_attrs = clean_attrs("tr", attributes.trim(), &mut table.losses);
            } else if let Some(caption) = line.strip_prefix("|+") {
                if table.caption.is_some() || !current.is_empty() || !table.rows.is_empty() {
                    return Err(fail("unsupported table caption"));
                }
                let parts = split_cells(caption, "|");
                let caption = if parts.len() > 1 {
                    table.caption_attrs =
                        clean_attrs("caption", parts[0].trim(), &mut table.losses);
                    parts[1..].join("|")
                } else {
                    caption.to_string()
                };
                table.caption = Some(inline::parse_wiki(caption.trim(), language)?);
            } else if line.starts_with('!') || line.starts_with('|') {
                let header = line.starts_with('!');
                for cell in split_cells(&line[1..], if header { "!!" } else { "||" }) {
                    let parts = split_cells(&cell, "|");
                    let (colspan, rowspan, attrs, text) = if parts.len() > 1 {
                        let (colspan, rowspan, attrs) = spans(
                            parts[0].trim(),
                            if header { "th" } else { "td" },
                            &mut table.losses,
                        )?;
                        (colspan, rowspan, attrs, parts[1..].join("|"))
                    } else {
                        (1, 1, String::new(), cell)
                    };
                    current.push(Cell {
                        header,
                        colspan,
                        rowspan,
                        content: inline::parse_wiki(text.trim(), language)?,
                        attrs,
                    });
                }
            } else if let Some(cell) = current.last_mut() {
                let mut continuation = inline::parse_wiki(line, language)?;
                inline::text(&mut cell.content, "\n");
                cell.content.append(&mut continuation);
            } else {
                return Err(fail("unsupported table content"));
            }
        }
        if !closed {
            return Err(fail("unclosed table"));
        }
        if !current.is_empty() {
            table.rows.push(current);
            table.row_attrs.push(current_attrs);
        }
        if table.rows.is_empty() {
            return Err(fail("empty tables are unsupported"));
        }
        Ok(table)
    }

    pub(crate) fn from_html(element: ElementRef<'_>) -> Result<Self, ConversionError> {
        fn add_rows(element: ElementRef<'_>, table: &mut Table) -> Result<(), ConversionError> {
            for child in element.children() {
                match child.value() {
                    Node::Text(t) if t.trim().is_empty() => {}
                    Node::Comment(_) => {}
                    Node::Element(el) if matches!(el.name(), "thead" | "tbody" | "tfoot") => {
                        add_rows(ElementRef::wrap(child).unwrap(), table)?
                    }
                    Node::Element(el) if el.name() == "caption" => {
                        if table.caption.is_some() {
                            return Err(fail("multiple table captions are unsupported"));
                        }
                        let caption = ElementRef::wrap(child).unwrap();
                        table.caption_attrs =
                            clean_attrs("caption", &html_attrs(caption, &[]), &mut table.losses);
                        table.caption = Some(parse_html_inlines(caption)?);
                    }
                    Node::Element(el) if el.name() == "tr" => {
                        let mut row = Vec::new();
                        for cell in child.children() {
                            match cell.value() {
                                Node::Text(t) if t.trim().is_empty() => {}
                                Node::Comment(_) => {}
                                Node::Element(c) if matches!(c.name(), "td" | "th") => {
                                    let span = |name| -> Result<u32, ConversionError> {
                                        match c.attr(name) {
                                            Some(v) => v
                                                .parse::<u32>()
                                                .ok()
                                                .filter(|v| *v > 0)
                                                .ok_or_else(|| fail("invalid table span")),
                                            None => Ok(1),
                                        }
                                    };
                                    let cell_element = ElementRef::wrap(cell).unwrap();
                                    let attrs = clean_attrs(
                                        c.name(),
                                        &html_attrs(cell_element, &["rowspan", "colspan"]),
                                        &mut table.losses,
                                    );
                                    row.push(Cell {
                                        attrs,
                                        header: c.name() == "th",
                                        colspan: span("colspan")?,
                                        rowspan: span("rowspan")?,
                                        content: parse_html_inlines(
                                            ElementRef::wrap(cell).unwrap(),
                                        )?,
                                    });
                                }
                                _ => return Err(fail("unsupported table row content")),
                            }
                        }
                        if row.is_empty() {
                            return Err(fail("empty table rows are unsupported"));
                        }
                        table.row_attrs.push(clean_attrs(
                            "tr",
                            &html_attrs(ElementRef::wrap(child).unwrap(), &[]),
                            &mut table.losses,
                        ));
                        table.rows.push(row);
                    }
                    _ => return Err(fail("unsupported table content")),
                }
            }
            Ok(())
        }
        let mut table = Self {
            caption: None,
            rows: Vec::new(),
            losses: Vec::new(),
            attrs: String::new(),
            row_attrs: Vec::new(),
            caption_attrs: String::new(),
        };
        table.attrs = clean_attrs("table", &html_attrs(element, &[]), &mut table.losses);
        add_rows(element, &mut table)?;
        if table.rows.is_empty() {
            return Err(fail("empty tables are unsupported"));
        }
        Ok(table)
    }

    pub(crate) fn render(&self, format: Format) -> Result<String, ConversionError> {
        let native = self.attrs.is_empty()
            && self.row_attrs.iter().all(String::is_empty)
            && self.caption.is_none()
            && !self.rows.is_empty()
            && !self.rows[0].is_empty()
            && self.rows[0].iter().all(|c| c.header)
            && self
                .rows
                .iter()
                .skip(1)
                .all(|r| r.len() == self.rows[0].len() && r.iter().all(|c| !c.header))
            && self.rows.iter().flatten().all(|c| {
                c.colspan == 1
                    && c.rowspan == 1
                    && c.attrs.is_empty()
                    && !inline::plain(&c.content).contains('\n')
                    && !contains_block_html(&c.content)
            });
        if format == Format::Gfm && native {
            let row = |cells: &[Cell]| {
                format!(
                    "| {} |",
                    cells
                        .iter()
                        .map(|c| render_inline(&c.content, Format::Gfm))
                        .collect::<Vec<_>>()
                        .join(" | ")
                )
            };
            let mut output = vec![
                row(&self.rows[0]),
                format!("| {} |", vec!["---"; self.rows[0].len()].join(" | ")),
            ];
            output.extend(self.rows.iter().skip(1).map(|r| row(r)));
            return Ok(output.join("\n"));
        }
        if format == Format::Wikitext {
            let attr = crate::html_policy::escape_wiki_attribute_markup;
            let mut output = format!("{{|{}", attr(&self.attrs));
            if let Some(caption) = &self.caption {
                output.push_str(&format!(
                    "\n|+{}{} {}",
                    attr(&self.caption_attrs),
                    if self.caption_attrs.is_empty() {
                        ""
                    } else {
                        " |"
                    },
                    render_inline(caption, format)
                ));
            }
            for (row, attrs) in self.rows.iter().zip(&self.row_attrs) {
                output.push_str(&format!("\n|-{}", attr(attrs)));
                for cell in row {
                    output.push('\n');
                    output.push(if cell.header { '!' } else { '|' });
                    output.push_str(&attr(&cell.attrs));
                    if cell.colspan != 1 {
                        output.push_str(&format!(" colspan=\"{}\"", cell.colspan));
                    }
                    if cell.rowspan != 1 {
                        output.push_str(&format!(" rowspan=\"{}\"", cell.rowspan));
                    }
                    if cell.colspan != 1 || cell.rowspan != 1 || !cell.attrs.is_empty() {
                        output.push_str(" |");
                    }
                    output.push(' ');
                    output.push_str(&render_cell_wiki(&cell.content)?);
                }
            }
            output.push_str("\n|}");
            return Ok(output);
        }
        let mut output = format!("<table{}>", self.attrs);
        if let Some(caption) = &self.caption {
            output.push_str(&format!(
                "<caption{}>{}</caption>",
                self.caption_attrs,
                render_inline(caption, Format::Html)
            ));
        }
        output.push_str("<tbody>");
        for (row, attrs) in self.rows.iter().zip(&self.row_attrs) {
            output.push_str(&format!("<tr{attrs}>"));
            for cell in row {
                let tag = if cell.header { "th" } else { "td" };
                output.push_str(&format!("<{tag}{}", cell.attrs));
                if cell.colspan != 1 {
                    output.push_str(&format!(" colspan=\"{}\"", cell.colspan));
                }
                if cell.rowspan != 1 {
                    output.push_str(&format!(" rowspan=\"{}\"", cell.rowspan));
                }
                output.push('>');
                output.push_str(&render_inline(&cell.content, Format::Html));
                output.push_str(&format!("</{tag}>"));
            }
            output.push_str("</tr>");
        }
        output.push_str("</tbody></table>");
        Ok(output)
    }
}
