@file:OptIn(kotlinx.cinterop.ExperimentalForeignApi::class)

package org.librewiki.rwikitext

import kotlinx.cinterop.*
import cnames.structs.rwikitext_v1_reply
import rwikitext.*

/**
 * Reply returned from a [Lookup] callback to the rWikiText version 1 C ABI.
 *
 * All [Found] and [Error] payloads must be valid UTF-8 JSON values without embedded
 * NUL characters:
 * - For [TEMPLATE_TEXT]: a JSON string (e.g., `"\"Hello, {{{1}}}!\""`).
 * - For [PAGE_EXISTS]: a JSON boolean (`"true"` or `"false"`).
 * - For [FILE_INFO]: a JSON object with `"url"` and optional `"width"` / `"height"`.
 * - For [Error]: a JSON string error message (e.g., `"\"offline\""`).
 */
sealed class Reply {
    /** Reports that the requested wiki data is missing. */
    object Missing : Reply()

    /**
     * Reports found wiki data encoded as a JSON value.
     *
     * @property valueJson UTF-8 JSON value without embedded NUL characters.
     */
    data class Found(val valueJson: String) : Reply()

    /**
     * Reports a lookup failure that aborts conversion with a `wiki_lookup` diagnostic.
     *
     * @property valueJson UTF-8 JSON string error message without embedded NUL characters.
     */
    data class Error(val valueJson: String) : Reply()
}

/** Lookup operation constant: fetch raw template wikitext (JSON string reply). */
const val TEMPLATE_TEXT: UInt = 1u

/** Lookup operation constant: check whether a page exists (JSON boolean reply). */
const val PAGE_EXISTS: UInt = 2u

/** Lookup operation constant: fetch file metadata (JSON object reply with `url` and optional `width`/`height`). */
const val FILE_INFO: UInt = 3u

/**
 * Synchronous wiki-data callback invoked on the thread calling [convert].
 *
 * NamuMark input does not invoke this callback. Any uncaught [Throwable] thrown inside
 * [lookup] is caught and converted into a `wiki_lookup` error diagnostic.
 */
fun interface Lookup {
    /**
     * Resolves wiki data for a canonical title during MediaWiki conversion.
     *
     * @param operation One of [TEMPLATE_TEXT], [PAGE_EXISTS], or [FILE_INFO].
     * @param title Canonical namespace-prefixed, underscore-separated title
     * (for example, `"Template:Greeting"` or `"File:Logo.png"`).
     * @return A [Reply] reporting missing data, JSON-encoded found data, or an error.
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
 * @param requestJson UTF-8 JSON conversion request without embedded NUL characters.
 * @param lookup Optional synchronous wiki-data callback, or `null` to report all lookups missing.
 * @return UTF-8 JSON response envelope (`"ok": true` with `content`, `metadata`, `diagnostics`,
 * or `"ok": false` with `error` and `diagnostics`).
 * @throws IllegalArgumentException If [requestJson] contains a NUL character (`'\u0000'`).
 * @throws IllegalStateException If the linked native ABI version is unsupported or the native
 * converter fails internally.
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
