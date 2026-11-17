# rWikiText

rWikiText converts MediaWiki wikitext to and from GitHub Flavored Markdown and HTML. It supports paragraphs, headings, links, emphasis, preformatted text, horizontal rules, nested and definition lists, and core tables. GFM output uses native tables where possible and HTML for structures such as merged cells. Wiki links use Parsoid-style destinations such as `./Main_Page` and `#Local_section`.

```sh
cargo run -- wikitext html input.wiki
cargo run -- html wikitext - < input.html
cargo run -- wikitext gfm input.wiki --strict
```

Use `wikitext`, `gfm`, or `html` as the source and target, with wikitext on one side of each conversion. The CLI reads standard input when the path is omitted or is `-`. It writes converted content to standard output and diagnostics to standard error. Set the wiki context with `--page-title`, `--site-language`, and `--reference-time`.

The `rwikitext::convert` Rust function accepts a `ConversionRequest` containing the source, target, input, and wiki context. It returns best-effort content, page metadata, and diagnostics with half-open byte ranges in the input. Pass `ConversionMode::Strict` to `convert_with_mode`, or use `--strict` in the CLI, to fail when conversion would lose document meaning. HTML removals that leave document meaning intact are still reported.

Use `convert_with_options` to supply a `WikiData` implementation and configure `ExpansionLimits`. Template, page, and file lookups run synchronously on the thread calling `convert_with_options`. Callback titles use namespace spelling such as `Template:Example_name` and `File:Example.png`. Return `Ok(Some(data))` for known data, `Ok(None)` for missing data, or `Err(LookupError::new(message))` to abort conversion with a located diagnostic. Page existence also accepts `Ok(Some(false))`. The default expansion limits are 40 levels, 10,000 evaluations, and 2 MiB of expanded wikitext. Evaluations include template calls, parser functions, and parameter substitutions.

Use `--wiki-data` for reproducible CLI lookups. This JSON file contains optional `context`, `templates`, `pages`, and `files` objects. CLI context flags override values in `context`. Use `null` or omit a title for a missing result. Use an object with an `error` string for a lookup error. A file entry supplies a `url` and optional pixel `width` and `height`.

```sh
cargo run -- wikitext html --wiki-data tests/fixtures/wiki-data.json
```

Enter `{{Outer|example}}` to expand the nested template in the fixture. HTML can preserve a template invocation and its arguments in Parsoid metadata for HTML-to-wikitext conversion. Wiki file syntax resolves media URLs through the caller's data. HTML and GFM imports report a conversion loss when the caller's file data cannot reproduce the supplied URL.

The result exposes category names and their corresponding sort keys, redirects, and display titles. Wikitext and Parsoid HTML can carry this metadata in the document. GFM returns it separately and reports what would be lost if only the GFM body were saved.

See [the compatibility baseline](docs/compatibility.md) for the pinned upstream tests.

Build the versioned C ABI and run the C, Python, PHP, Java, Kotlin/Native, and Node.js examples using the [native binding guide](docs/native-bindings.md).

## Project Goals

### Document Conversion and Fidelity

- **Four conversion directions**: Support `Wikitext → GFM`, `GFM → Wikitext`, `Wikitext → HTML`, and `HTML → Wikitext`, as well as round trips (`Wikitext → GFM → Wikitext` and `Wikitext → HTML → Wikitext`).
- **Parsoid compatibility baseline**: Follow MediaWiki syntax documentation and use [Parsoid](https://www.mediawiki.org/wiki/Parsoid/MediaWiki_DOM_spec) to decide behavior where documentation and rendering differ ([ADR 0001](docs/adr/0001-parsoid-and-document-fidelity.md)). Round-trip fidelity preserves rendered content, document structure, and page metadata while allowing equivalent wikitext spelling and whitespace to vary.
- **Parsoid and general HTML support**: Emit HTML with Parsoid metadata such as `data-mw`. Accept both general HTML and Parsoid HTML on import, using validated metadata to reconstruct transclusions (template calls and arguments).
- **Standards-based GFM**: Target the published [GFM specification](https://github.github.com/gfm/), preferring native GFM syntax and falling back to GFM-permitted HTML for structures that native Markdown cannot express (such as tables with merged cells).
- **Page metadata and conversion-loss reporting**: Return structured page metadata—categories, redirect targets, and display titles—alongside converted output and encode it where the target format supports it. Report any **conversion loss** with its location and reason (including when storing a GFM body alone drops page metadata). Support both best-effort conversion and a strict mode that treats conversion loss as an error.
- **Safety and resource bounds**: Enforce MediaWiki's allowed HTML elements, attributes, and URL policies on embedded and imported HTML, diagnosing removed content. Bound template expansion depth, expansion count, and output size.

### Architecture, CLI, and Language Bindings

- **Caller-supplied wiki data**: Conversions run against an explicit **wiki context** (page title, site language, and reference time) and synchronous caller callbacks on the invoking thread for template text, page existence, and file information ([ADR 0002](docs/adr/0002-versioned-c-abi-and-wiki-data-callbacks.md)). Callbacks distinguish data found, data missing (following Parsoid behavior), and lookup error (aborting the conversion so the caller can decide whether to retry), supporting both live wiki integrations and reproducible offline conversions.
- **Rust API and CLI**: Expose a single document conversion core in Rust and a CLI that reads from a file or standard input, serves wiki context and lookup data from local fixtures, writes converted content to standard output, and writes diagnostics to standard error.
- **Versioned C ABI and native bindings**: Ship the Rust core as a native static archive with a versioned C ABI and header shared across language bindings:
  - **C** and **Kotlin/Native** link the static archive through the C header.
  - **Python** and **Java** load shared native code.
  - **PHP** uses a native extension.
  - **Node.js** uses a Node-API addon.

| Linux C runtime | Architectures | Supported bindings |
| --- | --- | --- |
| `glibc` | `x86-64`, `ARM64` | C, Python, PHP, Java, Kotlin/Native, Node.js |
| `musl` | `x86-64`, `ARM64` | C, Python, PHP, Java, Node.js |

## Current Status

rWikiText has a Rust conversion API, test CLI, and native bindings with caller-supplied wiki data. The first-release roadmap tracks further MediaWiki compatibility and Linux runtime verification.

### Completed

- **Domain model and vocabulary**: Defined core terminology (`document meaning`, `page metadata`, `wiki context`, `transclusion`, `conversion loss`) in [GLOSSARY.md](GLOSSARY.md).
- **Architecture decisions**:
  - [ADR 0001](docs/adr/0001-parsoid-and-document-fidelity.md): Parsoid compatibility reference, conversion contract, metadata, sanitization, and verification strategy.
  - [ADR 0002](docs/adr/0002-versioned-c-abi-and-wiki-data-callbacks.md): Versioned C ABI, synchronous caller-supplied wiki data callbacks, test CLI contract, and first-release Linux runtime matrix.
- **Release specification and roadmap**: Published the first release specification ([#2](https://github.com/librewiki/rWikiText/issues/2)) and decomposed the implementation into tracked tasks ([#3](https://github.com/librewiki/rWikiText/issues/3)–[#28](https://github.com/librewiki/rWikiText/issues/28)).

### Roadmap

1. **First Development Milestone (Basic Document Conversion)**
   - Pin the exact stable MediaWiki release and Parsoid HTML specification baseline and implement end-to-end paragraph conversion in the Rust API and CLI ([#3](https://github.com/librewiki/rWikiText/issues/3), currently in progress).
   - Extend all four conversion directions and both wikitext round trips to headings ([#4](https://github.com/librewiki/rWikiText/issues/4)), links ([#5](https://github.com/librewiki/rWikiText/issues/5)), and basic ordered/unordered lists ([#6](https://github.com/librewiki/rWikiText/issues/6)).
   - Verify the complete basic document workflow across the Rust API and CLI ([#7](https://github.com/librewiki/rWikiText/issues/7)).
2. **Core MediaWiki Syntax, Metadata, and Callbacks**
   - Conversion-loss diagnostics and strict mode ([#8](https://github.com/librewiki/rWikiText/issues/8)) and MediaWiki HTML element/attribute/URL policy enforcement ([#9](https://github.com/librewiki/rWikiText/issues/9)).
   - Remaining core inline/block formatting ([#10](https://github.com/librewiki/rWikiText/issues/10)), tables with native GFM and permitted HTML fallback ([#11](https://github.com/librewiki/rWikiText/issues/11)), and structured page metadata for categories, redirects, and display titles ([#12](https://github.com/librewiki/rWikiText/issues/12)).
   - Caller callbacks and expansion bounds for template transclusion ([#13](https://github.com/librewiki/rWikiText/issues/13)–[#14](https://github.com/librewiki/rWikiText/issues/14)), page-existence link behavior ([#15](https://github.com/librewiki/rWikiText/issues/15)), file/media links ([#16](https://github.com/librewiki/rWikiText/issues/16)), core parser functions and magic words ([#17](https://github.com/librewiki/rWikiText/issues/17)), and template round-tripping through validated Parsoid `data-mw` metadata ([#18](https://github.com/librewiki/rWikiText/issues/18)).
   - Release-wide MediaWiki core compatibility verification ([#19](https://github.com/librewiki/rWikiText/issues/19)).
3. **Versioned C ABI, Language Bindings, and First Public Release**
   - Ship the versioned C ABI and static archive ([#20](https://github.com/librewiki/rWikiText/issues/20)) along with bindings and runnable callback examples for Python ([#21](https://github.com/librewiki/rWikiText/issues/21)), PHP ([#22](https://github.com/librewiki/rWikiText/issues/22)), Java ([#23](https://github.com/librewiki/rWikiText/issues/23)), Kotlin/Native ([#24](https://github.com/librewiki/rWikiText/issues/24)), and Node.js ([#25](https://github.com/librewiki/rWikiText/issues/25)).
   - Verify all 22 binding and architecture combinations on Linux `glibc` and `musl` ([#26](https://github.com/librewiki/rWikiText/issues/26)–[#27](https://github.com/librewiki/rWikiText/issues/27)) and publish the first public release ([#28](https://github.com/librewiki/rWikiText/issues/28)).

### Out of Scope for the First Release

- MediaWiki extension tags and extension parser functions (prioritized after the first release based on caller needs).
- GitHub.com-specific post-processing beyond the published GFM specification.
- Public syntax-tree editing or partial re-conversion APIs.
- 32-bit architectures and operating systems outside the Linux `glibc`/`musl` (`x86-64` and `ARM64`) verification matrix.

## Documentation

- [GLOSSARY.md](GLOSSARY.md) — Shared domain vocabulary
- [ADR 0001: Use Parsoid as the wikitext compatibility reference](docs/adr/0001-parsoid-and-document-fidelity.md)
- [ADR 0002: Share a versioned C ABI across language bindings](docs/adr/0002-versioned-c-abi-and-wiki-data-callbacks.md)
- [AGENTS.md](AGENTS.md) — Workflow, issue tracker, and conventions for contributors and agents
