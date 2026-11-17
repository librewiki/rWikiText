@file:OptIn(kotlinx.cinterop.ExperimentalForeignApi::class)

package org.librewiki.rwikitext

import kotlinx.cinterop.*
import cnames.structs.rwikitext_v1_reply
import rwikitext.*

/** A callback value is JSON: string for template text and error, boolean for page existence,
 * or an object with url and optional width/height for file information. */
sealed class Reply {
    object Missing : Reply()
    data class Found(val valueJson: String) : Reply()
    data class Error(val valueJson: String) : Reply()
}
const val TEMPLATE_TEXT: UInt = 1u
const val PAGE_EXISTS: UInt = 2u
const val FILE_INFO: UInt = 3u
fun interface Lookup { fun lookup(operation: UInt, title: String): Reply }

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

/** Request and response are UTF-8 JSON in the version 1 C ABI format. */
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
