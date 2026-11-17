use crate::WikiContext;

// This subset corresponds to context values available in WikiContext. Unknown names
// remain available for template lookup instead of being replaced with fabricated data.
fn variable(name: &str, context: &WikiContext<'_>, page_title: &str) -> Option<String> {
    let trimmed = name.trim();
    if let Some(value) = title_variable(trimmed, page_title) {
        return Some(escape_wikitext(&value));
    }
    Some(match trimmed {
        "!" => "|".into(),
        "=" => "=".into(),
        "CONTENTLANGUAGE" | "CONTENTLANG" | "PAGELANGUAGE" => context.site_language.into(),
        "CURRENTYEAR"
        | "CURRENTMONTH"
        | "CURRENTMONTH1"
        | "CURRENTDAY"
        | "CURRENTDAY2"
        | "CURRENTHOUR"
        | "CURRENTTIME"
        | "CURRENTTIMESTAMP"
        | "CURRENTDOW"
        | "CURRENTWEEK"
        | "CURRENTDAYNAME"
        | "CURRENTMONTHABBREV"
        | "CURRENTMONTHNAME"
        | "CURRENTMONTHNAMEGEN"
        | "LOCALYEAR"
        | "LOCALMONTH"
        | "LOCALMONTH1"
        | "LOCALMONTHNAME"
        | "LOCALMONTHNAMEGEN"
        | "LOCALMONTHABBREV"
        | "LOCALDAY"
        | "LOCALDAY2"
        | "LOCALDOW"
        | "LOCALDAYNAME"
        | "LOCALTIME"
        | "LOCALHOUR"
        | "LOCALWEEK"
        | "LOCALTIMESTAMP" => time_value(trimmed, context.reference_time)?,
        _ => return None,
    })
}

fn is_title_magic_word(name: &str) -> bool {
    matches!(
        name,
        "FULLPAGENAME"
            | "FULLPAGENAMEE"
            | "PAGENAME"
            | "PAGENAMEE"
            | "BASEPAGENAME"
            | "BASEPAGENAMEE"
            | "SUBPAGENAME"
            | "SUBPAGENAMEE"
            | "ROOTPAGENAME"
            | "ROOTPAGENAMEE"
            | "TALKPAGENAME"
            | "TALKPAGENAMEE"
            | "SUBJECTPAGENAME"
            | "SUBJECTPAGENAMEE"
            | "ARTICLEPAGENAME"
            | "ARTICLEPAGENAMEE"
            | "NAMESPACE"
            | "NAMESPACEE"
            | "NAMESPACENUMBER"
            | "TALKSPACE"
            | "TALKSPACEE"
            | "SUBJECTSPACE"
            | "SUBJECTSPACEE"
            | "ARTICLESPACE"
            | "ARTICLESPACEE"
    )
}

fn namespace_meta(canonical: &str) -> (&'static str, &'static str, &'static str) {
    match canonical {
        "" => ("0", "", "Talk"),
        "Talk" => ("1", "", "Talk"),
        "User" => ("2", "User", "User talk"),
        "User_talk" => ("3", "User", "User talk"),
        "Project" => ("4", "Project", "Project talk"),
        "Project_talk" => ("5", "Project", "Project talk"),
        "File" => ("6", "File", "File talk"),
        "File_talk" => ("7", "File", "File talk"),
        "MediaWiki" => ("8", "MediaWiki", "MediaWiki talk"),
        "MediaWiki_talk" => ("9", "MediaWiki", "MediaWiki talk"),
        "Template" => ("10", "Template", "Template talk"),
        "Template_talk" => ("11", "Template", "Template talk"),
        "Help" => ("12", "Help", "Help talk"),
        "Help_talk" => ("13", "Help", "Help talk"),
        "Category" => ("14", "Category", "Category talk"),
        "Category_talk" => ("15", "Category", "Category talk"),
        "Special" => ("-1", "Special", ""),
        "Media" => ("-2", "Media", ""),
        _ => ("0", "", "Talk"),
    }
}

fn namespace_by_id(id: i32) -> Option<&'static str> {
    Some(match id {
        -2 => "Media",
        -1 => "Special",
        0 => "",
        1 => "Talk",
        2 => "User",
        3 => "User talk",
        4 => "Project",
        5 => "Project talk",
        6 => "File",
        7 => "File talk",
        8 => "MediaWiki",
        9 => "MediaWiki talk",
        10 => "Template",
        11 => "Template talk",
        12 => "Help",
        13 => "Help talk",
        14 => "Category",
        15 => "Category talk",
        _ => return None,
    })
}

fn title_variable(key: &str, page_title: &str) -> Option<String> {
    let title = page_title.replace('_', " ");
    let (namespace, page) = split_title(&title);
    let (number, subject_space, talk_space) = namespace_meta(namespace);
    let full = if namespace.is_empty() {
        page.to_owned()
    } else {
        format!("{}:{page}", namespace.replace('_', " "))
    };
    let talk_page = if talk_space.is_empty() {
        String::new()
    } else {
        format!("{talk_space}:{page}")
    };
    let subject_page = if subject_space.is_empty() {
        page.to_owned()
    } else {
        format!("{subject_space}:{page}")
    };
    let base = page.rsplit_once('/').map_or(page, |(base, _)| base);
    let sub = page.rsplit('/').next().unwrap_or(page);
    let root = page.split('/').next().unwrap_or(page);
    let space = namespace.replace('_', " ");
    Some(match key {
        "FULLPAGENAME" => full,
        "FULLPAGENAMEE" => encode_wiki(&full),
        "PAGENAME" => page.to_owned(),
        "PAGENAMEE" => encode_wiki(page),
        "BASEPAGENAME" => base.to_owned(),
        "BASEPAGENAMEE" => encode_wiki(base),
        "SUBPAGENAME" => sub.to_owned(),
        "SUBPAGENAMEE" => encode_wiki(sub),
        "ROOTPAGENAME" => root.to_owned(),
        "ROOTPAGENAMEE" => encode_wiki(root),
        "TALKPAGENAME" => talk_page,
        "TALKPAGENAMEE" => encode_wiki(&talk_page),
        "SUBJECTPAGENAME" | "ARTICLEPAGENAME" => subject_page,
        "SUBJECTPAGENAMEE" | "ARTICLEPAGENAMEE" => encode_wiki(&subject_page),
        "NAMESPACE" => space,
        "NAMESPACEE" => encode_wiki(&space),
        "NAMESPACENUMBER" => number.to_owned(),
        "TALKSPACE" => talk_space.to_owned(),
        "TALKSPACEE" => encode_wiki(talk_space),
        "SUBJECTSPACE" | "ARTICLESPACE" => subject_space.to_owned(),
        "SUBJECTSPACEE" | "ARTICLESPACEE" => encode_wiki(subject_space),
        _ => return None,
    })
}

fn percent_encode(input: &str, space: Option<char>, unreserved: impl Fn(u8) -> bool) -> String {
    let mut out = String::with_capacity(input.len());
    for &b in input.as_bytes() {
        if b == b' ' {
            if let Some(replacement) = space {
                out.push(replacement);
                continue;
            }
        }
        if unreserved(b) {
            out.push(b as char);
        } else {
            out.push_str(&format!("%{b:02X}"));
        }
    }
    out
}

/// Percent-encodes a string using MediaWiki's `wfUrlencode` rules.
fn encode_wiki(input: &str) -> String {
    percent_encode(input, Some('_'), |b| {
        b.is_ascii_alphanumeric()
            || matches!(
                b,
                b'-' | b'_'
                    | b'.'
                    | b';'
                    | b'@'
                    | b'$'
                    | b'!'
                    | b'*'
                    | b'('
                    | b')'
                    | b','
                    | b'/'
                    | b'~'
                    | b':'
            )
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
    // Pinned Parsoid formats these through PHP DateTime, which uses English names
    // regardless of page language and currently treats month genitive as nominative.
    const DAYS: [&str; 7] = [
        "Sunday",
        "Monday",
        "Tuesday",
        "Wednesday",
        "Thursday",
        "Friday",
        "Saturday",
    ];
    const MONTHS: [&str; 12] = [
        "January",
        "February",
        "March",
        "April",
        "May",
        "June",
        "July",
        "August",
        "September",
        "October",
        "November",
        "December",
    ];
    const MONTHS_SHORT: [&str; 12] = [
        "Jan", "Feb", "Mar", "Apr", "May", "Jun", "Jul", "Aug", "Sep", "Oct", "Nov", "Dec",
    ];
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
    let calendar = || {
        let y: i32 = year.parse().ok()?;
        let leap = |year: i32| year % 4 == 0 && (year % 100 != 0 || year % 400 == 0);
        let month_days = [
            31,
            if leap(y) { 29 } else { 28 },
            31,
            30,
            31,
            30,
            31,
            31,
            30,
            31,
            30,
            31,
        ];
        if d > month_days[(m - 1) as usize] {
            return None;
        }
        let ordinal = month_days[..(m - 1) as usize].iter().sum::<u32>() + d;
        let weekday = |year: i32, day: u32| {
            let previous = year - 1;
            (previous * 365 + previous.div_euclid(4) - previous.div_euclid(100)
                + previous.div_euclid(400)
                + day as i32)
                .rem_euclid(7) as u32
        };
        let dow = weekday(y, ordinal);
        let iso_day = if dow == 0 { 7 } else { dow };
        let weeks = |year| {
            let jan1 = weekday(year, 1);
            if jan1 == 4 || (jan1 == 3 && leap(year)) {
                53
            } else {
                52
            }
        };
        let week = (ordinal as i32 - iso_day as i32 + 10) / 7;
        let week = if week == 0 {
            weeks(y - 1)
        } else if week > weeks(y) {
            1
        } else {
            week
        };
        Some((dow, week))
    };
    Some(match name.trim() {
        "CURRENTYEAR" | "LOCALYEAR" => year.into(),
        "CURRENTMONTH" | "LOCALMONTH" => month.into(),
        "CURRENTMONTH1" | "LOCALMONTH1" => m.to_string(),
        "CURRENTDAY" | "LOCALDAY" => d.to_string(),
        "CURRENTDAY2" | "LOCALDAY2" => day.into(),
        "CURRENTHOUR" | "LOCALHOUR" => hour.into(),
        "CURRENTTIME" | "LOCALTIME" => format!("{hour}:{minute}"),
        "CURRENTTIMESTAMP" | "LOCALTIMESTAMP" => format!("{year}{month}{day}{hour}{minute}{second}"),
        "CURRENTDOW" | "LOCALDOW" => calendar()?.0.to_string(),
        "CURRENTWEEK" | "LOCALWEEK" => calendar()?.1.to_string(),
        "CURRENTDAYNAME" | "LOCALDAYNAME" => DAYS[calendar()?.0 as usize].into(),
        "CURRENTMONTHABBREV" | "LOCALMONTHABBREV" => {
            calendar()?;
            MONTHS_SHORT[(m - 1) as usize].into()
        }
        "CURRENTMONTHNAME" | "CURRENTMONTHNAMEGEN" | "LOCALMONTHNAME" | "LOCALMONTHNAMEGEN" => {
            calendar()?;
            MONTHS[(m - 1) as usize].into()
        }
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
        "LC" | "UC" | "LCFIRST" | "UCFIRST" => {
            let first = strip_tags(first, &["nowiki"], false, false);
            match name {
                "LC" | "UC" if dotted_i => first
                    .chars()
                    .map(|ch| change_case(ch, name == "UC"))
                    .collect(),
                "LC" => first.to_lowercase(),
                "UC" => first.to_uppercase(),
                _ => {
                    let mut chars = first.chars();
                    let Some(c) = chars.next() else {
                        return Some(String::new());
                    };
                    let initial = change_case(c, name == "UCFIRST");
                    initial + chars.as_str()
                }
            }
        }
        "PADLEFT" | "PADRIGHT" => {
            let length = args
                .get(1)
                .and_then(|s| s.trim().parse::<usize>().ok())
                .unwrap_or(0)
                .min(500);
            let padding = kill_markers(args.get(2).map_or("0", String::as_str));
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

fn strip_tags(mut source: &str, tags: &[&str], paired: bool, keep_content: bool) -> String {
    let mut output = String::new();
    while let Some(pos) = source.find('<') {
        output.push_str(&source[..pos]);
        source = &source[pos..];
        if let Some((name, opening, false)) = crate::html_policy::tag_info(source) {
            if tags.contains(&name.as_str()) {
                let void = source[..opening]
                    .trim_end_matches('>')
                    .trim_end()
                    .ends_with('/');
                if void {
                    source = &source[opening..];
                    continue;
                }
                if paired {
                    match crate::html_policy::raw_close(&source[opening..], &name) {
                        Some((body_end, close_end)) => {
                            if keep_content {
                                output.push_str(&source[opening..opening + body_end]);
                            }
                            source = &source[opening + close_end..];
                        }
                        None => {
                            if keep_content {
                                output.push_str(&source[opening..]);
                            }
                            return output;
                        }
                    }
                    continue;
                }
            }
        }
        output.push('<');
        source = &source[1..];
    }
    output.push_str(source);
    output
}

fn kill_markers(source: &str) -> String {
    strip_tags(source, &["nowiki", "pre"], true, false)
}

fn strip_wikilinks(mut input: &str) -> String {
    let mut out = String::with_capacity(input.len());
    while let Some(start) = input.find("[[") {
        out.push_str(&input[..start]);
        let rest = &input[start + 2..];
        if let Some(end) = rest.find("]]") {
            let inside = &rest[..end];
            let label = inside.rsplit_once('|').map_or(inside, |(_, label)| label);
            out.push_str(label);
            input = &rest[end + 2..];
        } else {
            out.push_str("[[");
            input = rest;
        }
    }
    out.push_str(input);
    out
}

fn strip_quote_runs(mut input: &str) -> String {
    let mut out = String::with_capacity(input.len());
    while !input.is_empty() {
        if input.starts_with('\'') {
            let count = input.bytes().take_while(|&b| b == b'\'').count();
            if count == 1 {
                out.push('\'');
            }
            input = &input[count..];
        } else {
            let next = input.find('\'').unwrap_or(input.len());
            out.push_str(&input[..next]);
            input = &input[next..];
        }
    }
    out
}

fn escape_percent_hex(input: &str) -> String {
    let bytes = input.as_bytes();
    let mut out = String::with_capacity(input.len());
    let mut i = 0;
    while i < bytes.len() {
        if bytes[i] == b'%'
            && bytes.get(i + 1).is_some_and(u8::is_ascii_hexdigit)
            && bytes.get(i + 2).is_some_and(u8::is_ascii_hexdigit)
        {
            out.push_str("%25");
            out.push(bytes[i + 1] as char);
            out.push(bytes[i + 2] as char);
            i += 3;
        } else {
            let ch = input[i..].chars().next().unwrap();
            out.push(ch);
            i += ch.len_utf8();
        }
    }
    out
}

fn parse_tag_attributes(
    args: &[&str],
    base_size: usize,
    strip_quotes: bool,
    source: &std::ops::Range<usize>,
    max_output_bytes: usize,
    expand: &mut impl FnMut(&str) -> Result<String, crate::ConversionError>,
) -> Result<String, crate::ConversionError> {
    let check_size = |size: usize| {
        if size > max_output_bytes {
            Err(crate::ConversionError::expansion_limit(
                source.clone(),
                "template expansion output byte limit exceeded",
            ))
        } else {
            Ok(())
        }
    };
    check_size(base_size)?;
    let mut attributes = String::new();
    for arg in args {
        let parts = crate::expansion::split_top(arg, '=');
        if parts.len() < 2 {
            continue;
        }
        let name = remove_nowiki(&expand(parts[0])?, false);
        let name = name.trim();
        if name.is_empty()
            || !name
                .chars()
                .all(|c| c.is_ascii_alphanumeric() || matches!(c, '-' | '_' | ':'))
        {
            continue;
        }
        check_size(
            base_size
                .saturating_add(attributes.len())
                .saturating_add(name.len())
                .saturating_add(4),
        )?;
        let value = remove_nowiki(&expand(&parts[1..].join("="))?, false);
        let trimmed = value.trim();
        let unquoted = if strip_quotes
            && trimmed.len() >= 2
            && ((trimmed.starts_with('"') && trimmed.ends_with('"'))
                || (trimmed.starts_with('\'') && trimmed.ends_with('\'')))
        {
            &trimmed[1..trimmed.len() - 1]
        } else {
            trimmed
        };
        let value = html_escape::decode_html_entities(unquoted);
        let value = html_escape::encode_double_quoted_attribute(&value);
        check_size(
            base_size
                .saturating_add(attributes.len())
                .saturating_add(name.len())
                .saturating_add(value.len())
                .saturating_add(4),
        )?;
        attributes.push(' ');
        attributes.push_str(name);
        attributes.push_str("=\"");
        attributes.push_str(&value);
        attributes.push('"');
    }
    Ok(attributes)
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
    has_colon: bool,
    context: &WikiContext<'_>,
    data: &dyn crate::WikiData,
    source: std::ops::Range<usize>,
    max_output_bytes: usize,
    mut expand: impl FnMut(&str) -> Result<String, crate::ConversionError>,
) -> Result<Option<String>, crate::ConversionError> {
    let key = name.trim().to_ascii_uppercase();
    if args.is_empty() {
        if let Some(value) = variable(name, context, context.page_title) {
            return Ok(Some(value));
        }
    } else if has_colon && args.len() == 1 && is_title_magic_word(&key) {
        let expanded = expand(args[0])?;
        let decoded = html_escape::decode_html_entities(expanded.trim());
        let title = decoded
            .split('#')
            .next()
            .unwrap_or("")
            .trim_start_matches(':');
        let title = if crate::inline::internal_href(title).is_err() {
            String::new()
        } else {
            crate::inline::normalize_title(title)
        };
        if title.is_empty() {
            return Ok(Some(String::new()));
        }
        if let Some(value) = title_variable(&key, &title) {
            return Ok(Some(escape_wikitext(&value)));
        }
    }
    let value = match key.as_str() {
        "NS" | "NSE" => {
            if !has_colon {
                return Ok(None);
            }
            let raw = expand(args.first().copied().unwrap_or(""))?;
            let arg = raw.trim();
            let resolved = if arg.is_empty() {
                String::new()
            } else if let Some(ns) = arg.parse::<i32>().ok().and_then(namespace_by_id) {
                ns.to_owned()
            } else if let Some(ns) = crate::inline::namespace(arg) {
                ns.replace('_', " ")
            } else {
                return Ok(None);
            };
            if key == "NSE" {
                encode_wiki(&resolved)
            } else {
                resolved
            }
        }
        "URLENCODE" => {
            if !has_colon {
                return Ok(None);
            }
            let text = kill_markers(expand(args.first().copied().unwrap_or(""))?.trim());
            let style = match args.get(1) {
                Some(raw) => expand(raw)?.trim().to_ascii_uppercase(),
                None => String::new(),
            };
            match style.as_str() {
                "WIKI" => encode_wiki(&text),
                "PATH" => percent_encode(&text, None, |b| {
                    b.is_ascii_alphanumeric() || matches!(b, b'-' | b'_' | b'.' | b'~')
                }),
                _ => percent_encode(&text, Some('+'), |b| {
                    b.is_ascii_alphanumeric() || matches!(b, b'-' | b'_' | b'.')
                }),
            }
        }
        "ANCHORENCODE" => {
            if !has_colon {
                return Ok(None);
            }
            let stripped = kill_markers(&expand(args.first().copied().unwrap_or(""))?);
            let unlinked = strip_wikilinks(&stripped);
            let no_quotes = strip_quote_runs(&unlinked);
            let normalized = no_quotes
                .split(|c: char| c.is_ascii_whitespace() || c == '_')
                .filter(|s| !s.is_empty())
                .collect::<Vec<_>>()
                .join(" ");
            let decoded = html_escape::decode_html_entities(&normalized);
            let underscored = decoded.replace(' ', "_");
            let percent_escaped = escape_percent_hex(&underscored);
            escape_wikitext(&percent_escaped)
        }
        "#TAG" => {
            let tag = remove_nowiki(&expand(args.first().copied().unwrap_or(""))?, false);
            let tag = tag.trim().to_ascii_lowercase();
            if tag.is_empty()
                || !tag
                    .chars()
                    .all(|c| c.is_ascii_alphanumeric() || matches!(c, '-' | '_' | ':'))
                || matches!(
                    tag.as_str(),
                    "indicator"
                        | "ref"
                        | "references"
                        | "gallery"
                        | "poem"
                        | "syntaxhighlight"
                        | "source"
                        | "math"
                        | "score"
                        | "templatestyles"
                        | "section"
                        | "spantag"
                        | "divtag"
                        | "tag"
                        | "statictag"
                        | "categorytree"
                        | "charinsert"
                        | "hiero"
                        | "imagemap"
                        | "inputbox"
                        | "mapframe"
                        | "maplink"
                        | "phonos"
                        | "quiz"
                        | "rss"
                        | "timeline"
                )
            {
                return Ok(None);
            }
            if tag == "nowiki" {
                let content = remove_nowiki(&expand(args.get(1).copied().unwrap_or(""))?, true);
                // Emit literal characters as entities so embedded closing tags cannot
                // escape the literal and affect the rest of the document.
                html_escape::decode_html_entities(&content)
                    .chars()
                    .map(|c| format!("&#{};", c as u32))
                    .collect()
            } else if tag == "pre" {
                let content = remove_nowiki(&expand(args.get(1).copied().unwrap_or(""))?, true);
                let content = html_escape::decode_html_entities(&content);
                let content = html_escape::encode_text(&content);
                let base_size = "<pre></pre>".len().saturating_add(content.len());
                let attributes = parse_tag_attributes(
                    args.get(2..).unwrap_or(&[]),
                    base_size,
                    false,
                    &source,
                    max_output_bytes,
                    &mut expand,
                )?;
                format!("<pre{attributes}>{content}</pre>")
            } else {
                let inner = args.get(1).copied().map(&mut expand).transpose()?;
                let base_size = if let Some(ref body) = inner {
                    tag.len()
                        .saturating_mul(2)
                        .saturating_add(5)
                        .saturating_add(body.len())
                } else {
                    tag.len().saturating_add(3)
                };
                let attributes = parse_tag_attributes(
                    args.get(2..).unwrap_or(&[]),
                    base_size,
                    true,
                    &source,
                    max_output_bytes,
                    &mut expand,
                )?;
                if let Some(inner) = inner {
                    format!("<{tag}{attributes}>{inner}</{tag}>")
                } else {
                    format!("<{tag}{attributes}/>")
                }
            }
        }
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

// Literal nowiki segments act as strip markers during #tag expansion: their
// contents survive in tag bodies but disappear from names and attributes.
fn remove_nowiki(input: &str, keep_content: bool) -> String {
    strip_tags(input, &["nowiki"], true, keep_content)
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
            true,
            &context,
            &Data,
            0..1,
            usize::MAX,
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
            true,
            &context,
            &Data,
            0..1,
            usize::MAX,
            |s| Ok(s.to_owned()),
        );
        assert!(result.is_err());
    }
}
