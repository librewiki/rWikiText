use crate::WikiContext;

// This subset corresponds to context values available in WikiContext. Unknown names
// remain available for template lookup instead of being replaced with fabricated data.
fn variable(name: &str, context: &WikiContext<'_>) -> Option<String> {
    let title = context.page_title.replace('_', " ");
    let (namespace, page) = split_title(&title);
    Some(match name.trim() {
        "!" => "|".into(),
        "=" => "=".into(),
        "CONTENTLANGUAGE" | "CONTENTLANG" | "PAGELANGUAGE" => context.site_language.into(),
        "FULLPAGENAME" => title.clone(),
        "PAGENAME" => page.into(),
        "NAMESPACE" => namespace.replace('_', " "),
        "SUBPAGENAME" => page.rsplit('/').next().unwrap_or(page).into(),
        "BASEPAGENAME" => page.rsplit_once('/').map_or(page, |(base, _)| base).into(),
        "ROOTPAGENAME" => page.split('/').next().unwrap_or(page).into(),
        "CURRENTYEAR" | "CURRENTMONTH" | "CURRENTMONTH1" | "CURRENTDAY" | "CURRENTDAY2"
        | "CURRENTHOUR" | "CURRENTTIME" | "CURRENTTIMESTAMP" => {
            time_value(name, context.reference_time)?
        }
        _ => return None,
    })
}

fn split_title(title: &str) -> (&str, &str) {
    if let Some((prefix, page)) = title.split_once(':') {
        if let Some(namespace) = crate::inline::namespace(prefix) {
            return (namespace, page);
        }
    }
    ("", title)
}

fn time_value(name: &str, time: &str) -> Option<String> {
    let b = time.as_bytes();
    if b.len() != 20
        || b[4] != b'-'
        || b[7] != b'-'
        || b[10] != b'T'
        || b[13] != b':'
        || b[16] != b':'
        || b[19] != b'Z'
        || b.iter()
            .enumerate()
            .any(|(i, c)| ![4, 7, 10, 13, 16, 19].contains(&i) && !c.is_ascii_digit())
    {
        return None;
    }
    let year = &time[0..4];
    let month = &time[5..7];
    let day = &time[8..10];
    let hour = &time[11..13];
    let minute = &time[14..16];
    let second = &time[17..19];
    let m: u32 = month.parse().ok()?;
    let d: u32 = day.parse().ok()?;
    let h: u32 = hour.parse().ok()?;
    let min: u32 = minute.parse().ok()?;
    let sec: u32 = second.parse().ok()?;
    if !(1..=12).contains(&m) || !(1..=31).contains(&d) || h > 23 || min > 59 || sec > 59 {
        return None;
    }
    Some(match name.trim() {
        "CURRENTYEAR" => year.into(),
        "CURRENTMONTH" => month.into(),
        "CURRENTMONTH1" => m.to_string(),
        "CURRENTDAY" => d.to_string(),
        "CURRENTDAY2" => day.into(),
        "CURRENTHOUR" => hour.into(),
        "CURRENTTIME" => format!("{hour}:{minute}"),
        "CURRENTTIMESTAMP" => format!("{year}{month}{day}{hour}{minute}{second}"),
        _ => return None,
    })
}

fn string_function(name: &str, args: &[String], language: &str) -> Option<String> {
    let first = args.first().map_or("", String::as_str);
    let primary_language = language.split('-').next().unwrap_or_default();
    let dotted_i = ["tr", "az"]
        .iter()
        .any(|code| primary_language.eq_ignore_ascii_case(code));
    let change_case = |ch: char, upper: bool| match (dotted_i, upper, ch) {
        (true, false, 'I') => "ı".into(),
        (true, false, 'İ') => "i".into(),
        (true, true, 'i') => "İ".into(),
        (_, true, ch) => ch.to_uppercase().to_string(),
        (_, false, ch) => ch.to_lowercase().to_string(),
    };
    Some(match name {
        "LC" | "UC" if dotted_i => first
            .chars()
            .map(|ch| change_case(ch, name == "UC"))
            .collect(),
        "LC" => first.to_lowercase(),
        "UC" => first.to_uppercase(),
        "LCFIRST" | "UCFIRST" => {
            let mut chars = first.chars();
            let Some(c) = chars.next() else {
                return Some(String::new());
            };
            let initial = change_case(c, name == "UCFIRST");
            initial + chars.as_str()
        }
        "PADLEFT" | "PADRIGHT" => {
            let length = args
                .get(1)
                .and_then(|s| s.trim().parse::<usize>().ok())
                .unwrap_or(0)
                .min(500);
            let padding = padding_text(args.get(2).map_or("0", String::as_str));
            let count = length.saturating_sub(first.chars().count());
            let pad: String = if padding.is_empty() {
                String::new()
            } else {
                padding.chars().cycle().take(count).collect()
            };
            if name == "PADLEFT" {
                pad + first
            } else {
                first.to_owned() + &pad
            }
        }
        _ => return None,
    })
}

fn padding_text(mut source: &str) -> String {
    let mut output = String::new();
    while let Some(pos) = source.find('<') {
        output.push_str(&source[..pos]);
        source = &source[pos..];
        if let Some((name, opening, false)) = crate::html_policy::tag_info(source) {
            if name == "nowiki" {
                let void = source[..opening]
                    .trim_end_matches('>')
                    .trim_end()
                    .ends_with('/');
                let end = if void {
                    opening
                } else {
                    crate::html_policy::raw_close(&source[opening..], &name)
                        .map_or(source.len(), |(_, end)| opening + end)
                };
                source = &source[end..];
                continue;
            }
        }
        output.push('<');
        source = &source[1..];
    }
    output.push_str(source);
    output
}

fn equal_values(left: &str, right: &str) -> bool {
    let left = html_escape::decode_html_entities(left);
    let right = html_escape::decode_html_entities(right);
    if let (Ok(a), Ok(b)) = (left.parse::<i64>(), right.parse::<i64>()) {
        return a == b;
    }
    if let (Ok(a), Ok(b)) = (left.parse::<f64>(), right.parse::<f64>()) {
        if a.is_finite() && b.is_finite() {
            return a == b;
        }
    }
    left == right
}

pub(crate) fn evaluate(
    name: &str,
    args: &[&str],
    context: &WikiContext<'_>,
    data: &dyn crate::WikiData,
    source: std::ops::Range<usize>,
    mut expand: impl FnMut(&str) -> Result<String, crate::ConversionError>,
) -> Result<Option<String>, crate::ConversionError> {
    let key = name.trim().to_ascii_uppercase();
    if args.is_empty() {
        if let Some(value) = variable(name, context) {
            let literal = matches!(
                name.trim(),
                "PAGENAME"
                    | "FULLPAGENAME"
                    | "NAMESPACE"
                    | "SUBPAGENAME"
                    | "BASEPAGENAME"
                    | "ROOTPAGENAME"
            );
            return Ok(Some(if literal {
                escape_wikitext(&value)
            } else {
                value
            }));
        }
    }
    let value = match key.as_str() {
        "#IF" => {
            let test = expand(args.first().copied().unwrap_or(""))?;
            expand(
                args.get(if test.trim().is_empty() { 2 } else { 1 })
                    .copied()
                    .unwrap_or(""),
            )?
            .trim()
            .to_owned()
        }
        "#IFEQ" => {
            let left = expand(args.first().copied().unwrap_or(""))?;
            let right = expand(args.get(1).copied().unwrap_or(""))?;
            expand(
                args.get(if equal_values(left.trim(), right.trim()) {
                    2
                } else {
                    3
                })
                .copied()
                .unwrap_or(""),
            )?
            .trim()
            .to_owned()
        }
        "#IFEXIST" => {
            let title = expand(args.first().copied().unwrap_or(""))?;
            let title = html_escape::decode_html_entities(title.trim());
            let title = title
                .split('#')
                .next()
                .unwrap_or("")
                .trim_start_matches(':');
            let exists = if crate::inline::internal_href(title).is_err() {
                false
            } else if let Some((_, filename)) = title
                .split_once(':')
                .filter(|(namespace, _)| namespace.eq_ignore_ascii_case("media"))
            {
                let file = crate::inline::normalize_title(&format!("File:{filename}"));
                data.file_info(&file)
                    .map_err(|error| {
                        crate::ConversionError::lookup(source, "file information", &file, error)
                    })?
                    .is_some()
            } else {
                let page = crate::inline::normalize_title(title);
                data.page_exists(&page)
                    .map_err(|error| {
                        crate::ConversionError::lookup(source, "page existence", &page, error)
                    })?
                    .unwrap_or(false)
            };
            expand(args.get(if exists { 1 } else { 2 }).copied().unwrap_or(""))?
                .trim()
                .to_owned()
        }
        "#SWITCH" => {
            let primary = expand(args.first().copied().unwrap_or(""))?;
            let mut matched = false;
            let mut default: Option<&str> = None;
            let mut default_found = false;
            let mut last = None;
            let mut selected = None;
            for arg in args.iter().skip(1) {
                let parts = crate::expansion::split_top(arg, '=');
                if parts.len() > 1 {
                    last = None;
                    if matched {
                        selected = Some(parts[1..].join("="));
                        break;
                    }
                    let case = expand(parts[0])?;
                    if equal_values(primary.trim(), case.trim()) {
                        selected = Some(parts[1..].join("="));
                        break;
                    }
                    if default_found || case.trim().eq_ignore_ascii_case("#default") {
                        default = Some(&arg[parts[0].len() + 1..]);
                        default_found = false;
                    }
                } else {
                    let case = expand(arg)?;
                    matched |= equal_values(primary.trim(), case.trim());
                    default_found |= case.trim().eq_ignore_ascii_case("#default");
                    last = Some(case.trim().to_owned());
                }
            }
            if let Some(raw) = selected {
                expand(&raw)?.trim().to_owned()
            } else if let Some(last) = last {
                last
            } else {
                expand(default.unwrap_or(""))?.trim().to_owned()
            }
        }
        "LC" | "UC" | "LCFIRST" | "UCFIRST" | "PADLEFT" | "PADRIGHT" => {
            let mut expanded = Vec::new();
            for arg in args {
                expanded.push(expand(arg)?.trim().to_owned());
            }
            string_function(&key, &expanded, context.site_language).unwrap_or_default()
        }
        _ => return Ok(None),
    };
    Ok(Some(value))
}

// A page title is data, so punctuation in it must not become wiki markup.
fn escape_wikitext(value: &str) -> String {
    let mut out = String::new();
    for ch in value.chars() {
        if matches!(
            ch,
            '&' | '\'' | '[' | ']' | '{' | '}' | '<' | '>' | '|' | '#' | '*' | ':' | ';' | '='
        ) {
            out.push_str(&format!("&#{};", ch as u32));
        } else {
            out.push(ch);
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    struct Data;
    impl crate::WikiData for Data {
        fn page_exists(&self, title: &str) -> Result<Option<bool>, crate::LookupError> {
            if title == "Failure" {
                Err(crate::LookupError::new("offline"))
            } else {
                Ok(Some(title == "Present"))
            }
        }
    }
    #[test]
    fn decisions_expand_only_selected_arguments() {
        let context = WikiContext::default();
        let mut seen = Vec::new();
        let result = evaluate(
            "#if",
            &["yes", "chosen", "unused"],
            &context,
            &Data,
            0..1,
            |s| {
                seen.push(s.to_owned());
                Ok(s.to_owned())
            },
        )
        .unwrap();
        assert_eq!(result.as_deref(), Some("chosen"));
        assert_eq!(seen, ["yes", "chosen"]);
        let result = evaluate(
            "#ifexist",
            &["Failure", "yes", "no"],
            &context,
            &Data,
            0..1,
            |s| Ok(s.to_owned()),
        );
        assert!(result.is_err());
    }
}
