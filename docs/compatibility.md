# Compatibility baseline

## Implemented MediaWiki baseline

The historical results below cover their recorded MediaWiki candidate. The expanded NamuMark contract and its independently authored expectations are documented separately in the [NamuMark contract](namumark.md) and [candidate verification](#namumark-candidate-verification); the historical MediaWiki results do not establish NamuMark compatibility.

The implementation baseline as of 2026-11-17 is [MediaWiki 1.46.2](https://www.mediawiki.org/wiki/Release_notes/1.46), including Parsoid v0.23.1 with [Parsoid HTML 2.8.0](https://www.mediawiki.org/wiki/Specs/HTML/2.8.0) as its default HTML version. The Rust tests render Markdown against the [GitHub Flavored Markdown specification, version 0.29-gfm](https://github.github.com/gfm/).

Download and check the exact MediaWiki archive with Python 3.12 or newer, then run the selected upstream parser cases with Cargo.

```sh
python scripts/fetch-compatibility.py
cargo test --locked --test conversion pinned_parser_cases -- --ignored --nocapture
```

The tests compare normalized HTML against the expected DOM in the release's parser fixtures. The `html/parsoid` section takes precedence when available. For legacy heading fixtures, the comparison takes the heading from its MediaWiki editing wrapper. Normalization reads Parsoid entity wrappers as text and collapses HTML whitespace. It preserves paragraph boundaries, the heading outline, link labels and destinations, and list type and item order. The fixture data is read from the verified archive.

| Coverage | Fixture file in MediaWiki 1.46.2 | Case titles |
| --- | --- | --- |
| Paragraphs | `tests/parser/parserTests.txt` | `Blank input`, `Simple paragraph` |
| Paragraph boundaries | `vendor/wikimedia/parsoid/tests/parser/pWrapping.txt` | `No block tags` |
| Heading outline | `vendor/wikimedia/parsoid/tests/parser/headings.txt` | `Basic section headings` |
| Internal link title normalization | `tests/parser/parserTests.txt` | `Multiple spaces in titles should normalize to a single underscore` |
| Named and bare external links | `vendor/wikimedia/parsoid/tests/parser/extLinks.txt` | `External links: specified text`, `External links: non-bracketed`, `External links: Free with trailing punctuation` |
| Ordered and unordered lists | `tests/parser/parserTests.txt` | `Simple list`, `Common list`, `Numbered list` |

The `core`, `tables`, and `metadata` integration tests exercise additional pinned cases alongside API and CLI round trips. The core cases load nested and definition lists and horizontal rules from `tests/parser/parserTests.txt`, and compare apostrophe runs and preformatted text with `vendor/wikimedia/parsoid/tests/parser/quotes.txt` and `indentPre.txt`. Table cases cover `Simple table`, `Table rowspan`, and `Nested table` from `tests/parser/tables.txt`. Metadata cases cover category sort keys, redirects with and without the optional colon, and plain and styled display titles.

HTML policy tests cover safe attributes and URLs, prohibited elements and attributes, and malformed markup through general HTML, GFM, and embedded wikitext HTML. Imported HTML is repaired as a fragment. Parsoid's wikitext literal-tag rules preserve unsupported tags as visible text where applicable.

The mixed-document workflow needs PHP 8.3 or newer with DOM, Intl, and Mbstring enabled. The test uses `php` from `PATH` by default. Set `RWIKITEXT_PHP` to use a specific executable. The PHP renderer uses the bundled Parsoid and its local test site configuration.

The complete fixtures in `tests/fixtures/milestone.*` cover all four conversions and both wikitext round trips through the Rust API and CLI. The test renders the starting and recovered wikitext with the pinned Parsoid, using the same English test wiki, page title, and reference time. It compares the resulting structure and the intermediate GFM and HTML with the expected HTML fixture. CLI tests also exercise file and standard input and check standard output and standard error.

The `transclusion`, `wiki_links`, `functions`, and `parsoid_transclusion` integration suites exercise caller-thread lookups, the three lookup outcomes, each configurable expansion limit, and CLI conversion using `tests/fixtures/wiki-data.json`. They compare selected release fixtures for template inclusion, red links, image sizing and caption behavior, magic words, and bundled conditional functions. The Parsoid comparisons include template calls with a computed name, list transclusion, missing templates, and the `data-mw` for core padding functions.

The verified parser-function subset covers Unicode case conversion, padding, template-syntax escape words, page-title and language variables, UTC calendar variables, `#tag:nowiki`, `#tag:pre`, and the bundled `#if`, `#ifeq`, `#ifexist`, and `#switch` functions. Calendar names use the pinned standalone Parsoid handlers' English spelling, including the same month name for `CURRENTMONTHNAME` and `CURRENTMONTHNAMEGEN`. MediaWiki's integrated parser supplies localized date names through its page language.

HTML template recovery supports standalone and inline transclusion groups with validated targets and arguments. Transclusions that generate partial tags, link targets, or combined blocks can render as content with a conversion-loss diagnostic for the invocation.

## First-release verification

This section records the existing MediaWiki verification procedure and historical results. The expanded release also requires NamuMark verification and binding checks against the resulting candidate, as tracked in [#49](https://github.com/librewiki/rWikiText/issues/49), [#26](https://github.com/librewiki/rWikiText/issues/26), and [#27](https://github.com/librewiki/rWikiText/issues/27).

Use a PHP executable with the extensions listed above and run the complete suite, including the tests that require local upstream files.

```sh
python scripts/fetch-compatibility.py
cargo check --locked --all-targets
cargo test --locked -- --include-ignored --nocapture
```

The [pinned parser case inventory](parser-case-inventory.md) records the source cases and the scope of the upstream comparison.

The 2026-11-17 verification run on Linux x86_64 used Rust 1.100.0 and PHP 8.4.15. The archive checksum, all-target type check, and full suite passed with all tests and zero ignored. The forward corpus audit wrote 2,670 case records across 41 files (695 DOM matches, 102 matches with reported conversion loss, 827 verified outside-release cases, 381 context-dependent cases, 234 alternate-mode cases, 75 oracle-dependent cases, and 356 DOM differences to review). The inventory explains each status and documents the resolved core compatibility gaps.

| Release contract | Verification |
| --- | --- |
| Four conversion directions and both wikitext round trips | `conversion` and `release_documents` exercise the Rust API and CLI. They compare the GFM structures used by the fixtures, including native tables and permitted HTML fallback, and use pinned Parsoid to render the starting and recovered wikitext under the same wiki context. |
| Format restrictions | `conversion` checks that the API and CLI reject each of the other five format pairs. |
| Page metadata and GFM body-only loss | `metadata` and `release_documents` check structured metadata, HTML round-trip preservation, and located loss diagnostics for a standalone GFM body. |
| HTML policy, invalid Parsoid metadata, and strict mode | `conversion`, `tables`, and `parsoid_transclusion` check element, attribute, and URL handling, general HTML import, validated template recovery, and errors with original input byte ranges. Strict mode rejects conversion loss and still reports HTML removals that preserve document meaning. CLI tests check output and diagnostics on their separate streams. |
| Callback outcomes and reproducibility | `transclusion`, `wiki_links`, and `functions` cover synchronous caller-thread lookups, found and missing data, fatal lookup errors, and repeatable CLI conversions with local wiki data. |
| Expansion bounds | `transclusion` checks separate depth, count, and UTF-8 output-byte limits with located diagnostics. It also checks an expansion that reaches the configured limits and nested parameter expansion with empty output. |

## NamuMark candidate verification

Baseline v1 was fixed before core implementation on 2026-11-24 for [#47](https://github.com/librewiki/rWikiText/issues/47). [#48](https://github.com/librewiki/rWikiText/issues/48) adds the six NamuMark directions through shared Rust, CLI, C ABI, and bindings; [#49](https://github.com/librewiki/rWikiText/issues/49) tracks candidate evidence. Existing four directions remain unchanged in scope. Identity and direct GFM ↔ HTML remain unsupported.

The syntax reference is the supplied r927/2026-11-24 help snapshot, whose local SHA-256 is `520eedc1f02128f0e4eb35c044bdd466ad7e9fc51458b565942ca0b681f9b608`. The revision identification was supplied by the user and has not been independently checked online. The [baseline manifest](namumark-baseline.json) and [original test fixtures](../tests/fixtures/namumark-baseline.provenance.json) contain hand-authored expected meaning, fixed independently of rWikiText output. They are a snapshot-derived oracle; no real NamuMark renderer execution or captured render comparison is claimed. The [contract](namumark.md) lists exclusions and the independent NamuMark context, URL, loss, and strict-mode policies.

MediaWiki-side comparisons use the existing pinned MediaWiki 1.46.2 archive, SHA-256 `8f7f937f8bbc1acd4cef1c5362a95f248dcd242020f2588798eb2f73ac2e69ba`, bundled Parsoid v0.23.1, and HTML 2.8.0. GFM output is checked with the existing `pulldown-cmark` test dependency against a fixed expected semantic tree under the published GFM reference. These are separate kinds of evidence; Parsoid does not establish NamuMark interpretation.

The source candidate is `17b2ce0b9352ebc62e51534a0c037217716bfb91`; the evidence below applies to that source revision and excludes the subsequent documentation-only evidence commit. On 2026-11-24, Windows x86-64 verification used Rust `1.100.0 (0ef1bc386 2026-11-09)` and PHP 8.4.27 VC2022 x64 with DOM, Intl, and Mbstring enabled.

Reproduce the source checks with the following commands. The ignored bridge test requires the verified archive and a PHP runtime with the extensions described above; set `RWIKITEXT_PHP` to the absolute executable path when PHP is not available on `PATH`.

```sh
cargo check --locked --all-targets
cargo test --locked --test namumark --test namumark_cli --test namumark_reference
cargo test --locked --test namumark_reference pinned_parsoid_confirms_namumark_mediawiki_bridge -- --ignored --nocapture
cargo test --locked -- --include-ignored --nocapture
```

```powershell
Get-FileHash docs/namumark_manual_r927_20261124.txt -Algorithm SHA256
```

The Windows verification run selected its local PHP runtime as follows before the complete test run. This runtime path belongs to the verification environment; a source checkout does not supply a PHP installation.

```powershell
$env:RWIKITEXT_PHP = (Resolve-Path target/tooling/php/php.exe).Path
cargo test --locked -- --include-ignored --nocapture
```

The all-target check passed. The complete suite passed with **210 tests, zero failures, and zero ignored**, including the actual Parsoid bridge comparison and all existing MediaWiki regression tests. The generated local logs are `target/namumark-check-final.log` and `target/namumark-full-test.log`; they are not tracked repository artifacts.

The independent reference suite exercises all six API directions against original fixtures, round trips starting in NamuMark and each other format, raw newline / `[br]` / CRLF semantics, category and redirect metadata, GFM body-only loss, and strict mode. A separate CLI test converts the original baseline fixture files through all six directions and compares their document meaning and metadata with the independently authored expectations. Its bridge test, ignored by default but executed in this full run, renders original MediaWiki, NamuMark-to-MediaWiki output, and a MediaWiki-origin round trip with actual pinned Parsoid. The fixture explicitly declares `Orchard` an existing page through the helper's optional known-page context, as recorded in the provenance manifest; the comparison does not strip missing-page edit-link query parameters to force agreement. Additional core and CLI suites exercise unsupported/malformed input, located UTF-8 diagnostics, format restrictions, URL/context rules, and absence of NamuMark callbacks.

The full-run corpus audit classified 2,670 records: 695 DOM matches, 102 matches with diagnostics, 356 DOM differences, 381 context-dependent cases, 75 oracle-dependent cases, 234 alternate-mode cases, and 827 outside-release cases. A passing audit verifies those classifications and supported comparisons, not full MediaWiki conformance or resolution of every DOM difference. The historical baseline and [parser case inventory](parser-case-inventory.md) retain the scope and unresolved corpus limits.

| Candidate evidence | Status |
| --- | --- |
| Exact expanded source candidate commit | `17b2ce0b9352ebc62e51534a0c037217716bfb91`; subsequent documentation-only commit excluded |
| Snapshot SHA-256 | Verified locally on 2026-11-24; exact hash above |
| Rust all-target check and API/CLI baseline suites | Passed on Windows x86-64 with the toolchain above |
| Actual pinned Parsoid NamuMark/MediaWiki bridge comparison | Passed in the complete run; ignored only when running the default suite |
| Complete existing MediaWiki regression suite including ignored cases | Passed; complete suite 210 passed, zero failed, zero ignored |
| C ABI and Python/Go/Java examples on Linux GNU x86-64 | Passed against the identical production tree described below; Go package tests also passed. Rust 1.100.0, GCC 12.2.0, Python 3.11.2, Go 1.26.11, and Temurin JDK 21.0.9 |
| Other bindings, Linux ARM64, and musl candidate matrix | Not established by this local run; [#26](https://github.com/librewiki/rWikiText/issues/26) and [#27](https://github.com/librewiki/rWikiText/issues/27) remain release gates |
| Live NamuWiki / independent executable NamuMark renderer | Not run; no renderer pinned |

The Linux GNU x86-64 native rerun built `68a26a44c51ebec692a23b32e013f05bc17de060` into `target/linux-gnu/release`. The final source candidate adds a CLI fixture verification test; `git diff --exit-code 68a26a44c51ebec692a23b32e013f05bc17de060 17b2ce0b9352ebc62e51534a0c037217716bfb91 -- src Cargo.toml Cargo.lock bindings include examples` passed, confirming identical production inputs. The native artifacts were not rebuilt at the test-only successor. C and Python examples, JNI compilation, Java compilation/execution, and the Go example/package tests exited successfully. The local logs are `target/namumark-bindings-final.log`, `target/namumark-bindings-c-python-jni-final.log`, `target/namumark-bindings-java-execution-final.log`, and `target/namumark-bindings-go-final.log`; these generated files are not tracked artifacts. The [native binding guide](native-bindings.md), [Python procedure](../bindings/python/README.md), [Java procedure](../bindings/java/README.md), and [Go procedure](../bindings/go/README.md) provide the normal native build/run commands. This rerun verifies those runtimes on GNU x86-64 only; Node.js, PHP, and Kotlin/Native runtime execution, ARM64, and musl remain unverified on this candidate.

The rerun used `rust:bookworm` for the native library/C/Python, `gradle:8.14.3-jdk21` for Java, and `golang:1.26-bookworm` for Go, mounting the checkout at `/workspace`. These image tags identify the verification environments; they are not immutable image digests. The following native Linux commands reproduce the same build/output layout with matching installed tools. Java compilation needs a JDK including JNI headers. In the recorded run, those headers were copied into `target/java-gnu/jni-include` before the same JNI compile. The Go binding links the fixed `target/release/librwikitext.a` path, so copy the GNU archive there and force relinking after rebuilding it.

```sh
CARGO_TARGET_DIR=target/linux-gnu cargo build --locked --release --lib
cc -std=c11 -Wall -Wextra -Werror -Iinclude examples/convert.c \
  target/linux-gnu/release/librwikitext.a -ldl -lpthread -lm -o /tmp/convert-c
/tmp/convert-c
python3 bindings/python/example.py "$PWD/target/linux-gnu/release/librwikitext.so"

export JAVA_HOME="$(dirname "$(dirname "$(readlink -f "$(command -v javac)")")")"
mkdir -p target/java-gnu
cc -shared -fPIC -Iinclude -I"$JAVA_HOME/include" -I"$JAVA_HOME/include/linux" \
  bindings/java/native/rwikitext_jni.c -Ltarget/linux-gnu/release -lrwikitext \
  -Wl,-rpath,"$PWD/target/linux-gnu/release" -o target/java-gnu/librwikitext_jni.so
javac -encoding UTF-8 -d target/java-gnu \
  bindings/java/src/org/librewiki/rwikitext/WikiText.java bindings/java/Example.java
java -Djava.library.path=target/java-gnu -cp target/java-gnu Example

mkdir -p target/release
cp target/linux-gnu/release/librwikitext.a target/release/
cd bindings/go
CGO_ENABLED=1 go run -a ./example
CGO_ENABLED=1 go test -a ./...
```

The profile excludes tables, footnotes, media, includes, most macros, advanced formatting, folding, and other syntax listed in the contract; fallback literals plus diagnostics are not support for their rendered meaning. Output inline code additionally falls back with a loss for newline/closing-sequence collisions, terminal `}`, an odd trailing backslash count, or leading `#`. Preformatted blocks retain leading `#` and blank lines, but literal `}}}` in their content cannot be safely emitted and receives a loss. Best effort retains visible text while losing code/preformatted meaning; strict mode rejects the loss.

NamuMark input performs no lookups or template expansion. Its independent inline recursion bound is 32 levels; deeper content produces a located nesting-limit loss, rejected by strict mode. Caller-supplied MediaWiki expansion budgets do not control this bound. No general input-size or memory bound is claimed.

The candidate distinguishes NamuMark title spelling from MediaWiki normalization. HTML/GFM imports into NamuMark retain case, spaces, underscores, and escaped reserved title characters, and validated HTML metadata retains NamuMark spelling. NamuMark-to-MediaWiki title delimiter or namespace/normalization incompatibilities receive original-input loss locations. Unsafe metadata is rendered visibly and escaped while remaining available as structured metadata. Target-only losses may conservatively locate the entire original input where exact source ranges no longer survive the shared representation.

The NamuMark interpretation checks use the independent manual-derived oracle; the actual Parsoid execution establishes MediaWiki-side rendering evidence. Passing these checks does not establish full NamuWiki compatibility or complete the runtime/binding release matrix. Publication remains gated on the expanded candidate checks.
