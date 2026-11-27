use crate::LocalWiki;
use rwikitext::{
    convert_with_options, ConversionMode, ConversionOptions, ConversionRequest, Diagnostic,
    DiagnosticKind, Format, WikiContext, WikiData,
};
use serde::{Deserialize, Serialize};
use std::io::{self, BufReader, Read, Write};
use std::net::{TcpListener, TcpStream};
use std::sync::Arc;
use std::thread;
use std::time::{Duration, Instant};

const INDEX_HTML: &str = include_str!("serve_ui.html");
const MAX_HEADER_BYTES: usize = 64 * 1024;
const MAX_BODY_BYTES: usize = 16 * 1024 * 1024;
const SOCKET_TIMEOUT: Duration = Duration::from_secs(15);

pub struct ServeConfig {
    pub bind: String,
    pub port: u16,
    pub mode: ConversionMode,
    pub page_title: String,
    pub site_language: String,
    pub reference_time: String,
    pub wiki_data: Option<LocalWiki>,
}

#[derive(Deserialize)]
struct ConvertApiRequest {
    source: String,
    target: String,
    input: String,
    strict: Option<bool>,
}

#[derive(Serialize)]
struct DiagnosticJson {
    kind: &'static str,
    start: usize,
    end: usize,
    message: String,
    display: String,
}

#[derive(Serialize)]
struct MetadataJson {
    categories: Vec<String>,
    redirect_target: Option<String>,
    display_title: Option<String>,
}

#[derive(Serialize)]
struct ConvertApiResponse {
    content: String,
    elapsed_us: u128,
    diagnostics: Vec<DiagnosticJson>,
    metadata: MetadataJson,
}

#[derive(Serialize)]
struct ErrorApiResponse {
    error: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    elapsed_us: Option<u128>,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    diagnostics: Vec<DiagnosticJson>,
}

#[derive(Deserialize)]
struct PreviewApiRequest {
    format: String,
    content: String,
}

#[derive(Serialize)]
struct PreviewApiResponse {
    html: String,
    elapsed_us: u128,
}

struct HttpRequest {
    method: String,
    path: String,
    body: Vec<u8>,
}

struct HttpResponse {
    status: u16,
    reason: &'static str,
    content_type: &'static str,
    body: Vec<u8>,
}

impl HttpResponse {
    fn html(body: &'static str) -> Self {
        Self {
            status: 200,
            reason: "OK",
            content_type: "text/html; charset=utf-8",
            body: body.as_bytes().to_vec(),
        }
    }

    fn json<T: Serialize>(status: u16, reason: &'static str, value: &T) -> Self {
        let body = serde_json::to_vec(value).unwrap_or_else(|_| b"{}".to_vec());
        Self {
            status,
            reason,
            content_type: "application/json; charset=utf-8",
            body,
        }
    }

    fn text(status: u16, reason: &'static str, message: &str) -> Self {
        Self {
            status,
            reason,
            content_type: "text/plain; charset=utf-8",
            body: message.as_bytes().to_vec(),
        }
    }
}

pub fn run(config: ServeConfig) -> Result<(), Box<dyn std::error::Error>> {
    let listener = TcpListener::bind((config.bind.as_str(), config.port))?;
    let local_addr = listener.local_addr()?;
    let url = if local_addr.is_ipv6() {
        format!("http://[{}]:{}", local_addr.ip(), local_addr.port())
    } else {
        format!("http://{}:{}", local_addr.ip(), local_addr.port())
    };
    println!("Listening on {url}");
    io::stdout().flush()?;

    let config = Arc::new(config);
    for stream in listener.incoming() {
        match stream {
            Ok(stream) => {
                let config = Arc::clone(&config);
                thread::spawn(move || {
                    let _ = handle_connection(stream, &config);
                });
            }
            Err(error) => {
                eprintln!("rwikitext serve: accept error: {error}");
            }
        }
    }
    Ok(())
}

fn handle_connection(mut stream: TcpStream, config: &ServeConfig) -> io::Result<()> {
    let _ = stream.set_read_timeout(Some(SOCKET_TIMEOUT));
    let _ = stream.set_write_timeout(Some(SOCKET_TIMEOUT));

    let response = match read_request(&mut stream) {
        Ok(Some(request)) => route_request(request, config),
        Ok(None) => return Ok(()),
        Err(response) => response,
    };

    write_response(&mut stream, &response)
}

fn read_request(stream: &mut TcpStream) -> Result<Option<HttpRequest>, HttpResponse> {
    let mut reader = BufReader::new(stream.try_clone().map_err(|_| {
        HttpResponse::text(500, "Internal Server Error", "failed to clone stream")
    })?);

    let mut buffer = Vec::new();
    let mut chunk = [0u8; 4096];
    let header_end = loop {
        if let Some(pos) = find_header_end(&buffer) {
            break pos;
        }
        if buffer.len() > MAX_HEADER_BYTES {
            return Err(HttpResponse::text(
                431,
                "Request Header Fields Too Large",
                "request headers too large",
            ));
        }
        let n = reader
            .read(&mut chunk)
            .map_err(|_| HttpResponse::text(400, "Bad Request", "failed to read request"))?;
        if n == 0 {
            if buffer.is_empty() {
                return Ok(None);
            }
            return Err(HttpResponse::text(
                400,
                "Bad Request",
                "incomplete HTTP request",
            ));
        }
        buffer.extend_from_slice(&chunk[..n]);
    };

    let header_bytes = &buffer[..header_end];
    let header_str = std::str::from_utf8(header_bytes)
        .map_err(|_| HttpResponse::text(400, "Bad Request", "invalid UTF-8 in headers"))?;
    let mut lines = header_str.split("\r\n");
    let request_line = lines
        .next()
        .ok_or_else(|| HttpResponse::text(400, "Bad Request", "missing request line"))?;
    let mut parts = request_line.split_whitespace();
    let method = parts
        .next()
        .ok_or_else(|| HttpResponse::text(400, "Bad Request", "missing HTTP method"))?
        .to_string();
    let raw_target = parts
        .next()
        .ok_or_else(|| HttpResponse::text(400, "Bad Request", "missing request target"))?;
    let path = raw_target
        .split('?')
        .next()
        .unwrap_or(raw_target)
        .to_string();

    let mut content_length: usize = 0;
    let mut expect_continue = false;
    for line in lines {
        if let Some((name, value)) = line.split_once(':') {
            let name = name.trim();
            let value = value.trim();
            if name.eq_ignore_ascii_case("content-length") {
                content_length = value.parse::<usize>().map_err(|_| {
                    HttpResponse::text(400, "Bad Request", "invalid Content-Length")
                })?;
            } else if name.eq_ignore_ascii_case("expect")
                && value.eq_ignore_ascii_case("100-continue")
            {
                expect_continue = true;
            }
        }
    }

    if content_length > MAX_BODY_BYTES {
        return Err(HttpResponse::text(
            413,
            "Payload Too Large",
            "request body exceeds 16 MiB limit",
        ));
    }

    if expect_continue {
        let _ = stream.write_all(b"HTTP/1.1 100 Continue\r\n\r\n");
        let _ = stream.flush();
    }

    let body_start = header_end + 4;
    let mut body = Vec::with_capacity(content_length);
    if buffer.len() > body_start {
        let available = &buffer[body_start..];
        let take = available.len().min(content_length);
        body.extend_from_slice(&available[..take]);
    }
    while body.len() < content_length {
        let remaining = content_length - body.len();
        let to_read = remaining.min(chunk.len());
        let n = reader
            .read(&mut chunk[..to_read])
            .map_err(|_| HttpResponse::text(400, "Bad Request", "failed to read request body"))?;
        if n == 0 {
            return Err(HttpResponse::text(
                400,
                "Bad Request",
                "unexpected EOF while reading body",
            ));
        }
        body.extend_from_slice(&chunk[..n]);
    }

    Ok(Some(HttpRequest { method, path, body }))
}

fn find_header_end(buf: &[u8]) -> Option<usize> {
    buf.windows(4).position(|window| window == b"\r\n\r\n")
}

fn route_request(request: HttpRequest, config: &ServeConfig) -> HttpResponse {
    match (request.method.as_str(), request.path.as_str()) {
        ("GET", "/") | ("GET", "/index.html") => HttpResponse::html(INDEX_HTML),
        ("POST", "/api/convert") => handle_convert(&request.body, config),
        ("POST", "/api/preview") => handle_preview(&request.body, config),
        (_, "/" | "/index.html" | "/api/convert" | "/api/preview") => {
            HttpResponse::text(405, "Method Not Allowed", "method not allowed")
        }
        _ => HttpResponse::text(404, "Not Found", "not found"),
    }
}

fn handle_convert(body: &[u8], config: &ServeConfig) -> HttpResponse {
    let payload: ConvertApiRequest = match serde_json::from_slice(body) {
        Ok(payload) => payload,
        Err(error) => {
            return HttpResponse::json(
                400,
                "Bad Request",
                &ErrorApiResponse {
                    error: format!("invalid JSON request: {error}"),
                    elapsed_us: None,
                    diagnostics: Vec::new(),
                },
            );
        }
    };

    let source: Format = match payload.source.parse() {
        Ok(format) => format,
        Err(error) => {
            return HttpResponse::json(
                400,
                "Bad Request",
                &ErrorApiResponse {
                    error: error.to_string(),
                    elapsed_us: None,
                    diagnostics: Vec::new(),
                },
            );
        }
    };
    let target: Format = match payload.target.parse() {
        Ok(format) => format,
        Err(error) => {
            return HttpResponse::json(
                400,
                "Bad Request",
                &ErrorApiResponse {
                    error: error.to_string(),
                    elapsed_us: None,
                    diagnostics: Vec::new(),
                },
            );
        }
    };

    let context = WikiContext {
        page_title: &config.page_title,
        site_language: &config.site_language,
        reference_time: &config.reference_time,
    };
    let mode = match payload.strict {
        Some(true) => ConversionMode::Strict,
        Some(false) => ConversionMode::BestEffort,
        None => config.mode,
    };
    let options = ConversionOptions {
        mode,
        wiki_data: config.wiki_data.as_ref().map(|data| data as &dyn WikiData),
        ..ConversionOptions::default()
    };

    let started = Instant::now();
    let result = convert_with_options(
        ConversionRequest {
            source,
            target,
            input: &payload.input,
            context: &context,
        },
        options,
    );
    let elapsed_us = started.elapsed().as_micros();

    match result {
        Ok(result) => HttpResponse::json(
            200,
            "OK",
            &ConvertApiResponse {
                content: result.content,
                elapsed_us,
                diagnostics: result
                    .diagnostics
                    .iter()
                    .map(diagnostic_to_json)
                    .collect(),
                metadata: MetadataJson {
                    categories: result.metadata.categories,
                    redirect_target: result.metadata.redirect_target,
                    display_title: result.metadata.display_title,
                },
            },
        ),
        Err(error) => HttpResponse::json(
            400,
            "Bad Request",
            &ErrorApiResponse {
                error: error.to_string(),
                elapsed_us: Some(elapsed_us),
                diagnostics: error
                    .diagnostics()
                    .iter()
                    .map(diagnostic_to_json)
                    .collect(),
            },
        ),
    }
}

fn handle_preview(body: &[u8], config: &ServeConfig) -> HttpResponse {
    let payload: PreviewApiRequest = match serde_json::from_slice(body) {
        Ok(payload) => payload,
        Err(error) => {
            return HttpResponse::json(
                400,
                "Bad Request",
                &ErrorApiResponse {
                    error: format!("invalid JSON request: {error}"),
                    elapsed_us: None,
                    diagnostics: Vec::new(),
                },
            );
        }
    };

    let format: Format = match payload.format.parse() {
        Ok(format) => format,
        Err(error) => {
            return HttpResponse::json(
                400,
                "Bad Request",
                &ErrorApiResponse {
                    error: error.to_string(),
                    elapsed_us: None,
                    diagnostics: Vec::new(),
                },
            );
        }
    };

    let context = WikiContext {
        page_title: &config.page_title,
        site_language: &config.site_language,
        reference_time: &config.reference_time,
    };
    let options = ConversionOptions {
        mode: ConversionMode::BestEffort,
        wiki_data: config.wiki_data.as_ref().map(|data| data as &dyn WikiData),
        ..ConversionOptions::default()
    };

    let started = Instant::now();
    let html_result = match format {
        Format::Html => Ok(payload.content),
        Format::Wikitext | Format::NamuMark => convert_with_options(
            ConversionRequest {
                source: format,
                target: Format::Html,
                input: &payload.content,
                context: &context,
            },
            options,
        )
        .map(|res| res.content)
        .map_err(|error| error.to_string()),
        Format::Gfm => render_gfm_preview(&payload.content),
    };
    let elapsed_us = started.elapsed().as_micros();

    match html_result {
        Ok(html) => HttpResponse::json(200, "OK", &PreviewApiResponse { html, elapsed_us }),
        Err(error) => HttpResponse::json(
            400,
            "Bad Request",
            &ErrorApiResponse {
                error,
                elapsed_us: Some(elapsed_us),
                diagnostics: Vec::new(),
            },
        ),
    }
}

fn render_gfm_preview(markdown: &str) -> Result<String, String> {
    let mut options = comrak::Options::default();
    options.extension.autolink = true;
    options.extension.table = true;
    options.extension.strikethrough = true;
    options.extension.tasklist = true;
    options.render.unsafe_ = false;
    let arena = comrak::Arena::new();
    let root = comrak::parse_document(&arena, markdown, &options);
    let mut output = Vec::new();
    comrak::format_html(root, &options, &mut output)
        .map_err(|error| format!("failed to render GFM preview: {error}"))?;
    String::from_utf8(output).map_err(|error| format!("invalid UTF-8 in GFM preview: {error}"))
}

fn diagnostic_to_json(diagnostic: &Diagnostic) -> DiagnosticJson {
    let kind = match diagnostic.kind {
        DiagnosticKind::ConversionLoss => "conversion_loss",
        DiagnosticKind::HtmlRemoval => "html_removal",
        DiagnosticKind::WikiLookup => "wiki_lookup",
        DiagnosticKind::ExpansionLimit => "expansion_limit",
    };
    DiagnosticJson {
        kind,
        start: diagnostic.span.start,
        end: diagnostic.span.end,
        message: diagnostic.message.clone(),
        display: diagnostic.to_string(),
    }
}

fn write_response(stream: &mut TcpStream, response: &HttpResponse) -> io::Result<()> {
    let header = format!(
        "HTTP/1.1 {} {}\r\nContent-Type: {}\r\nContent-Length: {}\r\nX-Content-Type-Options: nosniff\r\nCache-Control: no-store\r\nConnection: close\r\n\r\n",
        response.status,
        response.reason,
        response.content_type,
        response.body.len()
    );
    stream.write_all(header.as_bytes())?;
    stream.write_all(&response.body)?;
    stream.flush()
}
