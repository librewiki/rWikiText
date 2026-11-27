use serde_json::{json, Value};
use std::io::{BufRead, BufReader, Read, Write};
use std::net::TcpStream;
use std::process::{Child, Command, Stdio};
use std::time::Duration;

struct ServerGuard {
    child: Child,
    addr: String,
}

impl ServerGuard {
    fn spawn(extra_args: &[&str]) -> Self {
        let mut child = Command::new(env!("CARGO_BIN_EXE_rwikitext"))
            .arg("serve")
            .args(extra_args)
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .expect("spawn rwikitext serve");

        let stdout = child.stdout.take().expect("capture stdout");
        let mut reader = BufReader::new(stdout);
        let mut line = String::new();
        reader
            .read_line(&mut line)
            .expect("read listening line from serve");
        let url = line
            .trim()
            .strip_prefix("Listening on http://")
            .unwrap_or_else(|| panic!("unexpected startup line: {line:?}"));

        Self {
            child,
            addr: url.to_owned(),
        }
    }

    fn request(&self, method: &str, path: &str, body: Option<&str>) -> (u16, String, String) {
        let mut stream = TcpStream::connect(&self.addr).expect("connect to server");
        stream
            .set_read_timeout(Some(Duration::from_secs(5)))
            .expect("set read timeout");
        let payload = body.unwrap_or("");
        let request = if body.is_some() {
            format!(
                "{method} {path} HTTP/1.1\r\nHost: {}\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{payload}",
                self.addr,
                payload.len()
            )
        } else {
            format!(
                "{method} {path} HTTP/1.1\r\nHost: {}\r\nConnection: close\r\n\r\n",
                self.addr
            )
        };
        stream.write_all(request.as_bytes()).expect("write request");
        stream.flush().expect("flush request");

        let mut raw = Vec::new();
        stream.read_to_end(&mut raw).expect("read response");
        let text = String::from_utf8(raw).expect("valid utf-8 response");
        let (headers, resp_body) = text
            .split_once("\r\n\r\n")
            .expect("HTTP response header/body separator");
        let status_line = headers.lines().next().expect("status line");
        let status: u16 = status_line
            .split_whitespace()
            .nth(1)
            .expect("status code")
            .parse()
            .expect("numeric status code");
        (status, headers.to_owned(), resp_body.to_owned())
    }
}

impl Drop for ServerGuard {
    fn drop(&mut self) {
        let _ = self.child.kill();
        let _ = self.child.wait();
    }
}

#[test]
fn serve_serves_ui_convert_and_lazy_preview_endpoints() {
    let server = ServerGuard::spawn(&["--bind", "127.0.0.1", "--port", "0"]);

    let (status, headers, html) = server.request("GET", "/", None);
    assert_eq!(status, 200);
    assert!(headers.contains("text/html"));
    assert!(html.contains("id=\"source-format\""));
    assert!(html.contains("id=\"target-format\""));
    assert!(html.contains("id=\"source-input\""));
    assert!(html.contains("id=\"raw-output\""));
    assert!(html.contains("id=\"tab-preview\""));
    assert!(html.contains("id=\"elapsed-time\""));
    assert!(html.contains("/api/convert"));
    assert!(html.contains("/api/preview"));
    assert!(html.contains("loading=\"lazy\"") || html.contains("\"loading\", \"lazy\""));

    let convert_body = json!({
        "source": "wikitext",
        "target": "html",
        "input": "Hello '''world'''"
    })
    .to_string();
    let (status, _, body) = server.request("POST", "/api/convert", Some(&convert_body));
    assert_eq!(status, 200, "{body}");
    let json: Value = serde_json::from_str(&body).expect("parse convert json");
    assert!(json["content"].as_str().unwrap().contains("<b>world</b>"));
    assert!(json["elapsed_us"].is_u64());

    let namu_body = json!({
        "source": "namumark",
        "target": "wikitext",
        "input": "'''굵게'''"
    })
    .to_string();
    let (status, _, body) = server.request("POST", "/api/convert", Some(&namu_body));
    assert_eq!(status, 200, "{body}");
    let json: Value = serde_json::from_str(&body).expect("parse namumark convert json");
    assert!(json["content"].as_str().unwrap().contains("'''굵게'''"));
    assert!(json["elapsed_us"].is_u64());

    let unsupported_body = json!({
        "source": "gfm",
        "target": "html",
        "input": "**bold**"
    })
    .to_string();
    let (status, _, body) = server.request("POST", "/api/convert", Some(&unsupported_body));
    assert_eq!(status, 400);
    let json: Value = serde_json::from_str(&body).expect("parse error json");
    assert!(json["error"]
        .as_str()
        .unwrap()
        .contains("unsupported conversion direction"));

    for (format, content, expected_snippet) in [
        ("html", "<p>Direct HTML</p>", "<p>Direct HTML</p>"),
        ("wikitext", "Hello '''wiki'''", "<b>wiki</b>"),
        ("namumark", "'''나무'''", "<strong>나무</strong>"),
        ("gfm", "**markdown**", "<strong>markdown</strong>"),
    ] {
        let preview_body = json!({
            "format": format,
            "content": content
        })
        .to_string();
        let (status, _, body) = server.request("POST", "/api/preview", Some(&preview_body));
        assert_eq!(status, 200, "preview {format}: {body}");
        let json: Value = serde_json::from_str(&body).expect("parse preview json");
        let rendered = json["html"].as_str().expect("preview html string");
        assert!(
            rendered.contains(expected_snippet),
            "preview {format}: expected {expected_snippet:?} in {rendered:?}"
        );
        assert!(json["elapsed_us"].is_u64());
    }

    let (status, _, _) = server.request("GET", "/nonexistent", None);
    assert_eq!(status, 404);

    let (status, _, _) = server.request("GET", "/api/convert", None);
    assert_eq!(status, 405);
}

#[test]
fn serve_supports_equals_flags_help_and_rejects_invalid_args() {
    let server = ServerGuard::spawn(&["--bind=127.0.0.1", "--port=0"]);
    let (status, _, _) = server.request("GET", "/", None);
    assert_eq!(status, 200);
    drop(server);

    let help = Command::new(env!("CARGO_BIN_EXE_rwikitext"))
        .args(["serve", "--help"])
        .output()
        .expect("run serve --help");
    assert!(help.status.success());
    let stdout = String::from_utf8(help.stdout).unwrap();
    assert!(stdout.contains("--port"));
    assert!(stdout.contains("--bind"));

    for bad_args in [
        &["serve", "--port", "not-a-port"][..],
        &["serve", "--port=99999"][..],
        &["serve", "--unknown"][..],
        &["serve", "unexpected-positional"][..],
    ] {
        let output = Command::new(env!("CARGO_BIN_EXE_rwikitext"))
            .args(bad_args)
            .output()
            .expect("run invalid serve args");
        assert!(!output.status.success(), "expected failure for {bad_args:?}");
        assert!(!output.stderr.is_empty());
    }
}
