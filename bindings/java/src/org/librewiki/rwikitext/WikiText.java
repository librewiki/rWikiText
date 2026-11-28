package org.librewiki.rwikitext;

import java.nio.charset.StandardCharsets;
import java.util.Objects;

/**
 * JNI binding for the rWikiText version 1 C ABI.
 *
 * <p>Converts UTF-8 documents between MediaWiki wikitext ({@code "wikitext"}), NamuMark
 * ({@code "namumark"}), GitHub Flavored Markdown ({@code "gfm"}), and HTML ({@code "html"}).
 * Supported format pairs are {@code "wikitext"} &harr; {@code "gfm"}, {@code "html"},
 * {@code "namumark"} and {@code "namumark"} &harr; {@code "wikitext"}, {@code "gfm"},
 * {@code "html"}. Identity conversions and direct {@code "gfm"} &harr; {@code "html"}
 * conversions are unsupported and return a JSON response with {@code "ok": false}.
 *
 * <h2>Request JSON Schema</h2>
 * <pre>{@code
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
 * }</pre>
 *
 * <h2>Usage Example</h2>
 * <pre>{@code
 * import org.librewiki.rwikitext.WikiText;
 *
 * String request = "{\"source\":\"wikitext\",\"target\":\"gfm\","
 *     + "\"input\":\"{{Greeting|{{PAGENAME}}}}\\n[[Category:Docs|Guide]]\","
 *     + "\"context\":{\"page_title\":\"Guide\"}}";
 *
 * String responseJson = WikiText.convert(request, (operation, title) -> {
 *     if (operation == WikiText.TEMPLATE_TEXT && "Template:Greeting".equals(title)) {
 *         return WikiText.Reply.found("\"Hello, {{{1}}}!\"");
 *     }
 *     if (operation == WikiText.PAGE_EXISTS) {
 *         return WikiText.Reply.found("Main_Page".equals(title) ? "true" : "false");
 *     }
 *     if (operation == WikiText.FILE_INFO && "File:Logo.png".equals(title)) {
 *         return WikiText.Reply.found(
 *             "{\"url\":\"https://example.org/logo.png\",\"width\":200,\"height\":100}");
 *     }
 *     return WikiText.Reply.missing();
 * });
 * System.out.println(responseJson);
 * }</pre>
 */
public final class WikiText {
    static { System.loadLibrary("rwikitext_jni"); }
    private WikiText() {}

    /** Template wikitext lookup (JSON string reply). */
    public static final int TEMPLATE_TEXT = 1;
    /** Page existence lookup (JSON boolean reply). */
    public static final int PAGE_EXISTS = 2;
    /** File info lookup (JSON object with {@code url} and optional {@code width}/{@code height}). */
    public static final int FILE_INFO = 3;

    /** Reply status for missing data. */
    public static final int MISSING = 0;
    /** Reply status for found data. */
    public static final int FOUND = 1;
    /** Reply status for a lookup error. */
    public static final int ERROR = 2;

    /**
     * Synchronous wiki-data callback run on the thread calling {@link #convert(String, Lookup)}.
     *
     * <p>NamuMark input never calls this callback. Uncaught exceptions from
     * {@link #lookup(int, String)} become {@code "wiki_lookup"} errors in the JSON response.
     */
    @FunctionalInterface
    public interface Lookup {
        /**
         * Looks up wiki data for a canonical title during MediaWiki conversion.
         *
         * @param operation {@link #TEMPLATE_TEXT}, {@link #PAGE_EXISTS}, or {@link #FILE_INFO}
         * @param title canonical namespace-prefixed, underscore-separated title
         *              (e.g. {@code "Template:Greeting"}, {@code "File:Logo.png"})
         * @return {@link Reply#found(String)} with JSON-encoded data (not a raw unquoted string),
         *         {@link Reply#missing()}, or {@link Reply#error(String)}
         */
        Reply lookup(int operation, String title);
    }

    /**
     * Reply returned by a {@link Lookup} callback.
     */
    public static final class Reply {
        /** {@link WikiText#MISSING}, {@link WikiText#FOUND}, or {@link WikiText#ERROR}. */
        public final int status;
        /** UTF-8 JSON bytes, or {@code null} when {@link #status} is {@link WikiText#MISSING}. */
        public final byte[] valueJson;

        private Reply(int status, String valueJson) {
            this.status = status;
            this.valueJson = valueJson == null ? null : utf8(valueJson);
        }

        /**
         * Reports missing wiki data.
         *
         * @return missing-data reply
         */
        public static Reply missing() { return new Reply(MISSING, null); }

        /**
         * Reports found wiki data as a JSON value.
         *
         * @param valueJson UTF-8 JSON value without NUL bytes: JSON string for
         *                  {@link #TEMPLATE_TEXT}, JSON boolean for {@link #PAGE_EXISTS},
         *                  or JSON object for {@link #FILE_INFO}
         * @return found-data reply
         * @throws NullPointerException if {@code valueJson} is {@code null}
         * @throws IllegalArgumentException if {@code valueJson} contains {@code '\0'}
         */
        public static Reply found(String valueJson) { return new Reply(FOUND, Objects.requireNonNull(valueJson)); }

        /**
         * Reports a lookup error and aborts conversion with a {@code "wiki_lookup"} diagnostic.
         *
         * @param valueJson JSON string error message (e.g. {@code "\"offline\""})
         * @return error reply
         * @throws NullPointerException if {@code valueJson} is {@code null}
         * @throws IllegalArgumentException if {@code valueJson} contains {@code '\0'}
         */
        public static Reply error(String valueJson) { return new Reply(ERROR, Objects.requireNonNull(valueJson)); }
    }

    private static byte[] utf8(String value) {
        if (value.indexOf('\0') >= 0) throw new IllegalArgumentException("NUL is not allowed");
        return value.getBytes(StandardCharsets.UTF_8);
    }

    /**
     * Converts a document from a UTF-8 JSON request and returns the native JSON response.
     *
     * <p>Success returns {@code "ok": true} with {@code "content"}, {@code "metadata"}, and
     * {@code "diagnostics"}. Failure returns {@code "ok": false} with {@code "error"} and
     * {@code "diagnostics"}. Exceptions thrown by {@code lookup} become {@code "wiki_lookup"}
     * failures in the response.
     *
     * @param requestJson UTF-8 JSON request without NUL bytes
     * @param lookup optional synchronous callback, or {@code null} to report all lookups missing
     * @return UTF-8 JSON response
     * @throws NullPointerException if {@code requestJson} is {@code null}
     * @throws IllegalArgumentException if {@code requestJson} contains {@code '\0'}
     * @throws IllegalStateException if the native converter fails internally and returns {@code null}
     */
    public static String convert(String requestJson, Lookup lookup) {
        byte[] response = convertNative(utf8(Objects.requireNonNull(requestJson)), lookup);
        if (response == null) throw new IllegalStateException("native conversion failed internally");
        return new String(response, StandardCharsets.UTF_8);
    }
    private static native byte[] convertNative(byte[] request, Lookup lookup);
}
