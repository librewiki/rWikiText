use crate::{expansion, html_policy, inline, Diagnostic};
use serde_json::{json, Value};
use std::collections::{BTreeMap, BTreeSet};
use std::ops::Range;

pub(crate) fn metadata(call: &expansion::Invocation) -> (String, String) {
    let target = if let Some(function) = &call.function {
        json!({"wt": call.target, "function": function})
    } else {
        json!({"wt": call.target, "href": format!("./{}", inline::encode_component(&call.title))})
    };
    let mut params = BTreeMap::new();
    let mut positional = Vec::new();
    for arg in &call.arguments {
        params.insert(&arg.key, json!({"wt": arg.value}));
        positional.push(if arg.named {
            json!({"k": arg.key, "named": true})
        } else {
            json!({"k": arg.key})
        });
    }
    (
        json!({"parts": [{"template": {"target": target, "params": params, "i": 0}}]}).to_string(),
        json!({"pi": [positional]}).to_string(),
    )
}

fn attribute(value: &str) -> String {
    html_escape::encode_double_quoted_attribute(value).into_owned()
}

pub(crate) fn annotate(
    html: &str,
    id: usize,
    block: bool,
    mw: &str,
    parsoid: &str,
) -> Option<String> {
    let about = format!(" about=\"#mwt{id}\"");
    let data = format!(
        " data-mw=\"{}\" data-parsoid=\"{}\"",
        attribute(mw),
        attribute(parsoid)
    );
    let html = if block { html.trim_matches('\n') } else { html };
    let mut output = String::new();
    let mut rest = html;
    let mut first = true;
    loop {
        let (length, name, opening, attrs) = if rest.starts_with('<') {
            let length = html_policy::element_len(rest)?;
            let (name, opening, _) = html_policy::tag_info(rest)?;
            (
                length,
                name,
                opening,
                html_policy::attributes(rest).collect::<BTreeMap<_, _>>(),
            )
        } else {
            let length = rest.find('<').unwrap_or(rest.len());
            if !first && block && rest[..length].trim_ascii().is_empty() {
                output.push_str(&rest[..length]);
                rest = &rest[length..];
                if rest.is_empty() {
                    break;
                }
                continue;
            }
            (length, "span".into(), 0, BTreeMap::new())
        };
        output.push('<');
        output.push_str(&name);
        output.push_str(&about);
        if first {
            let types = attrs.get("typeof").map_or("", String::as_str);
            output.push_str(" typeof=\"mw:Transclusion");
            if !types.is_empty() {
                output.push(' ');
                output.push_str(&attribute(types));
            }
            output.push('"');
            output.push_str(&data);
        }
        for (key, value) in attrs {
            if key != "about"
                && !(first && matches!(key.as_str(), "typeof" | "data-mw" | "data-parsoid"))
            {
                output.push_str(&format!(" {key}=\"{}\"", attribute(&value)));
            }
        }
        output.push('>');
        output.push_str(&rest[opening..length]);
        if opening == 0 {
            output.push_str("</span>");
        }
        first = false;
        rest = &rest[length..];
        if rest.is_empty() {
            break;
        }
    }
    Some(output)
}

fn valid_about(value: &str) -> bool {
    value
        .strip_prefix("#mwt")
        .is_some_and(|id| !id.is_empty() && id.bytes().all(|c| c.is_ascii_digit()))
}

fn valid_target(target: &Value, wt: &str) -> bool {
    if let Some(href) = target["href"].as_str() {
        let Some(encoded) = href.strip_prefix("./") else {
            return false;
        };
        let Ok(decoded) = inline::decode_component(encoded) else {
            return false;
        };
        if target.get("function").is_some()
            || inline::internal_href(&decoded).ok().as_deref() != Some(href)
        {
            return false;
        }
        if let Some(canonical) = expansion::canonical_template_target(wt) {
            inline::normalize_title(&canonical) == decoded
        } else {
            wt.contains("{{")
        }
    } else if let Some(function) = target["function"].as_str() {
        let name = wt.split_once(':').map_or(wt, |(name, _)| name).trim();
        !name.is_empty()
            && (name.eq_ignore_ascii_case(function)
                || name
                    .strip_prefix('#')
                    .is_some_and(|name| name.eq_ignore_ascii_case(function)))
    } else {
        false
    }
}

fn assemble(template: &Value, infos: Option<&Value>, index: usize) -> Option<String> {
    let target = &template["target"];
    let wt = target["wt"].as_str()?;
    if template["i"].as_u64()? != index as u64 || !valid_target(target, wt) {
        return None;
    }
    let empty = serde_json::Map::new();
    let params = template
        .get("params")
        .map_or(Some(&empty), Value::as_object)?;
    let mut positional = 1usize;
    let mut ordered = Vec::new();
    let mut emitted = BTreeSet::new();
    if let Some(infos) = infos {
        let infos = infos.as_array()?;
        for info in infos.iter().rev() {
            let key = info["k"].as_str()?;
            let named = info.get("named").map_or(Some(false), Value::as_bool)?;
            if emitted.insert(key) {
                ordered.push((key, Some(params.get(key)?), named));
            } else if !named {
                // The discarded positional argument still advances the next index.
                ordered.push((key, None, false));
            }
        }
        ordered.reverse();
    }
    let mut remaining: Vec<_> = params
        .iter()
        .filter(|(key, _)| !emitted.contains(key.as_str()))
        .collect();
    remaining.sort_by_key(|(key, _)| (key.parse::<usize>().unwrap_or(usize::MAX), *key));
    ordered.extend(
        remaining
            .into_iter()
            .map(|(key, value)| (key.as_str(), Some(value), false)),
    );

    let mut call = format!("{{{{{wt}");
    for (key, param, named) in ordered {
        let value = if let Some(param) = param {
            param["wt"].as_str()?
        } else {
            ""
        };
        let function = target.get("function").is_some();
        let explicit = named
            || key != positional.to_string()
            || !function && expansion::split_top(value, '=').len() > 1;
        if function && explicit {
            return None;
        }
        if key.trim() != key || key.contains(['|', '=', '{', '}', '<', '>']) {
            return None;
        }
        if expansion::split_top(value, '|').len() != 1 {
            return None;
        }
        call.push('|');
        if explicit {
            call.push_str(key);
            call.push('=');
        } else {
            positional += 1;
        }
        call.push_str(value);
    }
    call.push_str("}}");
    if wt.contains(|ch: char| ch.is_control() && !ch.is_ascii_whitespace())
        || expansion::split_top(wt, '|').len() != 1
        || call.contains('\0')
        || expansion::construct_end(&call) != Some(call.len())
    {
        return None;
    }
    Some(call)
}

fn invocation(mw: &str, parsoid: Option<&str>) -> Option<String> {
    let data: Value = serde_json::from_str(mw).ok()?;
    let parsoid: Value = parsoid
        .map(serde_json::from_str)
        .transpose()
        .ok()?
        .unwrap_or(Value::Null);
    if !parsoid.is_null() && !parsoid.is_object() {
        return None;
    }
    let info = parsoid.get("pi").map(Value::as_array).unwrap_or(None);
    if parsoid.get("pi").is_some() && info.is_none() {
        return None;
    }
    let parts = data["parts"].as_array()?;
    let mut output = String::new();
    let mut count = 0;
    for part in parts {
        if let Some(text) = part.as_str().filter(|text| {
            text.chars().all(|ch| {
                ch.is_alphanumeric()
                    || ch.is_ascii_whitespace()
                    || matches!(ch, '.' | ',' | '!' | '?' | '(' | ')')
            })
        }) {
            output.push_str(text);
        } else {
            output.push_str(&assemble(
                &part["template"],
                info.and_then(|entries| entries.get(count)),
                count,
            )?);
            count += 1;
        }
    }
    (count > 0 && info.is_none_or(|entries| entries.len() == count)).then_some(output)
}

pub(crate) struct RecoveredCall {
    pub span: Range<usize>,
    pub code: String,
    pub block: bool,
}

pub(crate) fn recover(input: &str) -> (Vec<RecoveredCall>, Vec<Diagnostic>) {
    let (mut edits, mut diagnostics, mut pos) = (Vec::new(), Vec::new(), 0);
    while let Some(next) = input[pos..].find('<') {
        pos += next;
        let rest = &input[pos..];
        if rest.starts_with("<!--") {
            pos += rest.find("-->").map_or(rest.len(), |end| end + 3);
            continue;
        }
        let Some((name, opening, closing)) = html_policy::tag_info(rest) else {
            pos += 1;
            continue;
        };
        if !closing
            && (html_policy::is_inert_html(&name)
                || matches!(name.as_str(), "svg" | "math" | "noscript" | "select"))
        {
            pos += if html_policy::is_inert_html(&name) && name != "template" {
                html_policy::raw_close(&rest[opening..], &name)
                    .map_or(rest.len(), |(_, end)| opening + end)
            } else {
                html_policy::element_len(rest).unwrap_or(rest.len())
            };
            continue;
        }
        let attrs: BTreeMap<_, _> = html_policy::attributes(rest).collect();
        if closing
            || !attrs.get("typeof").is_some_and(|types| {
                types
                    .split_ascii_whitespace()
                    .any(|kind| kind == "mw:Transclusion")
            })
        {
            pos += opening;
            continue;
        }
        let call = (|| {
            if !html_policy::can_import(&name)
                || attrs.len() != html_policy::attributes(rest).count()
            {
                return None;
            }
            let about = attrs.get("about").filter(|about| valid_about(about))?;
            let metadata = attrs.get("data-mw")?;
            let code = invocation(metadata, attrs.get("data-parsoid").map(String::as_str))?;
            let mut end = html_policy::element_len(rest)?;
            loop {
                let next = rest[end..].trim_start_matches(|ch: char| ch.is_ascii_whitespace());
                let attributes: BTreeMap<_, _> = html_policy::attributes(next).collect();
                if attributes.get("about") != Some(about) || attributes.contains_key("data-mw") {
                    break;
                }
                end = rest.len() - next.len() + html_policy::element_len(next)?;
            }
            Some((end, code))
        })();
        if let Some((end, code)) = call {
            edits.push(RecoveredCall {
                span: pos..pos + end,
                code,
                block: html_policy::is_block(&name),
            });
            pos += end;
        } else {
            diagnostics.push(Diagnostic::loss(
                pos..pos + opening,
                "invalid Parsoid transclusion metadata",
            ));
            pos += html_policy::element_len(rest).unwrap_or(opening);
        }
    }
    (edits, diagnostics)
}
