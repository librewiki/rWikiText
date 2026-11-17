use crate::{ConversionError, Diagnostic, ExpansionLimits, WikiContext, WikiData};
use std::collections::HashMap;
use std::ops::Range;
use std::rc::Rc;

#[derive(Debug, Clone)]
pub(crate) struct Argument {
    pub key: String,
    pub key_source: Option<String>,
    pub value: String,
    pub named: bool,
}
#[derive(Debug, Clone)]
pub(crate) struct Invocation {
    pub expanded_range: Range<usize>,
    pub source_range: Range<usize>,
    pub target: String,
    pub title: String,
    pub arguments: Vec<Argument>,
    pub function: Option<String>,
}
struct Origin {
    output: Range<usize>,
    source: Range<usize>,
    identity: bool,
}
pub(crate) struct Expansion {
    pub text: String,
    pub invocations: Vec<Invocation>,
    pub missing_links: Vec<(Range<usize>, String)>,
    pub diagnostics: Vec<Diagnostic>,
    origins: Vec<Origin>,
    input_len: usize,
    output_size: usize,
}
impl Expansion {
    pub fn original_span(&self, span: Range<usize>) -> Range<usize> {
        let (mut start, mut end) = (usize::MAX, 0);
        let first = self
            .origins
            .partition_point(|origin| origin.output.end <= span.start);
        for o in self.origins[first..]
            .iter()
            .take_while(|origin| origin.output.start < span.end)
        {
            let (a, b) = if o.identity {
                (
                    o.source.start + span.start.saturating_sub(o.output.start),
                    o.source.start + span.end.min(o.output.end) - o.output.start,
                )
            } else {
                (o.source.start, o.source.end)
            };
            start = start.min(a);
            end = end.max(b);
        }
        if start == usize::MAX {
            let p = self
                .origins
                .get(first)
                .map_or(self.input_len, |o| o.source.start);
            p..p
        } else {
            start..end
        }
    }
}
#[derive(Clone)]
struct Parameter {
    text: String,
    parent: Rc<Params>,
    named: bool,
}
type Params = HashMap<String, Parameter>;
struct EmptyData;
impl WikiData for EmptyData {}
static EMPTY_DATA: EmptyData = EmptyData;

struct Engine<'a, 'ctx> {
    data: Option<&'a dyn WikiData>,
    context: &'a WikiContext<'ctx>,
    limits: ExpansionLimits,
    count: usize,
    stack_depth: usize,
    nonce: u128,
    seen_inputs: Vec<String>,
    missing_links: HashMap<String, String>,
    diagnostics: Vec<Diagnostic>,
}

pub(crate) fn expand(
    input: &str,
    context: &WikiContext<'_>,
    data: Option<&dyn WikiData>,
    limits: ExpansionLimits,
) -> Result<Expansion, ConversionError> {
    let mut e = Engine {
        data,
        context,
        limits,
        count: 0,
        stack_depth: 0,
        nonce: std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_nanos(),
        missing_links: HashMap::new(),
        diagnostics: Vec::new(),
        seen_inputs: vec![input.to_owned()],
    };
    let mut out = Expansion {
        text: String::new(),
        invocations: Vec::new(),
        missing_links: Vec::new(),
        diagnostics: Vec::new(),
        origins: Vec::new(),
        input_len: input.len(),
        output_size: 0,
    };
    e.top(input, &mut out)?;
    let mut replacements = Vec::new();
    let mut expanded_text = String::with_capacity(out.output_size);
    let mut previous = 0;
    for (range, title) in e.link_markers(&out.text) {
        expanded_text.push_str(&out.text[previous..range.start]);
        let start = expanded_text.len();
        expanded_text.push_str(title);
        let output = start..expanded_text.len();
        previous = range.end;
        replacements.push((range, output.clone()));
        out.missing_links.push((output, title.to_owned()));
    }
    expanded_text.push_str(&out.text[previous..]);
    let map = |pos: usize| -> usize {
        let index = replacements.partition_point(|(input, _)| input.start <= pos);
        index.checked_sub(1).map_or(pos, |i| {
            let (input, output) = &replacements[i];
            if pos < input.end {
                output.start
            } else {
                output.end + pos - input.end
            }
        })
    };
    for call in &mut out.invocations {
        call.expanded_range = map(call.expanded_range.start)..map(call.expanded_range.end);
    }
    for origin in &mut out.origins {
        origin.output = map(origin.output.start)..map(origin.output.end);
    }
    out.text = expanded_text;
    out.diagnostics.extend(e.diagnostics);
    Ok(out)
}
impl Engine<'_, '_> {
    fn link_markers<'a>(
        &'a self,
        text: &'a str,
    ) -> impl Iterator<Item = (Range<usize>, &'a str)> + 'a {
        text.match_indices('\u{e000}').filter_map(|(start, _)| {
            let range = start..start + 42;
            let title = self.missing_links.get(text.get(range.clone())?)?;
            Some((range, title.as_str()))
        })
    }

    fn visible_len(&self, text: &str) -> usize {
        self.link_markers(text)
            .fold(text.len(), |len, (marker, title)| {
                len - marker.len() + title.len()
            })
    }
    fn append(
        &self,
        out: &mut Expansion,
        text: &str,
        source: Range<usize>,
        identity: bool,
    ) -> Result<(), ConversionError> {
        let size = self.visible_len(text);
        if out.output_size.saturating_add(size) > self.limits.max_output_bytes {
            return Err(ConversionError::expansion_limit(
                source,
                "template expansion output byte limit exceeded",
            ));
        }
        let start = out.text.len();
        out.output_size += size;
        out.text.push_str(text);
        if !text.is_empty() {
            if let Some(last) = out.origins.last_mut() {
                if identity
                    && last.identity
                    && last.source.end == source.start
                    && last.output.end == start
                {
                    last.source.end = source.end;
                    last.output.end = out.text.len();
                    return Ok(());
                }
            }
            out.origins.push(Origin {
                output: start..out.text.len(),
                source,
                identity,
            });
        }
        Ok(())
    }
    fn top(&mut self, input: &str, out: &mut Expansion) -> Result<(), ConversionError> {
        let no_params = Rc::new(Params::new());
        let mut pos = 0;
        let mut lexical = LexicalContext::default();
        while pos < input.len() {
            let tail = &input[pos..];
            if let Some(n) = literal_len(tail) {
                self.append(out, &tail[..n], pos..pos + n, true)?;
                lexical.bracket = None;
                pos += n;
                continue;
            }
            if let Some(n) = self_closed_include(tail) {
                pos += n;
                continue;
            }
            if starts_ci(tail, "<includeonly>") {
                pos += "<includeonly>".len();
                pos = find_ci(&input[pos..], "</includeonly>")
                    .map_or(input.len(), |n| pos + n + "</includeonly>".len());
                continue;
            }
            if let Some(tag) = [
                "<noinclude>",
                "</noinclude>",
                "<onlyinclude>",
                "</onlyinclude>",
            ]
            .into_iter()
            .find(|t| starts_ci(tail, t))
            {
                pos += tag.len();
                continue;
            }
            if tail.starts_with("{{") {
                if let Some(n) = construct_end(tail) {
                    let source = pos..pos + n;
                    let (value, call) =
                        self.construct(&tail[..n], &no_params, 0, source.clone())?;
                    if needs_implicit_newline(&out.text, &value) && lexical.allows_block() {
                        self.append(out, "\n", source.clone(), false)?;
                    }
                    let start = out.text.len();
                    self.append(out, &value, source.clone(), false)?;
                    lexical.advance(&value);
                    if let Some((target, title, arguments, function)) = call {
                        out.invocations.push(Invocation {
                            expanded_range: start..out.text.len(),
                            source_range: source,
                            target,
                            title,
                            arguments,
                            function,
                        });
                    }
                    pos += n;
                    continue;
                }
                self.append(out, tail, pos..input.len(), true)?;
                break;
            }
            let n = tail.chars().next().unwrap().len_utf8();
            self.append(out, &tail[..n], pos..pos + n, true)?;
            lexical.advance(&tail[..n]);
            pos += n;
        }
        Ok(())
    }
    fn text(
        &mut self,
        input: &str,
        params: &Rc<Params>,
        depth: usize,
        source: Range<usize>,
    ) -> Result<String, ConversionError> {
        let mut out = String::new();
        let mut output_size = 0usize;
        let mut pos = 0;
        let mut lexical = LexicalContext::default();
        while pos < input.len() {
            let tail = &input[pos..];
            let (mut part, n) = if let Some(n) = literal_len(tail) {
                (tail[..n].to_owned(), n)
            } else if tail.starts_with("{{") {
                if let Some(n) = construct_end(tail) {
                    (
                        self.construct(&tail[..n], params, depth, source.clone())?.0,
                        n,
                    )
                } else {
                    (tail.to_owned(), tail.len())
                }
            } else {
                let n = tail.chars().next().unwrap().len_utf8();
                (tail[..n].to_owned(), n)
            };
            if tail.starts_with("{{")
                && needs_implicit_newline(&out, &part)
                && lexical.allows_block()
            {
                part.insert(0, '\n');
            }
            let size = self.visible_len(&part);
            if output_size.saturating_add(size) > self.limits.max_output_bytes {
                return Err(ConversionError::expansion_limit(
                    source,
                    "template expansion output byte limit exceeded",
                ));
            }
            output_size += size;
            out.push_str(&part);
            if literal_len(tail).is_none() {
                lexical.advance(&part);
            } else {
                lexical.bracket = None;
            }
            pos += n;
        }
        Ok(out)
    }
    fn construct(
        &mut self,
        input: &str,
        params: &Rc<Params>,
        depth: usize,
        source: Range<usize>,
    ) -> Result<
        (
            String,
            Option<(String, String, Vec<Argument>, Option<String>)>,
        ),
        ConversionError,
    > {
        // Guard recursion independently of the caller's configured template depth.
        // Parameters and parser functions also recurse and must not exhaust the stack.
        const MAX_STACK_DEPTH: usize = 128;
        if self.stack_depth >= MAX_STACK_DEPTH {
            return Err(ConversionError::expansion_limit(
                source,
                "wikitext nesting limit exceeded",
            ));
        }
        if self.count >= self.limits.max_expansions {
            return Err(ConversionError::expansion_limit(
                source,
                "template expansion count limit exceeded",
            ));
        }
        self.count += 1;
        self.stack_depth += 1;
        let result = self.construct_inner(input, params, depth, source);
        self.stack_depth -= 1;
        result
    }
    fn construct_inner(
        &mut self,
        input: &str,
        params: &Rc<Params>,
        depth: usize,
        source: Range<usize>,
    ) -> Result<
        (
            String,
            Option<(String, String, Vec<Argument>, Option<String>)>,
        ),
        ConversionError,
    > {
        let parameter = input.starts_with("{{{") && !input.starts_with("{{{{");
        let width = if parameter { 3 } else { 2 };
        let parts = split_top(&input[width..input.len() - width], '|');
        if parameter {
            let key = self.text(parts[0], params, depth, source.clone())?;
            let key = strip_comments(&key);
            let value = if let Some(v) = params.get(key.trim()) {
                let text = self.text(&v.text, &v.parent, depth, source.clone())?;
                if v.named {
                    text.trim().to_owned()
                } else {
                    text
                }
            } else if parts.len() > 1 {
                self.text(&parts[1..].join("|"), params, depth, source)?
            } else {
                crate::escape_wikitext(input)
            };
            return Ok((value, None));
        }
        if depth >= self.limits.max_depth {
            return Err(ConversionError::expansion_limit(
                source,
                "template expansion depth limit exceeded",
            ));
        }
        let head = split_top(parts[0], ':');
        let func = strip_comments(&self.text(head[0], params, depth + 1, source.clone())?);
        let arg = (head.len() > 1).then(|| head[1..].join(":"));
        if func.trim().eq_ignore_ascii_case("DISPLAYTITLE") {
            if let Some(a) = arg.as_deref() {
                let val = self.text(a, params, depth + 1, source.clone())?;
                return Ok((format!("{{{{DISPLAYTITLE:{val}}}}}"), None));
            }
        }
        let mut args = Vec::new();
        if let Some(a) = arg.as_deref() {
            args.push(a);
        }
        args.extend_from_slice(&parts[1..]);
        let data = self.data.unwrap_or(&EMPTY_DATA);
        let context = self.context;
        let max_output_bytes = self.limits.max_output_bytes;
        let literal_tag = func.trim().eq_ignore_ascii_case("#tag");
        if let Some(value) = crate::functions::evaluate(
            func.trim(),
            &args,
            arg.is_some(),
            context,
            data,
            source.clone(),
            max_output_bytes,
            |s| {
                let text = self.text(s, params, depth + 1, source.clone())?;
                if !literal_tag {
                    return Ok(text);
                }
                let markers: Vec<_> = self
                    .link_markers(&text)
                    .map(|(range, title)| (range, title.to_owned()))
                    .collect();
                if markers.is_empty() {
                    return Ok(text);
                }
                self.diagnostics.push(Diagnostic::loss(
                    source.clone(),
                    "cannot preserve a missing-template link inside #tag literal content",
                ));
                let mut resolved = String::new();
                let mut previous = 0;
                for (range, title) in markers {
                    resolved.push_str(&text[previous..range.start]);
                    resolved.push_str("[[:");
                    resolved.push_str(&title);
                    resolved.push_str("]]");
                    previous = range.end;
                }
                resolved.push_str(&text[previous..]);
                Ok(resolved)
            },
        )? {
            let arguments = parts[1..]
                .iter()
                .enumerate()
                .map(|(i, value)| Argument {
                    key: (i + 1).to_string(),
                    key_source: None,
                    value: (*value).to_owned(),
                    named: false,
                })
                .collect();
            return Ok((
                value,
                Some((
                    parts[0].to_owned(),
                    String::new(),
                    arguments,
                    Some(func.trim().trim_start_matches('#').to_ascii_lowercase()),
                )),
            ));
        }
        if arg.is_some()
            && func.trim().strip_prefix('#').is_some_and(|name| {
                !name.is_empty() && name.chars().all(|c| c.is_ascii_alphanumeric() || c == '_')
            })
        {
            self.diagnostics.push(Diagnostic::loss(
                source.clone(),
                format!("unsupported parser function {}", func.trim()),
            ));
        }
        let name = if let Some(a) = arg {
            let suffix = self.text(&a, params, depth + 1, source.clone())?;
            if self
                .visible_len(&func)
                .saturating_add(1)
                .saturating_add(self.visible_len(&suffix))
                > self.limits.max_output_bytes
            {
                return Err(ConversionError::expansion_limit(
                    source,
                    "template expansion output byte limit exceeded",
                ));
            }
            format!("{func}:{suffix}")
        } else {
            func
        };
        let Some(title) = canonical_template_target(&name) else {
            const OPEN: &str = "&#123;&#123;";
            const CLOSE: &str = "&#125;&#125;";
            let mut size = OPEN
                .len()
                .saturating_add(self.visible_len(&name))
                .saturating_add(CLOSE.len());
            if size > self.limits.max_output_bytes {
                return Err(ConversionError::expansion_limit(
                    source,
                    "template expansion output byte limit exceeded",
                ));
            }
            let mut body = String::from(OPEN);
            body.push_str(&name);
            for part in &parts[1..] {
                size = size.saturating_add(1);
                if size > self.limits.max_output_bytes {
                    return Err(ConversionError::expansion_limit(
                        source,
                        "template expansion output byte limit exceeded",
                    ));
                }
                let value = self.text(part, params, depth + 1, source.clone())?;
                size = size.saturating_add(self.visible_len(&value));
                if size > self.limits.max_output_bytes {
                    return Err(ConversionError::expansion_limit(
                        source,
                        "template expansion output byte limit exceeded",
                    ));
                }
                body.push('|');
                body.push_str(&value);
            }
            body.push_str(CLOSE);
            return Ok((body, None));
        };
        if self.data.is_none() {
            self.diagnostics.push(Diagnostic::loss(
                source,
                "unsupported wikitext transclusion omitted",
            ));
            return Ok((String::new(), None));
        }
        let mut arguments = Vec::new();
        let mut values = Params::new();
        let mut position = 1;
        for part in &parts[1..] {
            let equal = split_top(part, '=');
            let (key, key_source, value, named) = if equal.len() > 1 {
                let expanded = self.text(equal[0], params, depth + 1, source.clone())?;
                let key = strip_comments(&expanded).trim().to_owned();
                let raw = equal[0].trim();
                let key_source =
                    (raw != key && strip_comments(raw).trim() == key).then(|| raw.to_owned());
                (key, key_source, equal[1..].join("="), true)
            } else {
                let key = position.to_string();
                position += 1;
                (key, None, (*part).to_owned(), false)
            };
            values.insert(
                key.clone(),
                Parameter {
                    text: value.clone(),
                    parent: Rc::clone(params),
                    named,
                },
            );
            arguments.push(Argument {
                key,
                key_source,
                value: if named {
                    value.trim().to_owned()
                } else {
                    value
                },
                named,
            });
        }
        let mut requested = title.split('#').next().unwrap_or(&title).to_owned();
        let mut depth = depth;
        let body = loop {
            match data.template_text(&requested) {
                Ok(Some(body)) => {
                    let Some((target, _)) = crate::metadata::redirect(&body) else {
                        break body;
                    };
                    depth += 1;
                    if depth >= self.limits.max_depth {
                        return Err(ConversionError::expansion_limit(
                            source,
                            "template expansion depth limit exceeded",
                        ));
                    }
                    if self.count >= self.limits.max_expansions {
                        return Err(ConversionError::expansion_limit(
                            source,
                            "template expansion count limit exceeded",
                        ));
                    }
                    self.count += 1;
                    requested = crate::inline::normalize_title(
                        target
                            .trim_start_matches(':')
                            .split('#')
                            .next()
                            .unwrap_or(""),
                    );
                }
                Ok(None) => {
                    let mut candidate = self.nonce;
                    let marker = loop {
                        let marker = marker(candidate, self.missing_links.len());
                        if !self.missing_links.contains_key(&marker)
                            && !self.seen_inputs.iter().any(|input| input.contains(&marker))
                        {
                            break marker;
                        }
                        candidate = candidate.wrapping_add(1);
                    };
                    self.missing_links.insert(marker.clone(), requested);
                    return Ok((marker, Some((parts[0].to_owned(), title, arguments, None))));
                }
                Err(error) => {
                    return Err(ConversionError::lookup(
                        source, "template", &requested, error,
                    ))
                }
            }
        };
        self.seen_inputs.push(body.clone());
        let value = self.text(&inclusion(&body), &Rc::new(values), depth + 1, source);
        self.seen_inputs.pop();
        Ok((value?, Some((parts[0].to_owned(), title, arguments, None))))
    }
}

// MediaWiki's T2529 rule gives expansions beginning with list or table
// syntax a line boundary, even when the invocation is mid-line.
#[derive(Default)]
struct LexicalContext {
    links: usize,
    bracket: Option<char>,
    tag: bool,
    tag_pending: bool,
    quote: Option<char>,
    external: bool,
    external_probe: Option<String>,
}

impl LexicalContext {
    fn allows_block(&self) -> bool {
        self.links == 0 && !self.external && !self.tag && !self.tag_pending
    }

    fn advance(&mut self, mut text: &str) {
        while !text.is_empty() {
            if !self.tag {
                if let Some(n) = literal_len(text) {
                    self.bracket = None;
                    text = &text[n..];
                    continue;
                }
            }
            let ch = text.chars().next().unwrap();
            text = &text[ch.len_utf8()..];
            if self.tag_pending {
                self.tag_pending = false;
                if ch.is_ascii_alphabetic() || matches!(ch, '/' | '!' | '?') {
                    self.tag = true;
                    continue;
                }
            }
            if self.tag {
                if let Some(quote) = self.quote {
                    if ch == quote {
                        self.quote = None;
                    }
                } else if matches!(ch, '\'' | '"') {
                    self.quote = Some(ch);
                } else if ch == '>' {
                    self.tag = false;
                }
                continue;
            }
            if self.external {
                if ch == '<' {
                    self.tag_pending = true;
                } else if ch == ']' {
                    self.external = false;
                }
                continue;
            }
            if self.bracket == Some('[') && ch != '[' && self.links == 0 {
                self.external_probe = Some(String::new());
            }
            if let Some(probe) = &mut self.external_probe {
                probe.push(ch.to_ascii_lowercase());
                if crate::html_policy::PROTOCOLS.contains(&probe.as_str()) {
                    self.external = true;
                    self.external_probe = None;
                    self.bracket = None;
                    continue;
                }
                if !crate::html_policy::PROTOCOLS
                    .iter()
                    .any(|protocol| protocol.starts_with(probe.as_str()))
                {
                    self.external_probe = None;
                }
            }
            if ch == '<' {
                self.tag_pending = true;
                self.bracket = None;
            } else if self.bracket == Some('[') && ch == '[' {
                self.links += 1;
                self.bracket = None;
            } else if self.bracket == Some(']') && ch == ']' && self.links > 0 {
                self.links -= 1;
                self.bracket = None;
            } else {
                self.bracket = matches!(ch, '[' | ']').then_some(ch);
            }
        }
    }
}

fn needs_implicit_newline(prefix: &str, expansion: &str) -> bool {
    !prefix.is_empty()
        && !prefix.ends_with('\n')
        && (expansion.starts_with(['*', '#', ':', ';']) || expansion.starts_with("{|"))
}

pub(crate) fn canonical_template_target(name: &str) -> Option<String> {
    let name = strip_comments(name);
    let name = html_escape::decode_html_entities(&name);
    let name = name.trim();
    if name.is_empty()
        || name
            .chars()
            .any(|c| c.is_control() || "[]{}<>|".contains(c))
    {
        return None;
    }
    let qualified = if let Some(name) = name.strip_prefix(':') {
        if name.trim().is_empty() {
            return None;
        }
        name.trim().to_owned()
    } else {
        let recognized = name
            .split_once(':')
            .is_some_and(|(prefix, _)| crate::inline::namespace(prefix).is_some());
        if recognized {
            name.to_owned()
        } else {
            format!("Template:{name}")
        }
    };
    if qualified.ends_with(':') {
        return None;
    }
    let (page, fragment) = qualified.split_once('#').unwrap_or((&qualified, ""));
    if page.is_empty() || page.ends_with(':') {
        return None;
    }
    let mut result = crate::inline::normalize_title(page);
    if result.is_empty() || result.ends_with(':') {
        return None;
    }
    if qualified.contains('#') {
        result.push('#');
        result.push_str(&crate::inline::normalize_fragment(fragment));
    }
    Some(result)
}

fn marker(nonce: u128, index: usize) -> String {
    let mut n = nonce ^ ((index as u128 + 1) << 64);
    let mut marker = String::new();
    marker.push('\u{e000}');
    for _ in 0..12 {
        marker.push(char::from_u32(0xe100 + (n % 6000) as u32).unwrap());
        n /= 6000;
    }
    marker.push('\u{e001}');
    marker
}
fn self_closed_include(input: &str) -> Option<usize> {
    if !input.starts_with('<') {
        return None;
    }
    let (name, end, closing) = crate::html_policy::tag_info(input)?;
    (!closing
        && matches!(name.as_str(), "noinclude" | "includeonly" | "onlyinclude")
        && input[..end].trim_end_matches('>').trim_end().ends_with('/'))
    .then_some(end)
}

fn starts_ci(input: &str, expected: &str) -> bool {
    input
        .get(..expected.len())
        .is_some_and(|s| s.eq_ignore_ascii_case(expected))
}
fn find_ci(input: &str, expected: &str) -> Option<usize> {
    input
        .as_bytes()
        .windows(expected.len())
        .position(|s| s.eq_ignore_ascii_case(expected.as_bytes()))
}
fn literal_len(input: &str) -> Option<usize> {
    if !input.starts_with('<') {
        return None;
    }
    if input.starts_with("<!--") {
        return Some(input.find("-->").map_or(input.len(), |n| n + 3));
    }
    if let Some((name, end, false)) = crate::html_policy::tag_info(input) {
        if name == "nowiki" || name == "pre" {
            if input[..end].trim_end_matches('>').trim_end().ends_with('/') {
                return Some(end);
            }
            return Some(
                crate::html_policy::raw_close(&input[end..], &name)
                    .map_or(input.len(), |(_, close_end)| end + close_end),
            );
        }
    }
    None
}

pub(crate) fn strip_comments(input: &str) -> String {
    let mut output = String::new();
    let mut pos = 0;
    while pos < input.len() {
        let tail = &input[pos..];
        if tail.starts_with("<!--") {
            pos += tail.find("-->").map_or(tail.len(), |n| n + 3);
        } else if let Some(len) = literal_len(tail) {
            output.push_str(&tail[..len]);
            pos += len;
        } else {
            let len = tail.chars().next().unwrap().len_utf8();
            output.push_str(&tail[..len]);
            pos += len;
        }
    }
    output
}
fn find_active_tag(input: &str, tag: &str) -> Option<usize> {
    let mut pos = 0;
    while pos < input.len() {
        let tail = &input[pos..];
        if let Some(n) = literal_len(tail) {
            pos += n;
            continue;
        }
        if starts_ci(tail, tag) {
            return Some(pos);
        }
        pos += tail.chars().next().unwrap().len_utf8();
    }
    None
}
pub(crate) fn split_top(input: &str, delimiter: char) -> Vec<&str> {
    let (mut result, mut start, mut pos, mut braces, mut links) =
        (Vec::new(), 0, 0, Vec::<usize>::new(), 0usize);
    while pos < input.len() {
        let tail = &input[pos..];
        if let Some(n) = literal_len(tail) {
            pos += n;
            continue;
        }
        if tail.starts_with("{{") {
            let n = if tail.starts_with("{{{") && !tail.starts_with("{{{{") {
                3
            } else {
                2
            };
            braces.push(n);
            pos += n;
        } else if let Some(&n) = braces.last() {
            if tail
                .as_bytes()
                .get(..n)
                .is_some_and(|s| s.iter().all(|b| *b == b'}'))
            {
                braces.pop();
                pos += n;
            } else {
                pos += tail.chars().next().unwrap().len_utf8();
            }
        } else if tail.starts_with("[[") {
            links += 1;
            pos += 2;
        } else if tail.starts_with("]]") && links > 0 {
            links -= 1;
            pos += 2;
        } else {
            let c = tail.chars().next().unwrap();
            if c == delimiter && links == 0 {
                result.push(&input[start..pos]);
                start = pos + c.len_utf8();
            }
            pos += c.len_utf8();
        }
    }
    result.push(&input[start..]);
    result
}
pub(crate) fn construct_end(input: &str) -> Option<usize> {
    let (mut stack, mut pos) = (Vec::<usize>::new(), 0);
    while pos < input.len() {
        let tail = &input[pos..];
        if let Some(n) = literal_len(tail) {
            pos += n;
            continue;
        }
        if tail.starts_with("{{") {
            let n = if tail.starts_with("{{{") && !tail.starts_with("{{{{") {
                3
            } else {
                2
            };
            stack.push(n);
            pos += n;
        } else if let Some(&n) = stack.last() {
            if tail
                .as_bytes()
                .get(..n)
                .is_some_and(|s| s.iter().all(|b| *b == b'}'))
            {
                stack.pop();
                pos += n;
                if stack.is_empty() {
                    return Some(pos);
                }
            } else {
                pos += tail.chars().next().unwrap().len_utf8();
            }
        } else {
            return None;
        }
    }
    None
}
fn inclusion(input: &str) -> String {
    let mut selected = String::new();
    let mut pos = 0;
    let mut found = false;
    while let Some(start) = find_active_tag(&input[pos..], "<onlyinclude>") {
        let start = pos + start + "<onlyinclude>".len();
        let Some(end) = find_active_tag(&input[start..], "</onlyinclude>") else {
            break;
        };
        found = true;
        selected.push_str(&input[start..start + end]);
        pos = start + end + "</onlyinclude>".len();
    }
    let input = if found { selected.as_str() } else { input };
    let mut out = String::new();
    let mut pos = 0;
    while pos < input.len() {
        let tail = &input[pos..];
        if let Some(n) = literal_len(tail) {
            out.push_str(&tail[..n]);
            pos += n;
        } else if let Some(n) = self_closed_include(tail) {
            pos += n;
        } else if starts_ci(tail, "<noinclude>") {
            pos += "<noinclude>".len();
            pos = find_ci(&input[pos..], "</noinclude>")
                .map_or(input.len(), |n| pos + n + "</noinclude>".len());
        } else if let Some(tag) = [
            "<includeonly>",
            "</includeonly>",
            "<onlyinclude>",
            "</onlyinclude>",
        ]
        .into_iter()
        .find(|t| starts_ci(tail, t))
        {
            pos += tag.len();
        } else {
            let n = tail.chars().next().unwrap().len_utf8();
            out.push_str(&tail[..n]);
            pos += n;
        }
    }
    out
}
