# rWikiText

rWikiText is a Rust library and command-line tool for converting documents between MediaWiki wikitext, NamuMark, GitHub Flavored Markdown (GFM), and HTML.

Use it to move wiki documents into Markdown or HTML workflows, import documents into a wiki, or embed conversion in an application. The library returns converted content, structured page metadata, and diagnostics that identify conversion losses by byte range in the original input.

NamuMark (나무마크) has a deliberately small first-release conversion profile with located diagnostics for unsupported syntax. Its syntax reference and independent hand-reviewed fixtures are pinned; live NamuWiki renderer compatibility remains unverified. See the [NamuMark contract and exclusions](docs/namumark.md) and [candidate verification](docs/compatibility.md#namumark-candidate-verification).

## Quick start

Build from a source checkout with a current Rust toolchain, Cargo, and the linker for your Rust target. The Rust library and CLI run locally with caller-supplied wiki data. Check the native binding guide for the additional tools each binding requires. [Native binding requirements](docs/native-bindings.md)

From the repository root, convert the supplied wikitext fixture to HTML.

```sh
cargo run --locked -- wikitext html tests/fixtures/paragraphs.wiki
```

Standard output contains two `<p>` elements. The second contains `Second paragraph &amp; more.`

## Conversion scope

The format pairs are MediaWiki wikitext ↔ GFM, MediaWiki wikitext ↔ HTML, and NamuMark ↔ each of the other three formats. Identity conversions and direct GFM ↔ HTML are unsupported. The following describes the MediaWiki conversions.

- Convert in both directions between wikitext and GFM or between wikitext and HTML. Each conversion takes wikitext as either its input or its output.
- Convert core document structure, including paragraphs, headings, links, emphasis, preformatted text, horizontal rules, lists, and tables. GFM uses native Markdown tables where possible and HTML for structures such as merged cells.
- Expand templates and supported core parser functions with explicit wiki context and caller-supplied data. HTML can carry Parsoid metadata that the importer validates to recover template calls and arguments.
- Return categories and their sort keys, redirects, and display titles as page metadata. Wikitext and HTML can carry these in the converted document. For GFM, the metadata is returned separately and diagnostics identify what saving the body alone would lose.
- Apply MediaWiki rules for elements, attributes, and URLs to embedded and imported HTML. Best-effort conversion reports losses and removals. Strict mode fails on conversion loss.

Compatibility is based on MediaWiki 1.46.2 and Parsoid HTML 2.8.0. Round-trip fidelity concerns rendered content, document structure, and page metadata. Equivalent wikitext can differ in spelling and whitespace. The first-release scope excludes MediaWiki extension tags and parser functions. See the verified cases and remaining differences in the [compatibility baseline](docs/compatibility.md) and [parser case inventory](docs/parser-case-inventory.md).

The NamuMark profile covers text, headings, bold/italic, links, escaping, literal code, top-level lists, line breaks, rules, categories, and redirects. Tables, footnotes, media, includes, and other macros are diagnosed as conversion losses. NamuMark input uses its own URL and page-title rules and makes no wiki data callbacks. Its reference fixtures are separate from the MediaWiki/Parsoid compatibility evidence. [NamuMark details](docs/namumark.md)

## Usage

### Rust library

Add a path dependency under `[dependencies]` in your application's `Cargo.toml`. This example assumes the rWikiText checkout is next to your application directory.

```toml
rwikitext = { path = "../rWikiText" }
```

Put this in your application's `src/main.rs` and run `cargo run` from its root. It prints `<p>Hello <b>wiki</b></p>`.

```rust
use rwikitext::{convert, ConversionRequest, Format, WikiContext};

fn main() -> Result<(), rwikitext::ConversionError> {
    let context = WikiContext::default();
    let result = convert(ConversionRequest {
        source: Format::Wikitext,
        target: Format::Html,
        input: "Hello '''wiki'''",
        context: &context,
    })?;
    println!("{}", result.content);
    Ok(())
}
```

Use `Format::NamuMark` to select NamuMark. Use `convert_with_mode` with `ConversionMode::Strict` to reject conversion loss. Use `convert_with_options` to supply a `WikiData` implementation and adjust `ExpansionLimits`. For MediaWiki input, the library calls back synchronously on the calling thread for template text, page existence, and file information. A lookup can return data, report missing data, or fail the conversion with a located diagnostic. [Rust API](src/lib.rs) · [Callbacks and expansion limits](src/wiki_data.rs)

### Command-line tool

Pass the source and target formats as `wikitext`, `namumark`, `gfm`, or `html`, using a supported pair above. Omit the file path or use `-` to read standard input. The CLI writes converted content to standard output and diagnostics to standard error. Add `--strict` to exit with status 1 on conversion loss.

Use `--wiki-data` to load offline wiki lookups from a JSON file. The optional `context`, `templates`, `pages`, and `files` objects supply wiki context, template text, page-existence booleans, and file records with a URL and optional dimensions in pixels. Omit a lookup title or use `null` for missing data. Use an object with an `error` string to simulate a lookup failure. [Example wiki data](tests/fixtures/wiki-data.json)

`--page-title`, `--site-language`, and `--reference-time` override the corresponding JSON context fields. The defaults are an empty page title, `en`, and `1970-01-01T00:00:00Z`.

From the repository root, expand a nested template using the example wiki data. The HTML renders `example` in bold and includes `data-mw` transclusion metadata.

```sh
echo "{{Outer|example}}" | cargo run --locked -- wikitext html - --wiki-data tests/fixtures/wiki-data.json
```

### Native bindings

The version 1 C ABI exposes the same conversion core to C, Python, PHP, Java, Kotlin/Native, Node.js, and Go. The first-release verification matrix targets Linux x86-64 and ARM64 with glibc or musl. Kotlin/Native targets glibc in that matrix. The Go binding has its own build and verification instructions. [Native binding build and callback examples](docs/native-bindings.md) · [Target matrix](docs/adr/0002-versioned-c-abi-and-wiki-data-callbacks.md) · [Go binding](bindings/go/README.md)

The first public release and both Linux runtime checks are planned. [glibc verification](https://github.com/librewiki/rWikiText/issues/26) · [musl verification](https://github.com/librewiki/rWikiText/issues/27) · [First public release](https://github.com/librewiki/rWikiText/issues/28)

## Development and testing

Run these commands from the repository root.

```sh
cargo build --locked
cargo test --locked
```

The default suite exercises conversion, round trips, diagnostics, and wiki data callbacks through the Rust API and CLI. The full compatibility suite also uses a verified MediaWiki archive and a local PHP runtime. [Full compatibility test instructions](docs/compatibility.md#first-release-verification)

The CLI and C ABI call the conversion pipeline in [`src/lib.rs`](src/lib.rs). The pipeline parses input into internal document blocks, renders them in the target format, and returns the result with page metadata and diagnostics. For wikitext input it also expands templates and resolves links using the caller's wiki data. See the [domain glossary](GLOSSARY.md) and [conversion design](docs/adr/0001-parsoid-and-document-fidelity.md).

For contributions, follow the repository's issue, branch, and pull request workflow. [Contributor and agent instructions](AGENTS.md) · [Work lifecycle](docs/agents/issue-tracker.md#work-lifecycle)

## License

rWikiText's code is licensed under the [MIT License](LICENSE). The supplied NamuWiki help snapshot is separately licensed under CC BY-NC-SA 2.0 KR; see [third-party notices](docs/THIRD_PARTY_NOTICES.md).
