@file:OptIn(kotlinx.cinterop.ExperimentalForeignApi::class)

package org.librewiki.rwikitext

import kotlinx.cinterop.*
import cnames.structs.rwikitext_v1_reply
import rwikitext.*

/**
 * Reply returned by a [Lookup] callback for the rWikiText version 1 C ABI.
 *
 * [Found] and [Error] payloads must be valid UTF-8 JSON without NUL characters:
 * - [TEMPLATE_TEXT]: JSON string (e.g. `"\"Hello, {{{1}}}!\""`)
 * - [PAGE_EXISTS]: JSON boolean (`"true"` or `"false"`)
 * - [FILE_INFO]: JSON object with `"url"` and optional `"width"` / `"height"`
 * - [Error]: JSON string error message (e.g. `"\"offline\""`)
 */
sealed class Reply {
    /** Missing wiki data. */
    object Missing : Reply()

    /**
     * Found wiki data encoded as JSON.
     *
     * @property valueJson UTF-8 JSON value without NUL characters.
     */
    data class Found(val valueJson: String) : Reply()

    /**
     * Lookup failure; aborts conversion with a `wiki_lookup` diagnostic.
     *
     * @property valueJson UTF-8 JSON string error message without NUL characters.
     */
    data class Error(val valueJson: String) : Reply()
}

/** Template wikitext lookup (JSON string reply). */
const val TEMPLATE_TEXT: UInt = 1u

/** Page existence lookup (JSON boolean reply). */
const val PAGE_EXISTS: UInt = 2u

/** File info lookup (JSON object reply with `url` and optional `width`/`height`). */
const val FILE_INFO: UInt = 3u

/**
 * Synchronous wiki-data callback run on the thread calling [convert].
 *
 * NamuMark input never calls this callback. Uncaught [Throwable]s from [lookup]
 * become `wiki_lookup` errors in the JSON response.
 */
fun interface Lookup {
    /**
     * Looks up wiki data for a canonical title during MediaWiki conversion.
     *
     * @param operation [TEMPLATE_TEXT], [PAGE_EXISTS], or [FILE_INFO].
     * @param title Canonical namespace-prefixed, underscore-separated title
     * (e.g. `"Template:Greeting"`, `"File:Logo.png"`).
     * @return [Reply.Missing], [Reply.Found], or [Reply.Error].
     */
    fun lookup(operation: UInt, title: String): Reply
}

private fun checked(value: String): String {
    require('\u0000' !in value) { "NUL is not allowed" }
    return value
}
private val callback = staticCFunction { user: COpaquePointer?, operation: UInt,
                                       title: CPointer<ByteVar>?, length: ULong,
                                       reply: CPointer<rwikitext_v1_reply>? ->
    try {
        require(user != null && title != null && reply != null && length <= Int.MAX_VALUE.toULong())
        val result = user.asStableRef<Lookup>().get().lookup(operation, title.readBytes(length.toInt()).decodeToString())
        memScoped {
            val status: UInt
            val value: CPointer<ByteVar>?
            when (result) {
                Reply.Missing -> { status = 0u; value = null }
                is Reply.Found -> { status = 1u; value = checked(result.valueJson).cstr.ptr }
                is Reply.Error -> { status = 2u; value = checked(result.valueJson).cstr.ptr }
            }
            rwikitext_v1_reply_set(reply, status, value)
        }
    } catch (_: Throwable) {
        if (reply != null) memScoped { rwikitext_v1_reply_set(reply, 2u, "\"Kotlin wiki callback failed\"".cstr.ptr) }
    }
    Unit
}

/**
 * Converts a document described by a UTF-8 JSON request using the rWikiText version 1 C ABI.
 *
 * Supported formats are `"wikitext"`, `"namumark"`, `"gfm"`, and `"html"`.
 * Supported pairs are `"wikitext"` <-> `"gfm"`, `"html"`, `"namumark"` and
 * `"namumark"` <-> `"wikitext"`, `"gfm"`, `"html"`. Identity conversions and direct
 * `"gfm"` <-> `"html"` conversions return a JSON object with `"ok": false`.
 *
 * Request JSON supports required fields `source`, `target`, and `input`, plus optional
 * `context` (`page_title`, `site_language`, `reference_time`), `mode` (`"best_effort"`
 * or `"strict"`), and `expansion_limits` (`max_depth`, `max_expansions`, `max_output_bytes`).
 *
 * ```kotlin
 * import org.librewiki.rwikitext.*
 *
 * val request = """
 *     {
 *       "source": "wikitext",
 *       "target": "gfm",
 *       "input": "{{Greeting|{{PAGENAME}}}}\n[[Category:Docs|Guide]]",
 *       "context": {"page_title": "Guide"}
 *     }
 * """.trimIndent()
 *
 * val responseJson = convert(request, Lookup { op, title ->
 *     when {
 *         op == TEMPLATE_TEXT && title == "Template:Greeting" ->
 *             Reply.Found("\"Hello, {{{1}}}!\"")
 *         op == PAGE_EXISTS ->
 *             Reply.Found(if (title == "Main_Page") "true" else "false")
 *         op == FILE_INFO && title == "File:Logo.png" ->
 *             Reply.Found("""{"url":"https://example.org/logo.png","width":200,"height":100}""")
 *         else -> Reply.Missing
 *     }
 * })
 * println(responseJson)
 * ```
 *
 * @param requestJson UTF-8 JSON request without NUL characters.
 * @param lookup Optional synchronous callback, or `null` to report all lookups missing.
 * @return UTF-8 JSON response (`"ok": true` with `content`, `metadata`, `diagnostics`,
 * or `"ok": false` with `error` and `diagnostics`).
 * @throws IllegalArgumentException If [requestJson] contains `'\u0000'`.
 * @throws IllegalStateException If the linked ABI version is unsupported or conversion fails internally.
 */
fun convert(requestJson: String, lookup: Lookup? = null): String {
    check(rwikitext_v1_abi_version() == 1u) { "Unsupported native ABI" }
    val ref = lookup?.let { StableRef.create(it) }
    try {
        return memScoped {
            val response = rwikitext_v1_convert(checked(requestJson).cstr.ptr,
                if (ref == null) null else callback, ref?.asCPointer())
                ?: error("Native conversion failed internally")
            try { response.toKString() } finally { rwikitext_v1_free(response) }
        }
    } finally { ref?.dispose() }
}
