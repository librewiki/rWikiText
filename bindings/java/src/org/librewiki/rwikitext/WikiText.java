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

    /** Lookup operation constant: fetch raw template wikitext (JSON string reply). */
    public static final int TEMPLATE_TEXT = 1;
    /** Lookup operation constant: check whether a page exists (JSON boolean reply). */
    public static final int PAGE_EXISTS = 2;
    /** Lookup operation constant: fetch file metadata (JSON object with {@code url} and optional {@code width}/{@code height}). */
    public static final int FILE_INFO = 3;

    /** Reply status constant: requested wiki data is missing. */
    public static final int MISSING = 0;
    /** Reply status constant: requested wiki data was found. */
    public static final int FOUND = 1;
    /** Reply status constant: lookup failed with a JSON string error message. */
    public static final int ERROR = 2;

    /**
     * Synchronous wiki-data callback invoked on the thread calling {@link #convert(String, Lookup)}.
     *
     * <p>NamuMark input does not invoke this callback. Any uncaught exception thrown by
     * {@link #lookup(int, String)} is caught by the JNI bridge and reported as a
     * {@code "wiki_lookup"} error in the JSON response.
     */
    @FunctionalInterface
    public interface Lookup {
        /**
         * Resolves wiki data for a canonical title during MediaWiki conversion.
         *
         * @param operation one of {@link #TEMPLATE_TEXT}, {@link #PAGE_EXISTS}, or {@link #FILE_INFO}
         * @param title canonical namespace-prefixed, underscore-separated title
         *              (for example, {@code "Template:Greeting"} or {@code "File:Logo.png"})
         * @return a {@link Reply} carrying JSON-encoded data (not a raw unquoted string),
         *         {@link Reply#missing()}, or {@link Reply#error(String)}
         */
        Reply lookup(int operation, String title);
    }

    /**
     * Reply returned from a {@link Lookup} callback.
     */
    public static final class Reply {
        /** Reply status: {@link WikiText#MISSING}, {@link WikiText#FOUND}, or {@link WikiText#ERROR}. */
        public final int status;
        /** UTF-8 encoded JSON payload, or {@code null} when {@link #status} is {@link WikiText#MISSING}. */
        public final byte[] valueJson;

        private Reply(int status, String valueJson) {
            this.status = status;
            this.valueJson = valueJson == null ? null : utf8(valueJson);
        }

        /**
         * Creates a reply indicating that the requested wiki data is missing.
         *
         * @return a missing-data reply
         */
        public static Reply missing() { return new Reply(MISSING, null); }

        /**
         * Creates a reply carrying found wiki data encoded as a JSON value.
         *
         * @param valueJson valid UTF-8 JSON value without embedded NUL characters:
         *                  a JSON string for {@link #TEMPLATE_TEXT}, a JSON boolean for
         *                  {@link #PAGE_EXISTS}, or a JSON object for {@link #FILE_INFO}
         * @return a found-data reply
         * @throws NullPointerException if {@code valueJson} is {@code null}
         * @throws IllegalArgumentException if {@code valueJson} contains a NUL character ({@code '\0'})
         */
        public static Reply found(String valueJson) { return new Reply(FOUND, Objects.requireNonNull(valueJson)); }

        /**
         * Creates an error reply that aborts conversion with a located {@code "wiki_lookup"} diagnostic.
         *
         * @param valueJson JSON string containing the error message (for example, {@code "\"offline\""})
         * @return an error reply
         * @throws NullPointerException if {@code valueJson} is {@code null}
         * @throws IllegalArgumentException if {@code valueJson} contains a NUL character ({@code '\0'})
         */
        public static Reply error(String valueJson) { return new Reply(ERROR, Objects.requireNonNull(valueJson)); }
    }

    private static byte[] utf8(String value) {
        if (value.indexOf('\0') >= 0) throw new IllegalArgumentException("NUL is not allowed");
        return value.getBytes(StandardCharsets.UTF_8);
    }

    /**
     * Converts a document described by a UTF-8 JSON request and returns the native JSON response.
     *
     * <p>On success, the returned JSON object contains {@code "ok": true}, {@code "content"},
     * {@code "metadata"}, and {@code "diagnostics"}. On failure, it contains {@code "ok": false},
     * {@code "error"}, and {@code "diagnostics"}. Any exception thrown by {@code lookup} is
     * converted into a {@code "wiki_lookup"} failure in the response.
     *
     * @param requestJson UTF-8 JSON conversion request without embedded NUL characters
     * @param lookup optional synchronous wiki-data callback, or {@code null} to report all lookups missing
     * @return UTF-8 JSON response envelope
     * @throws NullPointerException if {@code requestJson} is {@code null}
     * @throws IllegalArgumentException if {@code requestJson} contains a NUL character ({@code '\0'})
     * @throws IllegalStateException if the native converter fails internally and returns {@code null}
     */
    public static String convert(String requestJson, Lookup lookup) {
        byte[] response = convertNative(utf8(Objects.requireNonNull(requestJson)), lookup);
        if (response == null) throw new IllegalStateException("native conversion failed internally");
        return new String(response, StandardCharsets.UTF_8);
    }
    private static native byte[] convertNative(byte[] request, Lookup lookup);
}
