# Compatibility baseline

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

Use a PHP executable with the extensions listed above and run the complete suite, including the tests that require local upstream files.

```sh
python scripts/fetch-compatibility.py
cargo check --locked --all-targets
cargo test --locked -- --include-ignored --nocapture
```

The [pinned parser case inventory](parser-case-inventory.md) records the source cases and the scope of the upstream comparison.

The 2026-11-17 run on Windows x86-64 used Rust 1.100.0 and PHP 8.4.27. The archive checksum, all-target type check, and full suite passed with 154 tests and zero ignored. The forward corpus audit wrote 2,670 case records, with 242 scope exclusions and 581 DOM differences to review. The inventory explains each status and records confirmed core gaps.

| Release contract | Verification |
| --- | --- |
| Four conversion directions and both wikitext round trips | `conversion` and `release_documents` exercise the Rust API and CLI. They compare the GFM structures used by the fixtures, including native tables and permitted HTML fallback, and use pinned Parsoid to render the starting and recovered wikitext under the same wiki context. |
| Format restrictions | `conversion` checks that the API and CLI reject each of the other five format pairs. |
| Page metadata and GFM body-only loss | `metadata` and `release_documents` check structured metadata, HTML round-trip preservation, and located loss diagnostics for a standalone GFM body. |
| HTML policy, invalid Parsoid metadata, and strict mode | `conversion`, `tables`, and `parsoid_transclusion` check element, attribute, and URL handling, general HTML import, validated template recovery, and errors with original input byte ranges. Strict mode rejects conversion loss and still reports HTML removals that preserve document meaning. CLI tests check output and diagnostics on their separate streams. |
| Callback outcomes and reproducibility | `transclusion`, `wiki_links`, and `functions` cover synchronous caller-thread lookups, found and missing data, fatal lookup errors, and repeatable CLI conversions with local wiki data. |
| Expansion bounds | `transclusion` checks separate depth, count, and UTF-8 output-byte limits with located diagnostics. It also checks an expansion that reaches the configured limits and nested parameter expansion with empty output. |
