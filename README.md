# rWikiText

rWikiText converts MediaWiki wikitext to and from GitHub Flavored Markdown and HTML. It currently supports plain paragraphs and headings at levels one through six.

```sh
cargo run -- wikitext html input.wiki
cargo run -- html wikitext - < input.html
```

Use `wikitext`, `gfm`, or `html` as the source and target, with wikitext on one side of each conversion. The CLI reads standard input when the path is omitted or is `-`. It writes converted content to standard output and diagnostics to standard error. Set the wiki context with `--page-title`, `--site-language`, and `--reference-time`.

The `rwikitext::convert` Rust function accepts a `ConversionRequest` containing the source, target, input, and wiki context. The result includes the converted content, page metadata, and diagnostics.

See [the compatibility baseline](docs/compatibility.md) for the pinned upstream tests.
