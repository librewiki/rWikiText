use crate::{
    convert_with_options, ConversionMode, ConversionOptions, ConversionRequest, Diagnostic,
    DiagnosticKind, ExpansionLimits, FileInfo, LookupError, WikiContext, WikiData,
};
use serde::{de::DeserializeOwned, Deserialize};
use serde_json::{json, Value};
use std::ffi::{c_char, c_void, CStr, CString};
use std::panic::catch_unwind;
use std::ptr;

const TEMPLATE_TEXT: u32 = 1;
const PAGE_EXISTS: u32 = 2;
const FILE_INFO: u32 = 3;
const MISSING: u32 = 0;
const FOUND: u32 = 1;
const ERROR: u32 = 2;

enum Reply {
    Missing,
    Found(Value),
    Error(LookupError),
}

type Lookup = unsafe extern "C" fn(*mut c_void, u32, *const c_char, usize, *mut Reply);

struct Callbacks {
    lookup: Option<Lookup>,
    user_data: *mut c_void,
}

impl Callbacks {
    fn request<T: DeserializeOwned>(
        &self,
        operation: u32,
        title: &str,
    ) -> Result<Option<T>, LookupError> {
        let Some(lookup) = self.lookup else {
            return Ok(None);
        };
        let mut reply = Reply::Missing;
        // The title and reply are live until this synchronous callback returns.
        unsafe {
            lookup(
                self.user_data,
                operation,
                title.as_ptr().cast(),
                title.len(),
                &mut reply,
            );
        }
        match reply {
            Reply::Missing => Ok(None),
            Reply::Found(value) => serde_json::from_value(value)
                .map(Some)
                .map_err(|error| LookupError::new(format!("invalid lookup value: {error}"))),
            Reply::Error(error) => Err(error),
        }
    }
}

#[derive(Deserialize)]
struct FileReply {
    url: String,
    width: Option<u32>,
    height: Option<u32>,
}

impl WikiData for Callbacks {
    fn template_text(&self, title: &str) -> Result<Option<String>, LookupError> {
        self.request(TEMPLATE_TEXT, title)
    }

    fn page_exists(&self, title: &str) -> Result<Option<bool>, LookupError> {
        self.request(PAGE_EXISTS, title)
    }

    fn file_info(&self, title: &str) -> Result<Option<FileInfo>, LookupError> {
        Ok(self
            .request::<FileReply>(FILE_INFO, title)?
            .map(|file| FileInfo {
                url: file.url,
                width: file.width,
                height: file.height,
            }))
    }
}

#[derive(Deserialize)]
struct Request {
    source: String,
    target: String,
    input: String,
    #[serde(default)]
    context: Context,
    #[serde(default)]
    mode: Mode,
    #[serde(default)]
    expansion_limits: Limits,
}

#[derive(Default, Deserialize)]
struct Context {
    page_title: Option<String>,
    site_language: Option<String>,
    reference_time: Option<String>,
}

#[derive(Default, Deserialize)]
#[serde(rename_all = "snake_case")]
enum Mode {
    #[default]
    BestEffort,
    Strict,
}

#[derive(Default, Deserialize)]
struct Limits {
    max_depth: Option<usize>,
    max_expansions: Option<usize>,
    max_output_bytes: Option<usize>,
}

fn diagnostics_json(diagnostics: &[Diagnostic]) -> Vec<Value> {
    diagnostics
        .iter()
        .map(|diagnostic| {
            let kind = match diagnostic.kind {
                DiagnosticKind::ConversionLoss => "conversion_loss",
                DiagnosticKind::HtmlRemoval => "html_removal",
                DiagnosticKind::WikiLookup => "wiki_lookup",
                DiagnosticKind::ExpansionLimit => "expansion_limit",
            };
            json!({
                "kind": kind,
                "start": diagnostic.span.start,
                "end": diagnostic.span.end,
                "message": diagnostic.message,
            })
        })
        .collect()
}

fn failure(error: impl ToString, diagnostics: &[Diagnostic]) -> Value {
    json!({ "ok": false, "error": error.to_string(), "diagnostics": diagnostics_json(diagnostics) })
}

fn convert(request: Request, wiki_data: &dyn WikiData) -> Value {
    let (source, target) = match request
        .source
        .parse()
        .and_then(|source| request.target.parse().map(|target| (source, target)))
    {
        Ok(formats) => formats,
        Err(error) => return failure(error, &[]),
    };
    let defaults = WikiContext::default();
    let context = WikiContext {
        page_title: request
            .context
            .page_title
            .as_deref()
            .unwrap_or(defaults.page_title),
        site_language: request
            .context
            .site_language
            .as_deref()
            .unwrap_or(defaults.site_language),
        reference_time: request
            .context
            .reference_time
            .as_deref()
            .unwrap_or(defaults.reference_time),
    };
    let mode = match request.mode {
        Mode::BestEffort => ConversionMode::BestEffort,
        Mode::Strict => ConversionMode::Strict,
    };
    let defaults = ExpansionLimits::default();
    let limits = request.expansion_limits;
    let options = ConversionOptions {
        mode,
        wiki_data: Some(wiki_data),
        expansion_limits: ExpansionLimits {
            max_depth: limits.max_depth.unwrap_or(defaults.max_depth),
            max_expansions: limits.max_expansions.unwrap_or(defaults.max_expansions),
            max_output_bytes: limits.max_output_bytes.unwrap_or(defaults.max_output_bytes),
        },
    };
    match convert_with_options(
        ConversionRequest {
            source,
            target,
            input: &request.input,
            context: &context,
        },
        options,
    ) {
        Ok(result) => json!({
            "ok": true,
            "content": result.content,
            "metadata": {
                "categories": result.metadata.categories,
                "category_sort_keys": result.metadata.category_sort_keys,
                "redirect_target": result.metadata.redirect_target,
                "display_title": result.metadata.display_title,
            },
            "diagnostics": diagnostics_json(&result.diagnostics),
        }),
        Err(error) => failure(&error, error.diagnostics()),
    }
}

/// `value` must be null or point to a readable, NUL-terminated C string.
unsafe fn read_json<T: DeserializeOwned>(value: *const c_char) -> Result<T, String> {
    if value.is_null() {
        return Err("expected a JSON string buffer".into());
    }
    let json = unsafe { CStr::from_ptr(value) }
        .to_str()
        .map_err(|error| error.to_string())?;
    serde_json::from_str(json).map_err(|error| error.to_string())
}

#[no_mangle]
extern "C" fn rwikitext_v1_abi_version() -> u32 {
    1
}

/// `request_json` must be null or point to a readable, NUL-terminated UTF-8
/// string. The callback must obey the reply lifetime in the public header.
#[no_mangle]
unsafe extern "C" fn rwikitext_v1_convert(
    request_json: *const c_char,
    lookup: Option<Lookup>,
    user_data: *mut c_void,
) -> *const c_char {
    catch_unwind(|| {
        let response = match unsafe { read_json(request_json) } {
            Ok(request) => convert(request, &Callbacks { lookup, user_data }),
            Err(error) => failure(format!("invalid conversion request: {error}"), &[]),
        };
        // JSON serialization escapes every NUL in the payload.
        CString::new(response.to_string()).map_or(ptr::null(), |json| json.into_raw())
    })
    .unwrap_or(ptr::null())
}

/// `response` must be null or an unfreed pointer returned by the converter.
#[no_mangle]
unsafe extern "C" fn rwikitext_v1_free(response: *const c_char) {
    if !response.is_null() {
        drop(unsafe { CString::from_raw(response.cast_mut()) });
    }
}

/// `reply` must be null or the active callback's reply pointer. `value_json`
/// must be null or point to a readable, NUL-terminated UTF-8 string.
#[no_mangle]
unsafe extern "C" fn rwikitext_v1_reply_set(
    reply: *mut Reply,
    status: u32,
    value_json: *const c_char,
) -> i32 {
    let Some(reply) = (unsafe { reply.as_mut() }) else {
        return -1;
    };
    let value = match status {
        MISSING if value_json.is_null() => Ok(Reply::Missing),
        MISSING => Err("missing reply requires a NULL payload".into()),
        FOUND => unsafe { read_json(value_json) }.map(Reply::Found),
        ERROR => unsafe { read_json::<String>(value_json) }
            .map(|error| Reply::Error(LookupError::new(error))),
        _ => Err(format!("unknown lookup status: {status}")),
    };
    match value {
        Ok(value) => {
            *reply = value;
            0
        }
        Err(error) => {
            *reply = Reply::Error(LookupError::new(format!("invalid lookup reply: {error}")));
            -1
        }
    }
}
