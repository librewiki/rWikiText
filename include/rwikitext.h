/**
 * @file rwikitext.h
 * @brief Version 1 C ABI for rWikiText.
 *
 * Converts UTF-8 documents between MediaWiki wikitext (`"wikitext"`), NamuMark
 * (`"namumark"`), GitHub Flavored Markdown (`"gfm"`), and HTML (`"html"`).
 *
 * Supported pairs:
 * - `"wikitext"` <-> `"gfm"`, `"html"`, `"namumark"`
 * - `"namumark"` <-> `"wikitext"`, `"gfm"`, `"html"`
 *
 * Identity conversions (`source == target`) and direct `"gfm"` <-> `"html"`
 * conversions return `{"ok": false, ...}`.
 *
 * @section request_schema Request JSON
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
 * @section response_schema Response JSON
 * - Success: `{"ok": true, "content": "...", "metadata": {"categories": [...],
 *   "category_sort_keys": [...], "redirect_target": null, "display_title": null},
 *   "diagnostics": [...]}`
 * - Failure: `{"ok": false, "error": "...", "diagnostics": [...]}`
 * - Diagnostics have `kind` (`"conversion_loss"`, `"html_removal"`,
 *   `"wiki_lookup"`, or `"expansion_limit"`), `start` and `end` (half-open UTF-8
 *   byte offsets in `input`), and `message`.
 *
 * @section example_c Example
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

/** @brief Template wikitext lookup (JSON string reply). */
#define RWIKITEXT_V1_TEMPLATE_TEXT 1
/** @brief Page existence lookup (JSON boolean reply). */
#define RWIKITEXT_V1_PAGE_EXISTS 2
/** @brief File info lookup (JSON object reply with `url` and optional `width`/`height`). */
#define RWIKITEXT_V1_FILE_INFO 3

/** @brief Reply status for missing data; pass `NULL` as `value_json`. */
#define RWIKITEXT_V1_MISSING 0
/** @brief Reply status for found data; pass a JSON value as `value_json`. */
#define RWIKITEXT_V1_FOUND 1
/** @brief Reply status for a lookup error; pass a JSON string as `value_json`. */
#define RWIKITEXT_V1_ERROR 2

/** @brief Opaque reply handle passed to ::rwikitext_v1_lookup_fn. */
typedef struct rwikitext_v1_reply rwikitext_v1_reply;

/**
 * @brief Synchronous wiki-data callback run on the calling thread.
 *
 * Leaving @p reply untouched is the same as setting ::RWIKITEXT_V1_MISSING.
 * NamuMark input never calls this callback.
 *
 * @param user_data Opaque pointer passed to rwikitext_v1_convert().
 * @param operation ::RWIKITEXT_V1_TEMPLATE_TEXT, ::RWIKITEXT_V1_PAGE_EXISTS,
 *                  or ::RWIKITEXT_V1_FILE_INFO.
 * @param title     UTF-8 canonical title (namespace-prefixed, underscore-separated).
 *                  Not NUL-terminated; use @p title_len. Valid only during this call.
 * @param title_len Byte length of @p title.
 * @param reply     Opaque reply handle valid only during this call.
 *
 * @warning Must return normally. Catch foreign exceptions and contain `longjmp`
 *          before returning.
 */
typedef void (*rwikitext_v1_lookup_fn)(void *user_data, uint32_t operation,
                                     const char *title, size_t title_len,
                                     rwikitext_v1_reply *reply);

/**
 * @brief Returns the C ABI version implemented by the linked library (`1`).
 *
 * @return ABI version number.
 */
uint32_t rwikitext_v1_abi_version(void);

/**
 * @brief Converts a document from a UTF-8 JSON request.
 *
 * @param request_json Caller-owned, NUL-terminated UTF-8 JSON request.
 *                     Must stay valid until this function returns.
 * @param lookup       Optional synchronous callback, or `NULL` to report all lookups missing.
 * @param user_data    Passed unchanged to @p lookup.
 *
 * @return Caller-owned, read-only, NUL-terminated UTF-8 JSON response, or `NULL`
 *         on internal failure. Release every non-`NULL` result once with
 *         rwikitext_v1_free().
 *
 * @see rwikitext_v1_free()
 * @see rwikitext_v1_reply_set()
 */
const char *rwikitext_v1_convert(const char *request_json,
                                rwikitext_v1_lookup_fn lookup, void *user_data);

/**
 * @brief Frees a response returned by rwikitext_v1_convert().
 *
 * @param response Pointer from rwikitext_v1_convert(), or `NULL` (no-op).
 *                 Free each non-`NULL` pointer at most once with the same library instance.
 */
void rwikitext_v1_free(const char *response);

/**
 * @brief Sets the reply inside a ::rwikitext_v1_lookup_fn callback.
 *
 * Copies @p value_json before returning, so the callback can free or reuse its buffer
 * immediately.
 *
 * Payload by @p status:
 * - ::RWIKITEXT_V1_MISSING: `NULL`.
 * - ::RWIKITEXT_V1_FOUND:
 *   - ::RWIKITEXT_V1_TEMPLATE_TEXT: JSON string (e.g. `"\"Hello, {{{1}}}!\""`).
 *   - ::RWIKITEXT_V1_PAGE_EXISTS: JSON boolean (`"true"` or `"false"`).
 *   - ::RWIKITEXT_V1_FILE_INFO: JSON object with `"url"` and optional `uint32`
 *     `"width"` and `"height"` (e.g.
 *     `"{\"url\":\"https://example.org/a.png\",\"width\":300,\"height\":200}"`).
 * - ::RWIKITEXT_V1_ERROR: JSON string error message (e.g. `"\"wiki offline\""`).
 *
 * @param reply      Reply handle from the callback.
 * @param status     ::RWIKITEXT_V1_MISSING, ::RWIKITEXT_V1_FOUND, or ::RWIKITEXT_V1_ERROR.
 * @param value_json NUL-terminated UTF-8 JSON value, or `NULL` for ::RWIKITEXT_V1_MISSING.
 *
 * @return `0` on success, `-1` if @p reply is `NULL`, @p status is invalid, or
 *         @p value_json is malformed UTF-8/JSON (records a lookup error on a valid @p reply).
 */
int32_t rwikitext_v1_reply_set(rwikitext_v1_reply *reply, uint32_t status,
                              const char *value_json);

#ifdef __cplusplus
}
#endif

#endif
