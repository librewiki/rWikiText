package org.librewiki.rwikitext;

import java.nio.charset.StandardCharsets;
import java.util.Objects;

/** JSON interface to the version 1 native conversion API. */
public final class WikiText {
    static { System.loadLibrary("rwikitext_jni"); }
    private WikiText() {}

    public static final int TEMPLATE_TEXT = 1, PAGE_EXISTS = 2, FILE_INFO = 3;
    public static final int MISSING = 0, FOUND = 1, ERROR = 2;

    @FunctionalInterface public interface Lookup {
        /** Called synchronously. valueJson is JSON, not a raw string. */
        Reply lookup(int operation, String title);
    }
    public static final class Reply {
        public final int status;
        public final byte[] valueJson;
        private Reply(int status, String valueJson) {
            this.status = status;
            this.valueJson = valueJson == null ? null : utf8(valueJson);
        }
        public static Reply missing() { return new Reply(MISSING, null); }
        public static Reply found(String valueJson) { return new Reply(FOUND, Objects.requireNonNull(valueJson)); }
        public static Reply error(String valueJson) { return new Reply(ERROR, Objects.requireNonNull(valueJson)); }
    }
    private static byte[] utf8(String value) {
        if (value.indexOf('\0') >= 0) throw new IllegalArgumentException("NUL is not allowed");
        return value.getBytes(StandardCharsets.UTF_8);
    }
    /** Returns the native JSON response; a callback exception becomes a lookup error. */
    public static String convert(String requestJson, Lookup lookup) {
        byte[] response = convertNative(utf8(Objects.requireNonNull(requestJson)), lookup);
        if (response == null) throw new IllegalStateException("native conversion failed internally");
        return new String(response, StandardCharsets.UTF_8);
    }
    private static native byte[] convertNative(byte[] request, Lookup lookup);
}
