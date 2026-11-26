use crate::{html_policy, inline, Format, LookupError, WikiData};
use scraper::{ElementRef, Node};
use std::ops::Range;

/// Information needed to render a wiki file.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FileInfo {
    pub url: String,
    pub width: Option<u32>,
    pub height: Option<u32>,
}

pub(crate) struct Edit {
    pub range: Range<usize>,
    pub html: String,
    pub loss: Option<String>,
}

fn attr(s: &str) -> String {
    html_escape::encode_double_quoted_attribute(s).into_owned()
}
fn txt(s: &str) -> String {
    html_escape::encode_text(s).into_owned()
}
fn first_upper(s: &str) -> String {
    let mut c = s.chars();
    c.next()
        .map_or_else(String::new, |first| first.to_uppercase().chain(c).collect())
}
fn file_title(name: &str) -> String {
    format!(
        "File:{}",
        first_upper(
            &name
                .replace('_', " ")
                .split_whitespace()
                .collect::<Vec<_>>()
                .join(" ")
        )
    )
}
fn href(title: &str) -> String {
    format!("./{}", inline::encode_component(&title.replace(' ', "_")))
}
fn safe_url(value: &str) -> bool {
    (html_policy::external_url(value) || (value.starts_with('/') && !value.starts_with("//")))
        && !value
            .chars()
            .any(|c| c.is_whitespace() || c.is_control() || matches!(c, '<' | '>' | '"' | '\\'))
}

fn file_size(option: &str) -> Option<(Option<u32>, Option<u32>)> {
    let raw = option.strip_suffix("px")?;
    let (width, height) = raw.split_once('x').unwrap_or((raw, ""));
    let dimension = |value: &str| {
        if value.is_empty() {
            Some(None)
        } else {
            Some(Some(value.parse::<u32>().ok().filter(|n| *n > 0)?))
        }
    };
    let (width, height) = (dimension(width)?, dimension(height)?);
    (width.is_some() || height.is_some()).then_some((width, height))
}

fn display_size(
    info: &FileInfo,
    size: Option<(Option<u32>, Option<u32>)>,
) -> (Option<u32>, Option<u32>) {
    let Some((width, height)) = size else {
        return (info.width, info.height);
    };
    let (Some(ow), Some(oh)) = (
        info.width.filter(|w| *w > 0).map(u64::from),
        info.height.filter(|h| *h > 0).map(u64::from),
    ) else {
        return (width, height);
    };
    let round = |product: u64, divisor: u64| {
        u32::try_from(((product + divisor / 2) / divisor).max(1)).unwrap_or(u32::MAX)
    };
    if let Some(h) = height.filter(|h| width.is_none_or(|w| u64::from(*h) * ow < u64::from(w) * oh))
    {
        (Some(round(ow * u64::from(h), oh)), Some(h))
    } else {
        let w = width.expect("width or height is present");
        (Some(w), Some(round(oh * u64::from(w), ow)))
    }
}
fn html_label(label: &str, language: &str) -> Option<String> {
    inline::parse_wiki(label, language)
        .ok()
        .map(|parts| {
            crate::render_inline(
                &parts,
                crate::RenderTarget::Html(crate::HtmlStyle::Parsoid),
            )
        })
}

fn caption_html(
    caption: &str,
    language: &str,
    data: Option<&dyn WikiData>,
    offset: usize,
    depth: usize,
) -> Result<(String, Vec<String>), (Range<usize>, LookupError)> {
    if depth >= 32 {
        return Ok((
            txt(caption),
            vec!["media caption nesting limit exceeded".into()],
        ));
    }
    let edits = scan_inner(caption, language, Format::Html, data, depth + 1)
        .map_err(|(range, error)| ((offset + range.start)..(offset + range.end), error))?;
    let mut source = String::new();
    let mut cursor = 0;
    let mut replacements = Vec::new();
    let mut losses = Vec::new();
    for (index, edit) in edits.into_iter().enumerate() {
        source.push_str(&caption[cursor..edit.range.start]);
        let mut marker = format!("\u{e000}caption{index}\u{e001}");
        while caption.contains(&marker) {
            marker.push('x');
        }
        source.push_str(&marker);
        cursor = edit.range.end;
        replacements.push((marker, edit.html));
        if let Some(loss) = edit.loss {
            losses.push(loss);
        }
    }
    source.push_str(&caption[cursor..]);
    let mut html = html_label(&source, language).unwrap_or_else(|| txt(&source));
    for (marker, replacement) in replacements {
        html = html.replace(&marker, &replacement);
    }
    Ok((html, losses))
}

fn link_end(input: &str) -> Option<usize> {
    let mut depth = 0usize;
    let mut pos = 0;
    while pos < input.len() {
        let rest = &input[pos..];
        if rest.starts_with("[[") {
            depth += 1;
            pos += 2;
        } else if rest.starts_with("]]") {
            depth = depth.checked_sub(1)?;
            if depth == 0 {
                return Some(pos);
            }
            pos += 2;
        } else {
            pos += rest.chars().next()?.len_utf8();
        }
    }
    None
}

/// Locate eligible links without rewriting literal nowiki, pre, or comment content.
pub(crate) fn scan(
    input: &str,
    language: &str,
    target: Format,
    data: Option<&dyn WikiData>,
) -> Result<Vec<Edit>, (Range<usize>, LookupError)> {
    scan_inner(input, language, target, data, 0)
}

fn scan_inner(
    input: &str,
    language: &str,
    target: Format,
    data: Option<&dyn WikiData>,
    depth: usize,
) -> Result<Vec<Edit>, (Range<usize>, LookupError)> {
    let mut edits = Vec::new();
    let mut pos = 0;
    while pos < input.len() {
        let rest = &input[pos..];
        if rest.starts_with("<!--") {
            pos += rest.find("-->").map_or(rest.len(), |n| n + 3);
            continue;
        }
        if let Some((name, opening, closing)) = html_policy::tag_info(rest) {
            let literal = matches!(
                name.as_str(),
                "nowiki" | "pre" | "source" | "syntaxhighlight"
            );
            let self_closing = rest[..opening]
                .trim_end_matches('>')
                .trim_end()
                .ends_with('/');
            pos += if literal && !closing && !self_closing {
                html_policy::raw_close(&rest[opening..], &name)
                    .map_or(rest.len(), |(_, end)| opening + end)
            } else {
                opening
            };
            continue;
        }
        if !rest.starts_with("[[") {
            pos += rest.chars().next().unwrap().len_utf8();
            continue;
        }
        let Some(end) = link_end(rest) else { break };
        let body = &rest[2..end];
        let span = pos..pos + end + 2;
        let items = crate::expansion::split_top(body, '|');
        let mut items = items.into_iter();
        let raw_title =
            html_escape::decode_html_entities(items.next().unwrap_or("").trim()).into_owned();
        let options: Vec<_> = items.collect();
        let is_file = raw_title.split_once(':').is_some_and(|(ns, _)| {
            ns.eq_ignore_ascii_case("file") || ns.eq_ignore_ascii_case("image")
        });
        let is_media = raw_title
            .split_once(':')
            .is_some_and(|(ns, _)| ns.eq_ignore_ascii_case("media"));
        if is_media && options.len() <= 1 {
            let title = file_title(raw_title.split_once(':').unwrap().1);
            let found = if let Some(data) = data {
                data.file_info(&title.replace(' ', "_"))
                    .map_err(|e| (span.clone(), e))?
            } else {
                None
            };
            let trail = rest[end + 2..]
                .bytes()
                .take_while(u8::is_ascii_lowercase)
                .count();
            let label = options.first().copied().unwrap_or(&raw_title);
            if let Some(mut label) = html_label(label, language) {
                label.push_str(&txt(&rest[end + 2..end + 2 + trail]));
                let unsafe_url = found.as_ref().is_some_and(|f| !safe_url(&f.url));
                let dest = found
                    .filter(|f| safe_url(&f.url))
                    .map_or_else(|| href(&title), |f| f.url);
                edits.push(Edit {
                    range: pos..span.end + trail,
                    html: format!(
                        "<a rel=\"mw:MediaLink\" resource=\"{}\" href=\"{}\">{label}</a>",
                        attr(&href(&format!(
                            "Media:{}",
                            title.split_once(':').unwrap().1
                        ))),
                        attr(&dest)
                    ),
                    loss: unsafe_url.then(|| "unsafe file URL".into()),
                });
            }
        } else if is_file {
            let title = file_title(raw_title.split_once(':').unwrap().1);
            let found = if let Some(data) = data {
                data.file_info(&title.replace(' ', "_"))
                    .map_err(|e| (span.clone(), e))?
            } else {
                None
            };
            let mut alt: Option<String> = None;
            let mut caption = "";
            let mut thumb = false;
            let mut alignment: Option<&str> = None;
            let mut size = None;
            let mut link = Some(href(&title));
            let mut unsupported = Vec::new();
            if found.as_ref().is_some_and(|f| !safe_url(&f.url)) {
                unsupported.push("unsafe file URL");
            }
            for option in &options {
                let option = option.trim();
                if let Some(v) = option.strip_prefix("alt=") {
                    alt = Some(html_escape::decode_html_entities(v).into_owned());
                } else if let Some(v) = option.strip_prefix("link=") {
                    let v = html_escape::decode_html_entities(v);
                    link = if v.is_empty() {
                        None
                    } else if html_policy::external_url(&v) {
                        Some(v.into_owned())
                    } else if !v.contains(['[', ']', '<', '>', '|', '\n', '\r']) {
                        Some(href(&v))
                    } else {
                        unsupported.push("invalid link target");
                        None
                    };
                } else if matches!(option, "thumb" | "thumbnail") {
                    thumb = true;
                } else if matches!(option, "frame" | "framed") {
                    thumb = true;
                    unsupported.push("frame presentation");
                } else if let Some(dimensions) = file_size(option) {
                    size = Some(dimensions);
                } else if matches!(option, "left" | "right" | "center" | "none") {
                    alignment = Some(option);
                } else if matches!(option, "border" | "frameless") {
                    unsupported.push(option);
                } else if !option.is_empty() {
                    caption = option;
                }
            }
            let resource = href(&title);
            let caption_offset = if caption.is_empty() {
                pos + 2
            } else {
                caption.as_ptr() as usize - input.as_ptr() as usize
            };
            let (caption_html, nested_losses) =
                caption_html(caption, language, data, caption_offset, depth)?;
            unsupported.extend(nested_losses.iter().map(String::as_str));
            let html = if let Some(info) = found.filter(|f| safe_url(&f.url)) {
                let mut img = format!(
                    "<img src=\"{}\" resource=\"{}\"",
                    attr(&info.url),
                    attr(&resource)
                );
                if let Some(alt) = alt
                    .as_deref()
                    .or_else(|| (!thumb && !caption.is_empty()).then_some(caption))
                {
                    img.push_str(&format!(" alt=\"{}\"", attr(alt)));
                }
                let (width, height) = display_size(&info, size);
                if let Some(w) = width {
                    img.push_str(&format!(" width=\"{w}\""));
                }
                if let Some(h) = height {
                    img.push_str(&format!(" height=\"{h}\""));
                }
                img.push('>');
                if let Some(link) = link {
                    img = format!("<a href=\"{}\">{img}</a>", attr(&link));
                }
                if thumb || alignment.is_some() {
                    let class = alignment.map_or(String::new(), |align| {
                        format!(" class=\"mw-halign-{align}\"")
                    });
                    let kind = if thumb { "mw:File/Thumb" } else { "mw:File" };
                    format!("<figure{class} typeof=\"{kind}\">{img}<figcaption>{caption_html}</figcaption></figure>")
                } else {
                    if !caption.is_empty() {
                        unsupported.push("inline caption");
                    }
                    format!("<span typeof=\"mw:File\">{img}</span>")
                }
            } else {
                if alt.is_some() || size.is_some() || link.as_deref() != Some(resource.as_str()) {
                    unsupported
                        .push("alt text, dimensions or link override for an unavailable image");
                }
                let name = title.split_once(':').unwrap().1.replace(' ', "_");
                let broken = format!("<a href=\"./Special:FilePath/{}\" class=\"new\" title=\"{}\"><span class=\"mw-file-element mw-broken-media\" resource=\"{}\">{}</span></a>", attr(&inline::encode_component(&name)), attr(&title), attr(&resource), txt(&title));
                if thumb {
                    format!("<figure typeof=\"mw:Error mw:File/Thumb\">{broken}<figcaption>{caption_html}</figcaption></figure>")
                } else {
                    format!("<span typeof=\"mw:Error mw:File\">{broken}</span>")
                }
            };
            let loss = (!unsupported.is_empty()).then(|| {
                format!(
                    "file options cannot be represented: {}",
                    unsupported.join(", ")
                )
            });
            edits.push(Edit {
                range: span.clone(),
                html,
                loss,
            });
        } else if let Some(data) = data {
            let target_title = raw_title.trim_start_matches(':').trim_start();
            if !target_title.starts_with('#') && options.len() <= 1 {
                let page = target_title.split('#').next().unwrap_or("");
                let normalized = inline::normalize_title(page);
                if !normalized.is_empty() && inline::internal_href(page).is_ok() {
                    let lookup_title = normalized.replace('_', " ");
                    let exists = data
                        .page_exists(&normalized)
                        .map_err(|e| (span.clone(), e))?
                        .unwrap_or(false);
                    if !exists && target == Format::Html {
                        let full_href = match inline::internal_href(&raw_title) {
                            Ok(v) => v,
                            Err(_) => {
                                pos = span.end;
                                continue;
                            }
                        };
                        let (path, fragment) = full_href
                            .split_once('#')
                            .map_or((full_href.as_str(), None), |(a, b)| (a, Some(b)));
                        let red = format!(
                            "{path}?action=edit&redlink=1{}",
                            fragment.map_or(String::new(), |f| format!("#{f}"))
                        );
                        let trail = rest[end + 2..]
                            .bytes()
                            .take_while(u8::is_ascii_lowercase)
                            .count();
                        let label = options.first().copied().unwrap_or(target_title);
                        if let Some(mut label) = html_label(label, language) {
                            label.push_str(&txt(&rest[end + 2..end + 2 + trail]));
                            edits.push(Edit { range: pos..span.end+trail, html: format!("<a rel=\"mw:WikiLink\" href=\"{}\" title=\"{}\" class=\"new\">{label}</a>", attr(&red), attr(&lookup_title)), loss: None });
                        }
                    }
                }
            }
        }
        pos = span.end;
    }
    Ok(edits)
}

fn wiki_option(value: &str) -> String {
    let mut out = String::new();
    for c in value.chars() {
        if matches!(c, '&' | '|' | '[' | ']' | '{' | '}' | '\n' | '\r') {
            out.push_str(&format!("&#{};", c as u32));
        } else {
            out.push(c);
        }
    }
    out
}

pub(crate) fn file_element(element: ElementRef<'_>) -> Option<ElementRef<'_>> {
    let kind = element.attr("typeof")?;
    if !matches!(element.value().name(), "figure" | "span")
        || !kind.split_whitespace().any(|t| {
            matches!(
                t,
                "mw:File" | "mw:File/Thumb" | "mw:File/Frame" | "mw:File/Frameless"
            )
        })
    {
        return None;
    }
    if element
        .children()
        .any(|node| matches!(node.value(), Node::Text(text) if !text.trim_ascii().is_empty()))
    {
        return None;
    }
    let mut children = element.child_elements();
    let first = children.next()?;
    if children.next().is_some_and(|child| {
        element.value().name() != "figure" || child.value().name() != "figcaption"
    }) || children.next().is_some()
    {
        return None;
    }
    let media = if first.value().name() == "a" {
        let mut inside = first.child_elements();
        let media = inside.next()?;
        if inside.next().is_some()
            || first.children().any(
                |node| matches!(node.value(), Node::Text(text) if !text.trim_ascii().is_empty()),
            )
        {
            return None;
        }
        media
    } else {
        first
    };
    (media.value().name() == "img"
        || media.value().name() == "span"
            && kind.split_whitespace().any(|kind| kind == "mw:Error")
            && media.child_elements().next().is_none())
    .then_some(media)
}

pub(crate) fn file_resource(media: ElementRef<'_>) -> Option<String> {
    let resource = media
        .attr("resource")?
        .strip_prefix("./")
        .unwrap_or(media.attr("resource")?);
    let title = inline::decode_component(resource).ok()?.replace('_', " ");
    let (namespace, name) = title.split_once(':')?;
    if !namespace.eq_ignore_ascii_case("file")
        || name.is_empty()
        || title
            .chars()
            .any(|c| c.is_control() || matches!(c, '[' | ']' | '{' | '}' | '|' | '<' | '>'))
    {
        return None;
    }
    Some(title)
}

/// Recover a file from Parsoid's `mw:File` wrapper and validated resource.
pub(crate) fn media_to_wikitext(element: ElementRef<'_>) -> Option<String> {
    let kind = element.attr("typeof")?;
    let media = file_element(element)?;
    let title = file_resource(media)?;
    let mut wiki = format!("[[{}", wiki_option(&title));
    if kind.split_whitespace().any(|t| t == "mw:File/Thumb") {
        wiki.push_str("|thumb");
    }
    if let Some(alignment) = element
        .attr("class")
        .and_then(|classes| {
            classes
                .split_whitespace()
                .find_map(|class| class.strip_prefix("mw-halign-"))
        })
        .filter(|s| matches!(*s, "left" | "right" | "center" | "none"))
    {
        wiki.push('|');
        wiki.push_str(alignment);
    }
    if let Some(alt) = media.attr("alt") {
        wiki.push_str("|alt=");
        wiki.push_str(&wiki_option(alt));
    }
    if let Some(width) = media
        .attr("width")
        .filter(|w| w.parse::<u32>().is_ok_and(|n| n > 0))
    {
        wiki.push_str(&format!("|{width}px"));
    } else if let Some(height) = media
        .attr("height")
        .filter(|h| h.parse::<u32>().is_ok_and(|n| n > 0))
    {
        wiki.push_str(&format!("|x{height}px"));
    }
    let anchor = media
        .ancestors()
        .filter_map(ElementRef::wrap)
        .take_while(|p| p.id() != element.id())
        .find(|p| p.value().name() == "a");
    if let Some(anchor) = anchor {
        if let Some(dest) = anchor.attr("href") {
            let dest_title = dest
                .strip_prefix("./")
                .and_then(|s| inline::decode_component(s).ok())
                .map(|s| s.replace('_', " "));
            if dest_title.as_deref() != Some(title.as_str()) && !kind.contains("mw:Error") {
                if html_policy::external_url(dest) {
                    wiki.push_str("|link=");
                    wiki.push_str(&wiki_option(dest));
                } else if let Some(dest) = dest_title.filter(|s| {
                    !s.chars().any(|c| {
                        c.is_control() || matches!(c, '[' | ']' | '{' | '}' | '|' | '<' | '>')
                    })
                }) {
                    wiki.push_str("|link=");
                    wiki.push_str(&wiki_option(&dest));
                }
            }
        }
    } else if !kind.contains("mw:Error") {
        wiki.push_str("|link=");
    }
    if let Some(caption) = element
        .child_elements()
        .find(|child| child.value().name() == "figcaption")
    {
        let text = crate::html_to_wiki(&caption.inner_html());
        if !text.is_empty() {
            wiki.push('|');
            wiki.push_str(&text);
        }
    }
    wiki.push_str("]]");
    Some(wiki)
}

pub(crate) fn media_link_name(element: ElementRef<'_>) -> Option<String> {
    if element.value().name() != "a"
        || !element
            .attr("rel")?
            .split_whitespace()
            .any(|r| r == "mw:MediaLink")
    {
        return None;
    }
    let resource = element
        .attr("resource")?
        .strip_prefix("./")
        .unwrap_or(element.attr("resource")?);
    let title = inline::decode_component(resource).ok()?.replace('_', " ");
    let (namespace, name) = title.split_once(':')?;
    if !(namespace.eq_ignore_ascii_case("file") || namespace.eq_ignore_ascii_case("media"))
        || name.is_empty()
        || name
            .chars()
            .any(|c| c.is_control() || matches!(c, '[' | ']' | '{' | '}' | '|' | '<' | '>'))
    {
        return None;
    }
    Some(name.to_owned())
}

/// Recover a direct Media: link with a validated Parsoid resource.
pub(crate) fn media_link_to_wikitext(element: ElementRef<'_>) -> Option<String> {
    let name = media_link_name(element)?;
    let label = crate::html_to_wiki(&element.inner_html());
    Some(format!("[[Media:{}|{}]]", wiki_option(&name), label,))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::cell::RefCell;
    #[derive(Default)]
    struct Data(RefCell<Vec<String>>);
    impl WikiData for Data {
        fn page_exists(&self, title: &str) -> Result<Option<bool>, LookupError> {
            self.0.borrow_mut().push(title.into());
            Ok(Some(false))
        }
        fn file_info(&self, title: &str) -> Result<Option<FileInfo>, LookupError> {
            self.0.borrow_mut().push(title.into());
            Ok(None)
        }
    }
    #[test]
    fn skips_literals_and_uses_canonical_titles() {
        let data = Data::default();
        let input = "<nowiki>[[Ignore]]</nowiki> <!-- [[Ignore]] --> [[missing page|Label]]s [[File:missing.jpg|thumb|Caption]]";
        let edits = scan(input, "en", Format::Html, Some(&data)).unwrap();
        assert_eq!(&*data.0.borrow(), &["Missing_page", "File:Missing.jpg"]);
        assert_eq!(edits.len(), 2);
        assert!(edits[0].html.contains("Labels"));
        assert!(edits[0].html.contains("?action=edit&amp;redlink=1"));
        assert!(edits[1].html.contains("mw:Error mw:File/Thumb"));
        assert!(edits[1].html.contains("mw-broken-media"));
    }
    #[test]
    fn extracts_parsoid_file_resource() {
        let html = scraper::Html::parse_fragment("<figure typeof='mw:File/Thumb'><a href='./File:Foobar.jpg'><img src='http://example.com/foobar.jpg' resource='./File:Foobar.jpg' width='200' alt='text'></a><figcaption>Caption</figcaption></figure>");
        let selector = scraper::Selector::parse("figure").unwrap();
        let wiki = media_to_wikitext(html.select(&selector).next().unwrap()).unwrap();
        assert_eq!(wiki, "[[File:Foobar.jpg|thumb|alt=text|200px|Caption]]");
    }
}
