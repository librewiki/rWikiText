# Share a versioned C ABI across language bindings

The Rust library exposes document conversion through a versioned C ABI and ships as a native static archive. C, Kotlin/Native, and Go link the archive through a C header. Python and Java load shared native code, PHP uses a native extension, and Node.js uses a Node-API addon. All bindings use the same conversion implementation and expose the conversion result, page metadata, and diagnostics.

## Caller-supplied wiki data

Each conversion request supplies its input and output formats, wiki context, and any data callbacks. Outside NamuMark conversions, the Rust library requests template text, page existence, or file information as needed. Callbacks run synchronously on the thread invoking the conversion. A callback returns the requested data, reports that it is missing, or reports a lookup error. Missing data follows Parsoid behavior. A lookup error fails the conversion so the caller can decide whether to retry. The caller owns data access, which also permits offline conversion with local data.

The test CLI uses the same document conversion API. It selects a supported source and target format pair, reads input from a file or standard input, and serves wiki context and lookup responses from local test data. It writes converted content to standard output and diagnostics to standard error.

The four MediaWiki directions (Wikitext ↔ GFM and Wikitext ↔ HTML, in both directions) remain the implemented MediaWiki contract. The [NamuMark scope amendment](0001-parsoid-and-document-fidelity.md#scope-amendment-namumark-2026-11-24) adds the six NamuMark directions, now exposed through the shared API and bindings. Preserve existing ABI behavior and version incompatible changes; NamuMark context and callback semantics are defined by its own baseline, and NamuMark input invokes no wiki-data callbacks.

## First-release Linux runtime checks

| C runtime | Architectures | Bindings |
| --- | --- | --- |
| glibc | x86-64, ARM64 | C, Python, PHP, Java, Kotlin/Native, Node.js, Go |
| musl | x86-64, ARM64 | C, Python, PHP, Java, Node.js, Go |

Provide a runnable example for each binding that converts a document and exercises a wiki data callback. Run each example on all of its listed targets. Use Alpine's Node.js distribution for musl ARM64. Kotlin/Native's stock Linux targets use glibc. Defer 32-bit verification to later work. [Kotlin/Native targets](https://kotlinlang.org/docs/native-target-support.html) · [Node.js Alpine images](https://github.com/nodejs/docker-node#nodealpine)

The matrix records first-release check targets, not completed results. The recorded Go example run and package tests cover glibc x86-64 only; Linux ARM64, musl, and the remaining binding runtimes are release gates tracked in [#26](https://github.com/librewiki/rWikiText/issues/26) and [#27](https://github.com/librewiki/rWikiText/issues/27).
