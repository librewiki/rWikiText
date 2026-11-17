# Compatibility baseline

The implementation baseline as of 2026-11-17 is [MediaWiki 1.46.2](https://www.mediawiki.org/wiki/Release_notes/1.46), including Parsoid v0.23.1 with [Parsoid HTML 2.8.0](https://www.mediawiki.org/wiki/Specs/HTML/2.8.0) as its default HTML version. The Rust tests render Markdown using the [published GitHub Flavored Markdown specification](https://github.github.com/gfm/).

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

Run the mixed-document workflow with PHP 8.3 or newer and the DOM, Intl, and Mbstring extensions enabled. The test uses `php` from `PATH` by default. Set `RWIKITEXT_PHP` to use a specific executable. The PHP renderer uses the bundled Parsoid and its local test site configuration.

```sh
cargo test --locked -- --include-ignored --nocapture
```

The complete fixtures in `tests/fixtures/milestone.*` cover all four conversions and both wikitext round trips through the Rust API and CLI. The test renders the starting and recovered wikitext with the pinned Parsoid, using the same English test wiki, page title, and reference time. It compares the resulting structure and the intermediate GFM and HTML with the expected HTML fixture. CLI tests also exercise file and standard input and check standard output and standard error.
