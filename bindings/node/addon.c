#include <node_api.h>
#include <dlfcn.h>
#include <stdint.h>
#include <stdbool.h>
#include <stdlib.h>
#include "../../include/rwikitext.h"

typedef uint32_t (*version_fn)(void);
typedef const char *(*convert_fn)(const char *, rwikitext_v1_lookup_fn, void *);
typedef void (*free_fn)(const char *);
typedef int32_t (*set_fn)(rwikitext_v1_reply *, uint32_t, const char *);
typedef struct { napi_env env; napi_value callback; set_fn set; } lookup_context;

static char *utf8(napi_env env, napi_value value) {
    size_t n, written;
    if (napi_get_value_string_utf8(env, value, NULL, 0, &n) != napi_ok) return NULL;
    char *s = malloc(n + 1);
    if (!s) return NULL;
    if (napi_get_value_string_utf8(env, value, s, n + 1, &written) != napi_ok) { free(s); return NULL; }
    return s;
}

static void lookup(void *opaque, uint32_t operation, const char *title, size_t length, rwikitext_v1_reply *reply) {
    lookup_context *ctx = opaque;
    napi_handle_scope scope;
    if (napi_open_handle_scope(ctx->env, &scope) != napi_ok) {
        ctx->set(reply, RWIKITEXT_V1_ERROR, "\"could not open Node lookup scope\"");
        return;
    }

    napi_value global, args[2], result, status_value, value;
    uint32_t status;
    char *json = NULL;
    if (napi_get_global(ctx->env, &global) != napi_ok ||
        napi_create_uint32(ctx->env, operation, &args[0]) != napi_ok ||
        napi_create_string_utf8(ctx->env, title, length, &args[1]) != napi_ok ||
        napi_call_function(ctx->env, global, ctx->callback, 2, args, &result) != napi_ok ||
        napi_get_element(ctx->env, result, 0, &status_value) != napi_ok ||
        napi_get_value_uint32(ctx->env, status_value, &status) != napi_ok || status > RWIKITEXT_V1_ERROR ||
        (status && (napi_get_element(ctx->env, result, 1, &value) != napi_ok || !(json = utf8(ctx->env, value))))) {
        bool pending = false;
        napi_is_exception_pending(ctx->env, &pending);
        if (pending) { napi_value exception; napi_get_and_clear_last_exception(ctx->env, &exception); }
        ctx->set(reply, RWIKITEXT_V1_ERROR, "\"Node lookup bridge failed\"");
    } else {
        ctx->set(reply, status, json);
    }
    free(json);
    napi_close_handle_scope(ctx->env, scope);
}

static napi_value convert(napi_env env, napi_callback_info info) {
    size_t argc = 3;
    napi_value argv[3], result;
    char *path = NULL, *request = NULL;
    void *library = NULL;
    const char *response = NULL;
    if (napi_get_cb_info(env, info, &argc, argv, NULL, NULL) != napi_ok || argc != 3 ||
        !(path = utf8(env, argv[0])) || !(request = utf8(env, argv[1]))) {
        napi_throw_type_error(env, NULL, "expected library path, request JSON, and callback");
        goto done;
    }
    napi_valuetype callback_type;
    if (napi_typeof(env, argv[2], &callback_type) != napi_ok || callback_type != napi_function) {
        napi_throw_type_error(env, NULL, "lookup must be a function"); goto done;
    }
    library = dlopen(path, RTLD_NOW | RTLD_LOCAL);
    if (!library) { napi_throw_error(env, NULL, dlerror()); goto done; }
    version_fn version = (version_fn)dlsym(library, "rwikitext_v1_abi_version");
    convert_fn run = (convert_fn)dlsym(library, "rwikitext_v1_convert");
    free_fn release = (free_fn)dlsym(library, "rwikitext_v1_free");
    set_fn set = (set_fn)dlsym(library, "rwikitext_v1_reply_set");
    if (!version || !run || !release || !set || version() != RWIKITEXT_ABI_VERSION) {
        napi_throw_error(env, NULL, "missing or unsupported rWikiText ABI"); goto done;
    }
    lookup_context context = { env, argv[2], set };
    response = run(request, lookup, &context);
    if (!response) { napi_throw_error(env, NULL, "rWikiText native conversion failed"); goto done; }
    napi_status status = napi_create_string_utf8(env, response, NAPI_AUTO_LENGTH, &result);
    release(response);
    if (status != napi_ok) { napi_throw_error(env, NULL, "could not decode native response"); goto done; }
    free(path); free(request); dlclose(library);
    return result;
done:
    free(path); free(request); if (library) dlclose(library);
    return NULL;
}

static napi_value init(napi_env env, napi_value exports) {
    napi_value fn;
    if (napi_create_function(env, "convert", NAPI_AUTO_LENGTH, convert, NULL, &fn) != napi_ok ||
        napi_set_named_property(env, exports, "convert", fn) != napi_ok) return NULL;
    return exports;
}
NAPI_MODULE(NODE_GYP_MODULE_NAME, init)
