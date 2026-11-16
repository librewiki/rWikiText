# Compatibility baseline

The implementation baseline as of 2026-11-17 is [MediaWiki 1.46.2](https://www.mediawiki.org/wiki/Release_notes/1.46), including Parsoid v0.23.1 with [Parsoid HTML 2.8.0](https://www.mediawiki.org/wiki/Specs/HTML/2.8.0) as its default HTML version. The Rust tests render Markdown using the [published GitHub Flavored Markdown specification](https://github.github.com/gfm/).

Download and check the exact MediaWiki archive with Python 3.12 or newer, then run the selected upstream parser cases with Cargo.

```sh
python scripts/fetch-compatibility.py
cargo test --locked --test conversion pinned_parser_cases -- --ignored --nocapture
```

The tests compare normalized HTML against the expected Parsoid DOM in the release's parser fixtures. The `html/parsoid` section takes precedence over a shared `html` section when both are present. Normalization collapses HTML whitespace in the text and preserves paragraph boundaries. The fixture data is read from the verified archive.

| Coverage | Fixture file in MediaWiki 1.46.2 | Case titles |
| --- | --- | --- |
| Paragraphs | `tests/parser/parserTests.txt` | `Blank input`, `Simple paragraph` |
| Paragraph boundaries | `vendor/wikimedia/parsoid/tests/parser/pWrapping.txt` | `No block tags` |
