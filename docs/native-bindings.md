# Native bindings

The version 1 C ABI calls the same document conversion implementation as the Rust library and CLI. It uses UTF-8 JSON for requests and responses to give every binding the same field names and diagnostic representation.

## Build and run

On Linux with glibc, build the static archive and shared library with Cargo. The C example links the archive and exercises template text, page existence, and file information callbacks.

```sh
cargo build --locked --release --lib
cc -std=c11 -Wall -Wextra -Werror -Iinclude examples/convert.c \
  target/release/librwikitext.a -ldl -lpthread -lm -o target/convert-c
./target/convert-c
```

On Linux with musl, build with a matching musl Rust toolchain and C compiler. Use `RUSTFLAGS="-C target-feature=-crt-static" cargo build --locked --release --lib` to build the shared library alongside the archive. Run the same C compile and example commands.

- [Python](../bindings/python/README.md)
- [PHP](../bindings/php/README.md)
- [Java](../bindings/java/README.md)
- [Kotlin/Native](../bindings/kotlin-native/README.md)
- [Node.js](../bindings/node/README.md)
- [Go](../bindings/go/README.md)

## Conversion request

`source`, `target`, and `input` are required strings. The format names are `wikitext`, `gfm`, and `html`. Every supported conversion has wikitext on one side.

NamuMark format selection and conversion are planned and not yet exposed by this ABI or the bindings. See the [NamuMark implementation and release plan](namumark.md).

```json
{
  "source": "wikitext",
  "target": "gfm",
  "input": "{{Greeting|{{PAGENAME}}}}\n[[Category:Demo|Sort]]",
  "context": {
    "page_title": "Guide",
    "site_language": "en",
    "reference_time": "1970-01-01T00:00:00Z"
  },
  "mode": "best_effort",
  "expansion_limits": {
    "max_depth": 40,
    "max_expansions": 10000,
    "max_output_bytes": 2097152
  }
}
```

`context`, `mode`, `expansion_limits`, and each field within the two optional objects can be omitted. The default page title is empty. The other defaults match the values shown. Each expansion limit is a nonnegative integer. Use `strict` mode to fail the conversion on loss of document meaning.

## Result and diagnostics

- Success returns an object with `ok: true`, the converted `content` string, a `metadata` object, and a `diagnostics` array.
- Metadata contains the `categories` array and a matching `category_sort_keys` array of strings or nulls. `redirect_target` and `display_title` are strings or nulls.
- Failure returns `ok: false`, an `error` string, and a `diagnostics` array. Invalid requests and unsupported format pairs return an empty diagnostic array.
- Each diagnostic contains a `message`, a `kind` (`conversion_loss`, `html_removal`, `wiki_lookup`, or `expansion_limit`), and a half-open byte range `start` to `end` in the original input.

## C ownership and callbacks

Include [`rwikitext.h`](../include/rwikitext.h) and link the archive or shared library. The symbols carry the `rwikitext_v1_` prefix. `rwikitext_v1_abi_version()` returns 1.

Pass a readable, NUL-terminated UTF-8 JSON request to `rwikitext_v1_convert`. You own that buffer and any callback user data. Keep them alive through the call. A non-null response is a read-only, NUL-terminated UTF-8 JSON buffer. Release it once with `rwikitext_v1_free`, using the same library instance that allocated it. A null response indicates an internal failure. JSON escape sequences can represent embedded NULs in document strings.

The optional callback runs synchronously on the calling thread. It receives the user pointer, an operation, a canonical title with an explicit UTF-8 byte length, and an opaque reply pointer. Use the title bytes and reply only within the callback. Handle foreign exceptions within your binding.

| Operation | Meaning | JSON value for a found lookup |
| --- | --- | --- |
| `RWIKITEXT_V1_TEMPLATE_TEXT` (1) | Find template text | String |
| `RWIKITEXT_V1_PAGE_EXISTS` (2) | Check page existence | Boolean |
| `RWIKITEXT_V1_FILE_INFO` (3) | Find file information | Object with a string `url` and optional unsigned 32-bit integer `width` and `height` in pixels |

Use `rwikitext_v1_reply_set` with `RWIKITEXT_V1_FOUND` and the JSON value for known data. Use `RWIKITEXT_V1_ERROR` and a JSON string message to fail the conversion with a located `wiki_lookup` diagnostic. Use `RWIKITEXT_V1_MISSING` with a null payload or leave the reply untouched to follow the core's missing-data behavior. Omitting the callback also reports missing. Found `false` explicitly reports that a page is absent.

The setter copies its NUL-terminated JSON payload during the call, so the callback may immediately release its buffer. The setter returns 0 on success and -1 for an invalid status or malformed payload. Invalid callback replies fail the conversion with a lookup diagnostic.
