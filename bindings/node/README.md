# Node.js binding

The Node-API addon loads the shared rWikiText library at runtime. On Linux install Rust, a C compiler, Node.js and the matching Node development headers (e.g. `libnode-dev` on Debian/Ubuntu or `nodejs-dev` on Alpine). From the repository root on glibc or musl, x86-64 or ARM64:

```sh
# On glibc:
cargo build --release --lib
# On musl, use this instead:
RUSTFLAGS="-C target-feature=-crt-static" cargo build --release --lib
cc -std=c11 -O2 -fPIC -shared -I/usr/include/node bindings/node/addon.c -ldl -o bindings/node/rwikitext.node
node bindings/node/example.js "$(pwd)/target/release/librwikitext.so"
```

If the headers are installed elsewhere, replace `/usr/include/node` with the directory containing `node_api.h`. On Alpine ARM64 use Alpine's Node.js package. No npm packages are needed.

Use `const { WikiText } = require('./bindings/node')` and `new WikiText('/absolute/path/to/librwikitext.so')`. Call `wiki.convert(request, lookup)`; the optional synchronous `lookup(operation, title)` runs on the calling thread. Return JSON-serializable values:

- `template_text`: wikitext string, or `null`/`undefined` when missing.
- `page_exists`: boolean, or `null`/`undefined` when missing.
- `file_info`: object with `url` and optional `width` and `height`, or `null`/`undefined` when missing.

A thrown exception is sent to the converter as a lookup error. The conversion returns an error response rather than silently treating it as missing. Without a callback all lookups are missing. Promises and asynchronous callbacks are not supported.

The request is an object with `source`, `target` (`wikitext`, `gfm`, `html`, or `namumark`) and string `input`. Optional `context` contains `page_title`, `site_language`, and `reference_time`; optional `mode` is `best_effort` or `strict`; optional `expansion_limits` contains `max_depth`, `max_expansions`, and `max_output_bytes`. See the [C ABI documentation](../../docs/native-bindings.md) for the precise semantics and supported conversions.

The returned object is `{ok: true, content, metadata, diagnostics}` or `{ok: false, error, diagnostics}`. Metadata includes `categories`, `category_sort_keys`, `redirect_target`, and `display_title`; diagnostics have `kind`, `start`, `end`, and `message`. Invalid requests and lookup failures are represented by the native error response; native load, ABI, and internal failures throw JavaScript exceptions.
