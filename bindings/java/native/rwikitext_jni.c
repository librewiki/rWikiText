#include <jni.h>
#include <stdint.h>
#include <stdlib.h>
#include <string.h>
#include "rwikitext.h"

typedef struct { JNIEnv *env; jobject callback; jmethodID method; } context;
static char *bytes(JNIEnv *env, jbyteArray data) {
    if (!data) return NULL;
    jsize n = (*env)->GetArrayLength(env, data);
    char *out = malloc((size_t)n + 1);
    if (!out) return NULL;
    (*env)->GetByteArrayRegion(env, data, 0, n, (jbyte *)out);
    if ((*env)->ExceptionCheck(env)) { free(out); return NULL; }
    out[n] = 0;
    if (memchr(out, 0, (size_t)n)) { free(out); return NULL; }
    return out;
}
static void lookup(void *user, uint32_t operation, const char *title, size_t length, rwikitext_v1_reply *reply) {
    context *ctx = user; JNIEnv *env = ctx->env;
    jbyteArray title_bytes = NULL; jstring title_string = NULL; jobject result = NULL;
    jclass string_class = NULL, reply_class = NULL; jbyteArray value = NULL; char *json = NULL;
    if (length > INT32_MAX) goto failure;
    title_bytes = (*env)->NewByteArray(env, (jsize)length);
    if (!title_bytes) goto failure;
    (*env)->SetByteArrayRegion(env, title_bytes, 0, (jsize)length, (const jbyte *)title);
    if ((*env)->ExceptionCheck(env)) goto failure;
    string_class = (*env)->FindClass(env, "java/lang/String");
    if (!string_class) goto failure;
    jmethodID ctor = (*env)->GetMethodID(env, string_class, "<init>", "([BLjava/lang/String;)V");
    if (!ctor) goto failure;
    jstring charset = (*env)->NewStringUTF(env, "UTF-8");
    if (!charset) goto failure;
    title_string = (*env)->NewObject(env, string_class, ctor, title_bytes, charset);
    (*env)->DeleteLocalRef(env, charset);
    if (!title_string) goto failure;
    result = (*env)->CallObjectMethod(env, ctx->callback, ctx->method, (jint)operation, title_string);
    if ((*env)->ExceptionCheck(env) || !result) goto failure;
    reply_class = (*env)->GetObjectClass(env, result);
    if (!reply_class) goto failure;
    jfieldID status_field = (*env)->GetFieldID(env, reply_class, "status", "I");
    if (!status_field) goto failure;
    jfieldID value_field = (*env)->GetFieldID(env, reply_class, "valueJson", "[B");
    if (!value_field) goto failure;
    jint status = (*env)->GetIntField(env, result, status_field);
    value = (jbyteArray)(*env)->GetObjectField(env, result, value_field);
    if (value && !(json = bytes(env, value))) goto failure;
    rwikitext_v1_reply_set(reply, (uint32_t)status, json);
    goto done;
failure:
    if ((*env)->ExceptionCheck(env)) (*env)->ExceptionClear(env);
    rwikitext_v1_reply_set(reply, RWIKITEXT_V1_ERROR, "\"Java wiki callback failed\"");
done:
    free(json);
    if (value) (*env)->DeleteLocalRef(env, value);
    if (reply_class) (*env)->DeleteLocalRef(env, reply_class);
    if (result) (*env)->DeleteLocalRef(env, result);
    if (title_string) (*env)->DeleteLocalRef(env, title_string);
    if (string_class) (*env)->DeleteLocalRef(env, string_class);
    if (title_bytes) (*env)->DeleteLocalRef(env, title_bytes);
}
JNIEXPORT jbyteArray JNICALL Java_org_librewiki_rwikitext_WikiText_convertNative(JNIEnv *env, jclass cls, jbyteArray request, jobject callback) {
    (void)cls;
    if (rwikitext_v1_abi_version() != RWIKITEXT_ABI_VERSION) return NULL;
    char *input = bytes(env, request); if (!input) return NULL;
    context ctx = {env, callback, NULL};
    if (callback) {
        jclass c = (*env)->GetObjectClass(env, callback);
        if (!c) { free(input); return NULL; }
        ctx.method = (*env)->GetMethodID(env, c, "lookup", "(ILjava/lang/String;)Lorg/librewiki/rwikitext/WikiText$Reply;");
        (*env)->DeleteLocalRef(env, c);
        if (!ctx.method) { free(input); return NULL; }
    }
    const char *response = rwikitext_v1_convert(input, callback ? lookup : NULL, &ctx);
    free(input);
    if (!response) return NULL;
    size_t n = strlen(response); jbyteArray out = NULL;
    if (n <= INT32_MAX && (out = (*env)->NewByteArray(env, (jsize)n)))
        (*env)->SetByteArrayRegion(env, out, 0, (jsize)n, (const jbyte *)response);
    rwikitext_v1_free(response);
    return out;
}
