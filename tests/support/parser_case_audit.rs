use super::{document_structure, read_fixture, render_wikitext, Fixture, WikiContext, RELEASE};
use serde_json::{json, Value};
use std::collections::BTreeMap;
use std::io::{BufWriter, Write};
use std::path::Path;
use std::process::Command;

#[test]
#[ignore = "requires the verified MediaWiki archive and PHP; see docs/parser-case-inventory.md"]
fn pinned_forward_corpus_audit() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR"));
    let php = std::env::var_os("RWIKITEXT_PHP").unwrap_or_else(|| "php".into());
    let fixtures = Command::new(php)
        .arg(root.join("tests/read-parser-cases.php"))
        .arg(RELEASE)
        .output()
        .expect("run upstream parser fixture grammar");
    assert!(
        fixtures.status.success(),
        "{}",
        String::from_utf8_lossy(&fixtures.stderr)
    );

    let path = root.join("target/parser-case-audit.ndjson");
    let mut report = BufWriter::new(std::fs::File::create(&path).unwrap());
    let mut totals = BTreeMap::new();
    let mut file_count = 0;
    for file in String::from_utf8(fixtures.stdout).unwrap().lines() {
        let file: Value = serde_json::from_str(file).unwrap();
        let fixture_path = file["path"].as_str().unwrap();
        let source = read_fixture(fixture_path);
        let wiki = Fixture::parse(&source);
        for case in file["cases"].as_array().unwrap() {
            let (status, detail) = audit_case(fixture_path, &file["options"], case, &wiki);
            *totals.entry(status).or_insert(0) += 1;
            writeln!(
                report,
                "{}",
                json!({
                    "path": fixture_path,
                    "line": case["lineNumStart"],
                    "title": case["testName"],
                    "status": status,
                    "detail": detail,
                })
            )
            .unwrap();
        }
        file_count += 1;
    }
    report.flush().unwrap();
    assert_eq!(file_count, 41);
    assert_eq!(totals.values().sum::<usize>(), 2670);
    assert_eq!(totals.get("conversion-error").copied().unwrap_or(0), 0);
    println!("{} {totals:?}", path.display());
}

fn audit_case(
    path: &str,
    file_options: &Value,
    case: &Value,
    wiki: &Fixture<'_>,
) -> (&'static str, String) {
    let options = &case["options"];
    let input = case["wikitext"].as_str().unwrap_or_default();
    if let Some(reason) = exclusion(path, case, input) {
        return ("outside-release", reason.into());
    }
    if let Some(modes) = options["parsoid"]["modes"].as_array() {
        if !modes.is_empty() && !modes.iter().any(|mode| mode == "wt2html") {
            return ("other-mode", format!("Upstream modes {modes:?}"));
        }
    }
    let standalone_expected = ["html/parsoid+standalone", "html/parsoid", "html", "html/*"]
        .into_iter()
        .find_map(|key| case[key].as_str().map(|html| (key, html, false)));
    let fallback_expected = ["html/parsoid+integrated", "html/php"]
        .into_iter()
        .find_map(|key| case[key].as_str().map(|html| (key, html, true)));
    let (Some(input), Some((oracle, expected, fallback_oracle))) = (
        case["wikitext"].as_str(),
        standalone_expected.or(fallback_expected),
    ) else {
        return (
            "needs-oracle",
            "Missing ordinary forward input or a standalone/shared HTML expectation".into(),
        );
    };
    let unsupported = [
        (file_options, &["version", "parsoid-compatible"][..]),
        (options, &["title", "language", "parsoid", "subpage"][..]),
        (
            &options["parsoid"],
            &["modes", "changes", "selser", "normalizePhp"][..],
        ),
    ];
    for (settings, known) in unsupported {
        if let Some(key) = settings
            .as_object()
            .and_then(|settings| settings.keys().find(|key| !known.contains(&key.as_str())))
        {
            return ("needs-context", format!("Unmodeled fixture setting {key}"));
        }
    }
    if let Some(key) = case["config"].as_object().and_then(|config| {
        config.keys().find(|key| {
            key.as_str() != "wgParsoidExperimentalParserFunctionOutput" || config[*key] != false
        })
    }) {
        return ("needs-context", format!("Unmodeled fixture setting {key}"));
    }
    if expected.contains("<img") || expected.contains("<video") || expected.contains("<audio") {
        return (
            "needs-context",
            "The probe does not reproduce upstream media fixtures".into(),
        );
    }
    let context = WikiContext {
        page_title: options["title"].as_str().unwrap_or("Parser test"),
        site_language: options["language"].as_str().unwrap_or("en"),
        reference_time: "1970-01-01T00:02:03Z",
    };
    let actual = match render_wikitext(input, &context, wiki) {
        Ok(actual) => actual,
        Err(error) => return ("conversion-error", error.to_string()),
    };
    if document_structure(&actual.content) != document_structure(expected) {
        if fallback_oracle {
            return (
                "needs-oracle",
                format!("Has only an integrated Parsoid or legacy PHP expectation ({oracle})"),
            );
        }
        return ("dom-differs", format!("Compared against {oracle}"));
    }
    if !actual.diagnostics.is_empty() {
        return (
            "dom-matches-with-diagnostics",
            format!("Compared against {oracle}, {:?}", actual.diagnostics),
        );
    }
    ("dom-matches", format!("Compared against {oracle}"))
}

fn exclusion(path: &str, case: &Value, input: &str) -> Option<&'static str> {
    let options = &case["options"];
    let file = path.rsplit('/').next().unwrap();
    let lower = input.to_ascii_lowercase();
    let upper = input.to_ascii_uppercase();
    if matches!(
        file,
        "annotationParserTests.txt"
            | "timedMediaHandlerParserTests.txt"
            | "i18nParserTests.txt"
            | "indicators.txt"
            | "treeBuilder.txt"
            | "tableFixupsParserTests.txt"
    ) || (file == "pFragmentHandlerTests.txt" && case["testName"] != "Leaking fragment marker")
        || (file == "encapParserTests.txt"
            && case["html/parsoid"]
                .as_str()
                .is_some_and(|html| html.contains("mw:Extension/pwraptest")))
        || [
            "<indicator",
            "<ref",
            "<references",
            "<gallery",
            "<poem",
            "<section",
            "<templatestyles",
            "<syntaxhighlight",
            "<source",
            "<math",
            "<score",
            "<spantag",
            "<divtag",
            "<tag",
            "<statictag",
            "<sealtag",
            "<asidetag",
            "<embedtag",
            "<translate",
            "<tvar",
            "<tåg",
            "{{t1}}",
            "{{#tag:indicator",
            "{{#tag:ref",
        ]
        .iter()
        .any(|tag| lower.contains(tag))
    {
        return Some("Requires an extension or fixture-only hook");
    }
    if file == "stringFunctionTests.txt"
        || (file == "funcsParserTests.txt"
            && ["#time", "#titleparts", "#iferror", "#ifexpr", "#expr"]
                .iter()
                .any(|word| lower.contains(word)))
    {
        return Some("Uses a ParserFunctions extension hook outside the supported subset");
    }
    if case["config"]["wgParsoidExperimentalParserFunctionOutput"] == true {
        return Some("Requires experimental Parsoid HTML v3.x");
    }
    if options.get("pst").is_some()
        || (file == "pst.txt" && lower.contains("subst"))
        || lower.contains("{{subst:")
        || lower.contains("{{safesubst:")
    {
        return Some("Tests the pre-save transformation");
    }
    if file == "selserWrappingParserTests.txt"
        || options["parsoid"]["modes"]
            .as_array()
            .is_some_and(|modes| !modes.is_empty() && modes.iter().all(|mode| mode == "selser"))
    {
        return Some("Tests selective serialization");
    }
    if case["testName"] == "Template infinite loop" {
        return Some("Expects a MediaWiki loop marker where rWikiText returns an expansion error");
    }
    if file == "magicLinks.txt"
        || ["RFC ", "PMID ", "ISBN "]
            .iter()
            .any(|word| input.contains(word))
        || lower.contains("{{#isbn:")
    {
        return Some("Uses RFC, PMID, or ISBN magic links outside the release scope");
    }
    if file == "interwikiLinks.txt"
        || [
            "{{#interwikilink:",
            "{{#interlanguagelink:",
            "[[es:",
            "[[fr:",
            "[[zh:",
            "[[ko:",
            "[[de:",
            "[[wikipedia:",
            "[[meatball:",
            "[[:en:",
            "[[meta:",
            "[[memoryalpha:",
        ]
        .iter()
        .any(|prefix| lower.contains(prefix))
    {
        return Some("Uses interwiki or interlanguage links outside the release scope");
    }
    if input.contains("-{") {
        return Some("Uses MediaWiki language converter markup outside the release scope");
    }
    if [
        "__TOC__",
        "__NOTOC__",
        "__FORCETOC__",
        "__NOEDITSECTION__",
        "__NEWSECTIONLINK__",
        "__NONEWSECTIONLINK__",
        "__HIDDENCAT__",
        "__INDEX__",
        "__NOINDEX__",
        "__STATICREDIRECT__",
        "__NOGLOBALS__",
        "__DISAMBIG__",
        "＿＿",
    ]
    .iter()
    .any(|word| upper.contains(word))
    {
        return Some(
            "Uses behavior switches or automatic table-of-contents generation outside the release scope",
        );
    }
    let language = options["language"].as_str().unwrap_or("en");
    if [
        "{{server",
        "{{sitename",
        "{{scriptpath",
        "{{stylepath",
        "{{numberof",
        "{{pagesincategory",
        "{{pagesize",
        "{{protection",
        "{{cascadingsources",
        "{{revision",
        "{{pageid",
        "{{userlanguage",
        "{{directionmark",
        "{{dirmark",
        "{{currentversion",
        "{{defaultsort",
        "{{localurl",
        "{{fullurl",
        "{{canonicalurl",
        "{{filepath",
        "{{int:",
        "{{formatnum",
        "{{grammar",
        "{{gender",
        "{{plural",
        "{{formal",
        "{{bidi",
        "{{#language",
        "{{#dir",
        "{{#bcp47",
        "{{#special",
        "{{#formatdate",
        "{{#contentmodel",
        "#tilvísun",
    ]
    .iter()
    .any(|word| lower.contains(word))
        || (language != "en"
            && [
                "{{ns:",
                "{{nse:",
                "{{ucfırst:",
                "{{první",
                "[[miðill:",
                "[[تصنيف:",
            ]
            .iter()
            .any(|word| lower.contains(word)))
    {
        return Some(
            "Uses site configuration, runtime metadata, or localized magic words outside WikiContext",
        );
    }
    None
}
