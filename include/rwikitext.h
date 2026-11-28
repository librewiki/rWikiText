/**
 * @file rwikitext.h
 * @brief Version 1 C ABI for the rWikiText document converter.
 *
 * rWikiText converts UTF-8 documents between MediaWiki wikitext (`"wikitext"`),
 * NamuMark (`"namumark"`), GitHub Flavored Markdown (`"gfm"`), and HTML (`"html"`).
 *
 * Supported format pairs:
 * - `"wikitext"` <-> `"gfm"`, `"html"`, `"namumark"`
 * - `"namumark"` <-> `"wikitext"`, `"gfm"`, `"html"`
 *
 * Identity conversions (`source == target`) and direct `"gfm"` <-> `"html"`
 * conversions are unsupported and return a JSON error envelope (`"ok": false`).
 *
 * @section request_schema Request JSON Schema
 * Pass a NUL-terminated UTF-8 JSON object to rwikitext_v1_convert():
 * @code{.json}
 * {
 *   "source": "wikitext",
 *   "target": "gfm",
 *   "input": "{{Greeting|{{PAGENAME}}}}\n[[Category:Demo|Sort]]",
 *   "context": {
 *     "page_title": "Guide",
 *     "site_language": "en",
 *     "reference_time": "1970-01-01T00:00:00Z"
 *   },
 *   "mode": "best_effort",
 *   "expansion_limits": {
 *     "max_depth": 40,
 *     "max_expansions": 10000,
 *     "max_output_bytes": 2097152
 *   }
 * }
 * @endcode
 * `source`, `target`, and `input` are required. `context`, `mode` (`"best_effort"`
 * or `"strict"`), and `expansion_limits` are optional.
 *
 * @section response_schema Response JSON Schema
 * - Success: `{"ok": true, "content": "...", "metadata": {"categories": [...],
 *   "category_sort_keys": [...], "redirect_target": null, "display_title": null},
 *   "diagnostics": [...]}`
 * - Failure: `{"ok": false, "error": "...", "diagnostics": [...]}`
 * - Each diagnostic has `kind` (`"conversion_loss"`, `"html_removal"`,
 *   `"wiki_lookup"`, or `"expansion_limit"`), `start` and `end` (half-open UTF-8
 *   byte offsets in the input), and `message`.
 *
 * @section example_c Basic C Example
 * @code{.c}
 * #include "rwikitext.h"
 * #include <stdio.h>
 * #include <string.h>
 *
 * static void my_lookup(void *user_data, uint32_t operation, const char *title,
 *                       size_t title_len, rwikitext_v1_reply *reply) {
 *     (void)user_data;
 *     if (operation == RWIKITEXT_V1_TEMPLATE_TEXT &&
 *         title_len == strlen("Template:Greeting") &&
 *         memcmp(title, "Template:Greeting", title_len) == 0) {
 *         rwikitext_v1_reply_set(reply, RWIKITEXT_V1_FOUND, "\"Hello, {{{1}}}!\"");
 *     }
 * }
 *
 * int main(void) {
 *     if (rwikitext_v1_abi_version() != RWIKITEXT_ABI_VERSION) {
 *         return 1;
 *     }
 *     const char *req =
 *         "{\"source\":\"wikitext\",\"target\":\"gfm\","
 *         "\"input\":\"{{Greeting|World}}\"}";
 *     const char *res = rwikitext_v1_convert(req, my_lookup, NULL);
 *     if (res == NULL) {
 *         return 1;
 *     }
 *     puts(res);
 *     rwikitext_v1_free(res);
 *     return 0;
 * }
 * @endcode
 */
#ifndef RWIKITEXT_H
#define RWIKITEXT_H

#include <stddef.h>
#include <stdint.h>

#ifdef __cplusplus
extern "C" {
#endif

/** @brief Header ABI version. Compare with rwikitext_v1_abi_version() at runtime. */
#define RWIKITEXT_ABI_VERSION 1

/** @brief Lookup operation: fetch raw template wikitext (JSON string reply). */
#define RWIKITEXT_V1_TEMPLATE_TEXT 1
/** @brief Lookup operation: check whether a page exists (JSON boolean reply). */
#define RWIKITEXT_V1_PAGE_EXISTS 2
/** @brief Lookup operation: fetch file metadata (JSON object reply with `url` and optional `width`/`height`). */
#define RWIKITEXT_V1_FILE_INFO 3

/** @brief Reply status: requested data is missing; pass `NULL` as `value_json`. */
#define RWIKITEXT_V1_MISSING 0
/** @brief Reply status: requested data was found; pass a JSON value as `value_json`. */
#define RWIKITEXT_V1_FOUND 1
/** @brief Reply status: lookup failed; pass a JSON string error message as `value_json`. */
#define RWIKITEXT_V1_ERROR 2

/** @brief Opaque reply handle passed to ::rwikitext_v1_lookup_fn callbacks. */
typedef struct rwikitext_v1_reply rwikitext_v1_reply;

/**
 * @brief Synchronous wiki-data lookup callback invoked during conversion.
 *
 * Runs on the thread that called rwikitext_v1_convert(). Leaving @p reply untouched
 * is equivalent to calling rwikitext_v1_reply_set() with ::RWIKITEXT_V1_MISSING.
 * NamuMark input does not invoke this callback.
 *
 * @param user_data Opaque pointer passed to rwikitext_v1_convert().
 * @param operation One of ::RWIKITEXT_V1_TEMPLATE_TEXT, ::RWIKITEXT_V1_PAGE_EXISTS,
 *                  or ::RWIKITEXT_V1_FILE_INFO.
 * @param title     UTF-8 canonical title (namespace-prefixed, underscore-separated).
 *                  Not guaranteed to be NUL-terminated; use @p title_len.
 *                  Valid only for the duration of this callback invocation.
 * @param title_len Length of @p title in bytes.
 * @param reply     Opaque reply handle valid only for this callback invocation.
 *
 * @warning Callbacks must return normally. Catch foreign exceptions and contain
 *          nonlocal jumps (`longjmp`) inside the callback before returning.
 */
typedef void (*rwikitext_v1_lookup_fn)(void *user_data, uint32_t operation,
                                     const char *title, size_t title_len,
                                     rwikitext_v1_reply *reply);

/**
 * @brief Returns the runtime C ABI version implemented by the linked library.
 *
 * @return ABI version number (currently ::RWIKITEXT_ABI_VERSION, i.e., `1`).
 */
uint32_t rwikitext_v1_abi_version(void);

/**
 * @brief Converts a document described by a UTF-8 JSON request.
 *
 * @param request_json NUL-terminated UTF-8 JSON conversion request owned by the caller.
 *                     Must remain valid for the duration of the call.
 * @param lookup       Optional synchronous wiki-data callback, or `NULL` to report
 *                     all lookups as missing.
 * @param user_data    Opaque pointer forwarded unchanged to @p lookup.
 *
 * @return Newly allocated, read-only, NUL-terminated UTF-8 JSON response string
 *         owned by the caller, or `NULL` on an unrecoverable internal failure.
 *         Every non-`NULL` return value must be released exactly once with
 *         rwikitext_v1_free().
 *
 * @see rwikitext_v1_free()
 * @see rwikitext_v1_reply_set()
 */
const char *rwikitext_v1_convert(const char *request_json,
                                rwikitext_v1_lookup_fn lookup, void *user_data);

/**
 * @brief Releases a response buffer returned by rwikitext_v1_convert().
 *
 * @param response Pointer previously returned by rwikitext_v1_convert(), or `NULL`
 *                 (which is a no-op). Must be freed at most once using the same
 *                 library instance that allocated it.
 */
void rwikitext_v1_free(const char *response);

/**
 * @brief Populates the reply handle inside a ::rwikitext_v1_lookup_fn callback.
 *
 * The library copies @p value_json before returning, so the caller may immediately
 * free or reuse its buffer.
 *
 * Expected @p value_json format by @p status:
 * - ::RWIKITEXT_V1_MISSING: `NULL`.
 * - ::RWIKITEXT_V1_FOUND:
 *   - ::RWIKITEXT_V1_TEMPLATE_TEXT: JSON string (e.g., `"\"Hello, {{{1}}}!\""`).
 *   - ::RWIKITEXT_V1_PAGE_EXISTS: JSON boolean (`"true"` or `"false"`).
 *   - ::RWIKITEXT_V1_FILE_INFO: JSON object with `"url"` (string) and optional
 *     unsigned 32-bit `"width"` and `"height"` (e.g.,
 *     `"{\"url\":\"https://example.org/a.png\",\"width\":300,\"height\":200}"`).
 * - ::RWIKITEXT_V1_ERROR: JSON string error message (e.g., `"\"wiki offline\""`).
 *
 * @param reply      Opaque reply pointer provided to the callback.
 * @param status     One of ::RWIKITEXT_V1_MISSING, ::RWIKITEXT_V1_FOUND, or
 *                   ::RWIKITEXT_V1_ERROR.
 * @param value_json NUL-terminated UTF-8 JSON payload, or `NULL` for
 *                   ::RWIKITEXT_V1_MISSING.
 *
 * @return `0` on success; `-1` if @p reply is `NULL`, @p status is invalid, or
 *         @p value_json is not valid UTF-8 / JSON (for a non-`NULL` @p reply, an
 *         invalid call also records a lookup error diagnostic).
 */
int32_t rwikitext_v1_reply_set(rwikitext_v1_reply *reply, uint32_t status,
                              const char *value_json);

#ifdef __cplusplus
}
#endif

#endif
