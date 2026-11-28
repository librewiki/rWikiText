# Python binding

This synchronous binding loads the shared rWikiText library through `ctypes`.

## Installation

Install a prebuilt wheel from PyPI (bundles the native shared library):

```sh
pip install rwikitext
```

```python
from rwikitext import WikiText

wiki = WikiText()
result = wiki.convert({"source": "wikitext", "target": "html", "input": "Hello '''wiki'''"})
print(result["content"])
```

## Building from source

To run against a locally built library from this repository without installing a wheel (Linux glibc/musl, x86-64/ARM64):

```sh
# On glibc:
cargo build --release --lib
# On musl, use this instead:
RUSTFLAGS="-C target-feature=-crt-static" cargo build --release --lib
python3 bindings/python/example.py "$(pwd)/target/release/librwikitext.so"
```

You can also pass an explicit shared library path to `WikiText('/absolute/path/to/librwikitext.so')`. Call `wiki.convert(request, lookup)`; the optional synchronous `lookup(operation, title)` is invoked on the calling thread. Its result must be JSON-serializable:

- `template_text`: a string of wikitext, or `None` when missing.
- `page_exists`: a boolean, or `None` when missing.
- `file_info`: an object with `url` and optional `width` and `height`, or `None` when missing.

A raised exception is sent to the converter as a lookup error. The conversion returns an error response rather than silently treating it as missing. Without a callback all lookups are missing. Do not return awaitables.

The request is a dict with `source`, `target` (`wikitext`, `gfm`, `html`, or `namumark`) and string `input`. Optional `context` contains `page_title`, `site_language`, and `reference_time`; optional `mode` is `best_effort` or `strict`; optional `expansion_limits` contains `max_depth`, `max_expansions`, and `max_output_bytes`. See the [C ABI documentation](../../docs/native-bindings.md) for the precise semantics and supported conversions.

The returned dict is `{ok: true, content, metadata, diagnostics}` or `{ok: false, error, diagnostics}`. Metadata includes `categories`, `category_sort_keys`, `redirect_target`, and `display_title`; diagnostics have `kind`, `start`, `end`, and `message`. Invalid requests and lookup failures are represented by the native error response; native load, ABI, and internal failures raise Python exceptions.
