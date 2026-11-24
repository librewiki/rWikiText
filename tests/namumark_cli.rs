use std::io::Write;
use std::process::{Command, Stdio};

#[test]
fn usage_lists_namumark_as_an_available_format() {
    let output = Command::new(env!("CARGO_BIN_EXE_rwikitext"))
        .output()
        .expect("run CLI");
    assert!(!output.status.success());
    assert!(String::from_utf8(output.stderr)
        .unwrap()
        .contains("wikitext|gfm|html|namumark"));
}

#[test]
fn namumark_converts_in_both_directions_via_stdin_and_file() {
    let cases = [
        ("namumark", "wikitext", "안녕 world", "안녕 world"),
        ("wikitext", "namumark", "안녕 world", "안녕 world"),
        ("namumark", "gfm", "안녕 world", "안녕 world"),
        ("gfm", "namumark", "안녕 world", "안녕 world"),
        ("namumark", "html", "안녕 world", "<p>안녕 world</p>"),
        ("html", "namumark", "<p>안녕 world</p>", "안녕 world"),
    ];
    let file = std::env::temp_dir().join(format!("rwikitext-namumark-{}.txt", std::process::id()));
    for (source, target, input, expected) in cases {
        let mut child = Command::new(env!("CARGO_BIN_EXE_rwikitext"))
            .args([source, target, "-"])
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .unwrap();
        child
            .stdin
            .take()
            .unwrap()
            .write_all(input.as_bytes())
            .unwrap();
        let result = child.wait_with_output().unwrap();
        assert!(
            result.status.success(),
            "{}",
            String::from_utf8_lossy(&result.stderr)
        );
        assert!(
            result.stderr.is_empty(),
            "{}",
            String::from_utf8_lossy(&result.stderr)
        );
        assert_eq!(String::from_utf8(result.stdout).unwrap().trim(), expected);
        std::fs::write(&file, input).unwrap();
        let result = Command::new(env!("CARGO_BIN_EXE_rwikitext"))
            .args([source, target])
            .arg(&file)
            .output()
            .unwrap();
        assert!(
            result.status.success(),
            "{}",
            String::from_utf8_lossy(&result.stderr)
        );
        assert!(
            result.stderr.is_empty(),
            "{}",
            String::from_utf8_lossy(&result.stderr)
        );
        assert_eq!(String::from_utf8(result.stdout).unwrap().trim(), expected);
    }
    std::fs::remove_file(file).unwrap();
}

#[test]
fn strict_namumark_loss_reports_error_without_output() {
    let mut child = Command::new(env!("CARGO_BIN_EXE_rwikitext"))
        .args(["namumark", "html", "-", "--strict"])
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .unwrap();
    child
        .stdin
        .take()
        .unwrap()
        .write_all("가\r\n||cell||".as_bytes())
        .unwrap();
    let result = child.wait_with_output().unwrap();
    assert!(!result.status.success());
    assert!(result.stdout.is_empty());
    assert!(String::from_utf8(result.stderr)
        .unwrap()
        .contains("conversion loss at bytes 5..13:"));
}

#[test]
fn cli_rejects_identity_and_gfm_html_pairs() {
    for (source, target) in [("namumark", "namumark"), ("gfm", "html"), ("html", "gfm")] {
        let result = Command::new(env!("CARGO_BIN_EXE_rwikitext"))
            .args([source, target])
            .stdin(Stdio::null())
            .output()
            .unwrap();
        assert!(!result.status.success(), "{source} -> {target}");
        assert!(result.stdout.is_empty());
        assert!(!result.stderr.is_empty());
    }
}
