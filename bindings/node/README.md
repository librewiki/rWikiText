# Node.js binding (`@librewiki/rwikitext`)

Node-API binding for [rWikiText](../../README.md) built with [napi-rs](https://napi.rs/). The native Rust converter is compiled directly into the `.node` binary—no separate shared library path or runtime `dlopen` is required.

## Installation

```sh
npm install @librewiki/rwikitext
```

Prebuilt binaries are distributed via platform-specific `optionalDependencies` for Linux (`x64`/`arm64`, glibc and musl), macOS (`x64`/`arm64`), and Windows (`x64`/`arm64`).

## Usage

```js
const { WikiText, convert, convertAsync } = require('@librewiki/rwikitext');

// Top-level synchronous conversion with optional lookup callback
const result = convert(
  { source: 'wikitext', target: 'html', input: '{{Greeting}}' },
  (operation, title) => {
    if (operation === 'template_text' && title === 'Template:Greeting') {
      return 'Hello from Node';
    }
    return null;
  },
);

// Class API (also accepts an optional ignored argument for compatibility)
const wiki = new WikiText();
const namu = wiki.convert({
  source: 'namumark',
  target: 'gfm',
  input: "== 제목 ==\n'''안녕'''",
});

// Asynchronous conversion on the libuv worker pool (no lookup callback)
const asyncResult = await convertAsync({
  source: 'wikitext',
  target: 'html',
  input: "'''Async'''",
});
```

### Lookup callback

The optional synchronous `lookup(operation, title)` runs on the calling thread. Return JSON-serializable values:

- `template_text`: wikitext string, or `null`/`undefined` when missing.
- `page_exists`: boolean, or `null`/`undefined` when missing.
- `file_info`: object with `url` and optional `width` and `height`, or `null`/`undefined` when missing.

A thrown exception is sent to the converter as a lookup error and returns `{ ok: false, error, diagnostics }` rather than silently treating it as missing. Without a callback all lookups are treated as missing. Promises and asynchronous callbacks are not supported.

### Request and response schema

The request is an object with `source`, `target` (`wikitext`, `gfm`, `html`, or `namumark`) and string `input`. Optional `context` contains `page_title`, `site_language`, and `reference_time`; optional `mode` is `best_effort` or `strict`; optional `expansion_limits` contains `max_depth`, `max_expansions`, and `max_output_bytes`. See the [native bindings documentation](../../docs/native-bindings.md) for the precise semantics and supported conversions.

The returned object is `{ ok: true, content, metadata, diagnostics }` or `{ ok: false, error, diagnostics }`. Metadata includes `categories`, `category_sort_keys`, `redirect_target`, and `display_title`; diagnostics have `kind`, `start`, `end`, and `message`.

## Local development

Install Rust and Node.js (>= 18), then from `bindings/node`:

```sh
npm install
npm run build        # release build (or `npm run build:debug`)
npm test
```

## Publishing to npm

Automated publishing runs in GitHub Actions via [`.github/workflows/release.yml`](../../.github/workflows/release.yml) and [`.github/workflows/publish-node.yml`](../../.github/workflows/publish-node.yml) when a `release/*` pull request is merged into `main` (or when triggered manually with `workflow_dispatch`).

1. Sync versions in `bindings/node/Cargo.toml` and `bindings/node/package.json` on the release branch.
2. Merge the `release/*` pull request into `main`. CI cross-compiles all 8 platform targets (`linux-x64-gnu`, `linux-arm64-gnu`, `linux-x64-musl`, `linux-arm64-musl`, `darwin-x64`, `darwin-arm64`, `win32-x64-msvc`, `win32-arm64-msvc`), assembles `optionalDependencies`, and publishes each package via npm Trusted Publishing (OIDC) using `npm stage publish --provenance --access public`.

To assemble platform package directories locally for inspection:

```sh
npm run create-npm-dir
npm run artifacts
```
