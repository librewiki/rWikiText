use napi::bindgen_prelude::*;
use napi::{JsFunction, JsObject, JsUnknown, ValueType};
use napi_derive::napi;
use rwikitext::{
    convert_with_options, ConversionMode, ConversionOptions, ConversionRequest as CoreRequest,
    Diagnostic as CoreDiagnostic, DiagnosticKind, ExpansionLimits as CoreExpansionLimits,
    FileInfo, Format, LookupError, WikiContext as CoreWikiContext, WikiData,
};
use serde::de::DeserializeOwned;
use serde::Deserialize;

#[napi(object)]
#[derive(Default, Clone)]
pub struct WikiContext {
    #[napi(js_name = "page_title")]
    pub page_title: Option<String>,
    #[napi(js_name = "site_language")]
    pub site_language: Option<String>,
    #[napi(js_name = "reference_time")]
    pub reference_time: Option<String>,
}

#[napi(object)]
#[derive(Default, Clone)]
pub struct ExpansionLimits {
    #[napi(js_name = "max_depth")]
    pub max_depth: Option<u32>,
    #[napi(js_name = "max_expansions")]
    pub max_expansions: Option<u32>,
    #[napi(js_name = "max_output_bytes")]
    pub max_output_bytes: Option<u32>,
}

#[napi(object)]
#[derive(Clone)]
pub struct ConversionRequest {
    pub source: String,
    pub target: String,
    pub input: String,
    pub context: Option<WikiContext>,
    pub mode: Option<String>,
    #[napi(js_name = "expansion_limits")]
    pub expansion_limits: Option<ExpansionLimits>,
}

#[napi(object)]
pub struct Diagnostic {
    pub kind: String,
    pub start: u32,
    pub end: u32,
    pub message: String,
}

#[napi(object)]
pub struct PageMetadata {
    pub categories: Vec<String>,
    #[napi(js_name = "category_sort_keys")]
    pub category_sort_keys: Vec<Either<String, Null>>,
    #[napi(js_name = "redirect_target")]
    pub redirect_target: Either<String, Null>,
    #[napi(js_name = "display_title")]
    pub display_title: Either<String, Null>,
}

#[napi(object)]
pub struct ConversionResponse {
    pub ok: bool,
    pub content: Option<String>,
    pub metadata: Option<PageMetadata>,
    pub error: Option<String>,
    pub diagnostics: Vec<Diagnostic>,
}

#[derive(Deserialize)]
struct FileReply {
    url: String,
    width: Option<u32>,
    height: Option<u32>,
}

struct JsWikiData<'a> {
    env: &'a Env,
    lookup: Option<&'a JsFunction>,
}

impl JsWikiData<'_> {
    fn request<T: DeserializeOwned>(
        &self,
        operation: &str,
        title: &str,
    ) -> std::result::Result<Option<T>, LookupError> {
        let Some(lookup) = self.lookup else {
            return Ok(None);
        };

        let op_arg = self
            .env
            .create_string(operation)
            .map_err(|e| LookupError::new(e.reason))?
            .into_unknown();
        let title_arg = self
            .env
            .create_string(title)
            .map_err(|e| LookupError::new(e.reason))?
            .into_unknown();

        let value: JsUnknown = match lookup.call(None, &[op_arg, title_arg]) {
            Ok(val) => val,
            Err(err) => {
                let message = extract_js_error_message(self.env, err);
                return Err(LookupError::new(message));
            }
        };

        let value_type = value
            .get_type()
            .map_err(|e| LookupError::new(e.reason))?;
        let value = match value_type {
            ValueType::Undefined | ValueType::Null => return Ok(None),
            ValueType::Object | ValueType::Function => {
                let obj = value
                    .coerce_to_object()
                    .map_err(|e| LookupError::new(e.reason))?;
                if is_thenable(&obj) {
                    return Err(LookupError::new("lookup must be synchronous"));
                }
                obj.into_unknown()
            }
            _ => value,
        };

        self.env
            .from_js_value::<T, _>(value)
            .map(Some)
            .map_err(|e| LookupError::new(format!("invalid lookup value: {}", e.reason)))
    }
}

fn is_thenable(obj: &JsObject) -> bool {
    match obj.get_named_property::<JsUnknown>("then") {
        Ok(then_prop) => matches!(then_prop.get_type(), Ok(ValueType::Function)),
        Err(_) => false,
    }
}

fn extract_js_error_message(env: &Env, err: napi::Error) -> String {
    let mut is_pending = false;
    let pending_ok =
        unsafe { napi::sys::napi_is_exception_pending(env.raw(), &mut is_pending) } == napi::sys::Status::napi_ok;
    if pending_ok && is_pending {
        let mut raw_exception = std::ptr::null_mut();
        if unsafe { napi::sys::napi_get_and_clear_last_exception(env.raw(), &mut raw_exception) }
            == napi::sys::Status::napi_ok
            && !raw_exception.is_null()
        {
            if let Ok(exception) = unsafe { JsUnknown::from_napi_value(env.raw(), raw_exception) } {
                if let Ok(obj) = exception.coerce_to_object() {
                    if let Ok(msg) = obj.get_named_property::<JsUnknown>("message") {
                        if let Ok(s) = msg.coerce_to_string() {
                            if let Ok(utf8) = s.into_utf8() {
                                if let Ok(text) = utf8.as_str() {
                                    if !text.is_empty() {
                                        return text.to_string();
                                    }
                                }
                            }
                        }
                    }
                    if let Ok(s) = obj.coerce_to_string() {
                        if let Ok(utf8) = s.into_utf8() {
                            if let Ok(text) = utf8.as_str() {
                                if !text.is_empty() {
                                    return text.to_string();
                                }
                            }
                        }
                    }
                }
            }
        }
    }
    if !err.reason.is_empty() {
        return err.reason;
    }
    err.to_string()
}

impl WikiData for JsWikiData<'_> {
    fn template_text(&self, title: &str) -> std::result::Result<Option<String>, LookupError> {
        self.request("template_text", title)
    }

    fn page_exists(&self, title: &str) -> std::result::Result<Option<bool>, LookupError> {
        self.request("page_exists", title)
    }

    fn file_info(&self, title: &str) -> std::result::Result<Option<FileInfo>, LookupError> {
        Ok(self
            .request::<FileReply>("file_info", title)?
            .map(|file| FileInfo {
                url: file.url,
                width: file.width,
                height: file.height,
            }))
    }
}

fn convert_diagnostics(diagnostics: &[CoreDiagnostic]) -> Vec<Diagnostic> {
    diagnostics
        .iter()
        .map(|d| {
            let kind = match d.kind {
                DiagnosticKind::ConversionLoss => "conversion_loss",
                DiagnosticKind::HtmlRemoval => "html_removal",
                DiagnosticKind::WikiLookup => "wiki_lookup",
                DiagnosticKind::ExpansionLimit => "expansion_limit",
            };
            Diagnostic {
                kind: kind.to_string(),
                start: d.span.start as u32,
                end: d.span.end as u32,
                message: d.message.clone(),
            }
        })
        .collect()
}

fn failure(error: impl ToString, diagnostics: &[CoreDiagnostic]) -> ConversionResponse {
    ConversionResponse {
        ok: false,
        content: None,
        metadata: None,
        error: Some(error.to_string()),
        diagnostics: convert_diagnostics(diagnostics),
    }
}

fn run_conversion(request: &ConversionRequest, wiki_data: Option<&dyn WikiData>) -> ConversionResponse {
    let source: Format = match request.source.parse() {
        Ok(f) => f,
        Err(e) => return failure(e, &[]),
    };
    let target: Format = match request.target.parse() {
        Ok(f) => f,
        Err(e) => return failure(e, &[]),
    };
    let mode = match request.mode.as_deref() {
        None | Some("best_effort") => ConversionMode::BestEffort,
        Some("strict") => ConversionMode::Strict,
        Some(other) => return failure(format!("unknown conversion mode: {other}"), &[]),
    };

    let default_ctx = CoreWikiContext::default();
    let ctx_input = request.context.as_ref();
    let context = CoreWikiContext {
        page_title: ctx_input
            .and_then(|c| c.page_title.as_deref())
            .unwrap_or(default_ctx.page_title),
        site_language: ctx_input
            .and_then(|c| c.site_language.as_deref())
            .unwrap_or(default_ctx.site_language),
        reference_time: ctx_input
            .and_then(|c| c.reference_time.as_deref())
            .unwrap_or(default_ctx.reference_time),
    };

    let default_limits = CoreExpansionLimits::default();
    let limits_input = request.expansion_limits.as_ref();
    let expansion_limits = CoreExpansionLimits {
        max_depth: limits_input
            .and_then(|l| l.max_depth)
            .map(|v| v as usize)
            .unwrap_or(default_limits.max_depth),
        max_expansions: limits_input
            .and_then(|l| l.max_expansions)
            .map(|v| v as usize)
            .unwrap_or(default_limits.max_expansions),
        max_output_bytes: limits_input
            .and_then(|l| l.max_output_bytes)
            .map(|v| v as usize)
            .unwrap_or(default_limits.max_output_bytes),
    };

    let options = ConversionOptions {
        mode,
        wiki_data,
        expansion_limits,
    };

    match convert_with_options(
        CoreRequest {
            source,
            target,
            input: &request.input,
            context: &context,
        },
        options,
    ) {
        Ok(result) => ConversionResponse {
            ok: true,
            content: Some(result.content),
            metadata: Some(PageMetadata {
                categories: result.metadata.categories,
                category_sort_keys: result
                    .metadata
                    .category_sort_keys
                    .into_iter()
                    .map(|k| match k {
                        Some(v) => Either::A(v),
                        None => Either::B(Null),
                    })
                    .collect(),
                redirect_target: match result.metadata.redirect_target {
                    Some(v) => Either::A(v),
                    None => Either::B(Null),
                },
                display_title: match result.metadata.display_title {
                    Some(v) => Either::A(v),
                    None => Either::B(Null),
                },
            }),
            error: None,
            diagnostics: convert_diagnostics(&result.diagnostics),
        },
        Err(error) => failure(&error, error.diagnostics()),
    }
}

pub struct ConvertTask {
    request: ConversionRequest,
}

#[napi]
impl Task for ConvertTask {
    type Output = ConversionResponse;
    type JsValue = ConversionResponse;

    fn compute(&mut self) -> Result<Self::Output> {
        Ok(run_conversion(&self.request, None))
    }

    fn resolve(&mut self, _env: Env, output: Self::Output) -> Result<Self::JsValue> {
        Ok(output)
    }
}

/// Convert a document synchronously.
///
/// The optional synchronous `lookup(operation, title)` callback runs on the calling thread.
#[napi]
pub fn convert(
    env: Env,
    request: ConversionRequest,
    #[napi(ts_arg_type = "(operation: 'template_text' | 'page_exists' | 'file_info', title: string) => unknown")]
    lookup: Option<JsFunction>,
) -> ConversionResponse {
    let wiki_data = JsWikiData {
        env: &env,
        lookup: lookup.as_ref(),
    };
    run_conversion(&request, Some(&wiki_data))
}

/// Convert a document asynchronously on the libuv worker pool (without lookup callbacks).
#[napi]
pub fn convert_async(request: ConversionRequest) -> AsyncTask<ConvertTask> {
    AsyncTask::new(ConvertTask { request })
}

#[napi]
pub struct WikiText;

#[napi]
impl WikiText {
    #[napi(constructor)]
    pub fn new(_library_path: Option<String>) -> Self {
        Self
    }

    #[napi]
    pub fn convert(
        &self,
        env: Env,
        request: ConversionRequest,
        #[napi(ts_arg_type = "(operation: 'template_text' | 'page_exists' | 'file_info', title: string) => unknown")]
        lookup: Option<JsFunction>,
    ) -> ConversionResponse {
        convert(env, request, lookup)
    }

    #[napi]
    pub fn convert_async(&self, request: ConversionRequest) -> AsyncTask<ConvertTask> {
        convert_async(request)
    }
}
