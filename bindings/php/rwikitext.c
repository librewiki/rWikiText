#ifdef HAVE_CONFIG_H
#include "config.h"
#endif
#include <php.h>
#include <ext/json/php_json.h>
#include <Zend/zend_exceptions.h>
#include <Zend/zend_smart_str.h>
#include "rwikitext.h"

typedef struct {
    zval *callback;
    bool bailed_out;
} php_rwikitext_context;

static void lookup_error(rwikitext_v1_reply *reply, const char *message, size_t length)
{
    zval value;
    smart_str json = {0};
    ZVAL_STRINGL(&value, message, length);
    if (php_json_encode(&json, &value, PHP_JSON_INVALID_UTF8_SUBSTITUTE) == SUCCESS) {
        smart_str_0(&json);
        rwikitext_v1_reply_set(reply, RWIKITEXT_V1_ERROR, ZSTR_VAL(json.s));
    } else {
        rwikitext_v1_reply_set(reply, RWIKITEXT_V1_ERROR, "\"PHP callback failed\"");
    }
    smart_str_free(&json);
    zval_ptr_dtor(&value);
}

static void lookup_exception(rwikitext_v1_reply *reply)
{
    zval rv;
    zend_object *exception = EG(exception);
    if (zend_is_unwind_exit(exception) || zend_is_graceful_exit(exception)) {
        rwikitext_v1_reply_set(reply, RWIKITEXT_V1_ERROR, "\"PHP request exited during wiki lookup\"");
        return;
    }
    zval *message = zend_read_property(exception->ce, exception, "message",
                                      sizeof("message") - 1, 1, &rv);
    zend_string *copy = Z_TYPE_P(message) == IS_STRING ? zend_string_copy(Z_STR_P(message)) : NULL;
    zend_clear_exception();
    if (copy) {
        lookup_error(reply, ZSTR_VAL(copy), ZSTR_LEN(copy));
        zend_string_release(copy);
    } else {
        lookup_error(reply, ZEND_STRL("PHP callback failed"));
    }
}

static void perform_lookup(zval *callback, uint32_t operation, const char *title,
                           size_t title_len, rwikitext_v1_reply *reply)
{
    const char *name;
    switch (operation) {
        case RWIKITEXT_V1_TEMPLATE_TEXT: name = "template_text"; break;
        case RWIKITEXT_V1_PAGE_EXISTS: name = "page_exists"; break;
        case RWIKITEXT_V1_FILE_INFO: name = "file_info"; break;
        default: lookup_error(reply, ZEND_STRL("Unknown wiki lookup operation")); return;
    }
    zval args[2], result;
    smart_str json = {0};
    ZVAL_STRING(&args[0], name);
    ZVAL_STRINGL(&args[1], title, title_len);
    ZVAL_UNDEF(&result);
    int status = call_user_function(EG(function_table), NULL, callback, &result, 2, args);
    zval_ptr_dtor(&args[0]);
    zval_ptr_dtor(&args[1]);
    if (EG(exception)) {
        lookup_exception(reply);
    } else if (status != SUCCESS || Z_ISUNDEF(result)) {
        lookup_error(reply, ZEND_STRL("Could not call PHP wiki lookup callback"));
    } else if (Z_TYPE(result) == IS_NULL) {
        rwikitext_v1_reply_set(reply, RWIKITEXT_V1_MISSING, NULL);
    } else if (php_json_encode(&json, &result, 0) != SUCCESS) {
        lookup_error(reply, ZEND_STRL("Could not encode PHP wiki lookup result as JSON"));
    } else {
        smart_str_0(&json);
        rwikitext_v1_reply_set(reply, RWIKITEXT_V1_FOUND, ZSTR_VAL(json.s));
    }
    smart_str_free(&json);
    if (EG(exception)) lookup_exception(reply);
    if (!Z_ISUNDEF(result)) zval_ptr_dtor(&result);
    if (EG(exception)) lookup_exception(reply);
}

static void php_rwikitext_lookup(void *user_data, uint32_t operation, const char *title,
                                size_t title_len, rwikitext_v1_reply *reply)
{
    php_rwikitext_context *ctx = user_data;
    /* Contain Zend's longjmp within the callback so Rust can unwind normally. */
    zend_try {
        perform_lookup(ctx->callback, operation, title, title_len, reply);
    } zend_catch {
        ctx->bailed_out = true;
        rwikitext_v1_reply_set(reply, RWIKITEXT_V1_ERROR, "\"PHP request failed during wiki lookup\"");
    } zend_end_try();
}

ZEND_BEGIN_ARG_WITH_RETURN_TYPE_INFO_EX(arginfo_rwikitext_convert, 0, 1, IS_ARRAY, 0)
    ZEND_ARG_TYPE_INFO(0, request, IS_ARRAY, 0)
    ZEND_ARG_TYPE_INFO_WITH_DEFAULT_VALUE(0, lookup, IS_CALLABLE, 1, "null")
ZEND_END_ARG_INFO()

/**
 * @brief PHP function `rwikitext_convert(array $request, ?callable $lookup = null): array`.
 *
 * Converts a UTF-8 document using the rWikiText version 1 C ABI. See
 * `rwikitext.stub.php` for the full PHPDoc signature, array shapes, and PHP examples.
 */
PHP_FUNCTION(rwikitext_convert)
{
    zval *request, *callback = NULL;
    smart_str json = {0};
    const char *response;
    ZEND_PARSE_PARAMETERS_START(1, 2)
        Z_PARAM_ARRAY(request)
        Z_PARAM_OPTIONAL
        Z_PARAM_ZVAL(callback)
    ZEND_PARSE_PARAMETERS_END();
    if (callback && Z_TYPE_P(callback) != IS_NULL && !zend_is_callable(callback, 0, NULL)) {
        zend_argument_type_error(2, "must be a valid callback or null");
        RETURN_THROWS();
    }
    if (rwikitext_v1_abi_version() != RWIKITEXT_ABI_VERSION) {
        zend_throw_exception(NULL, "Unsupported rWikiText C ABI version", 0);
        RETURN_THROWS();
    }
    if (php_json_encode(&json, request, 0) != SUCCESS || EG(exception)) {
        smart_str_free(&json);
        if (!EG(exception)) zend_throw_exception(NULL, "Could not encode rWikiText request as JSON", 0);
        RETURN_THROWS();
    }
    smart_str_0(&json);
    php_rwikitext_context ctx = { callback, false };
    response = rwikitext_v1_convert(ZSTR_VAL(json.s), callback && Z_TYPE_P(callback) != IS_NULL ? php_rwikitext_lookup : NULL, &ctx);
    smart_str_free(&json);
    if (ctx.bailed_out) {
        rwikitext_v1_free(response);
        zend_bailout();
    }
    if (EG(exception)) {
        rwikitext_v1_free(response);
        RETURN_THROWS();
    }
    if (!response) {
        zend_throw_exception(NULL, "rWikiText conversion failed internally", 0);
        RETURN_THROWS();
    }
    int status = php_json_decode_ex(return_value, response, strlen(response), PHP_JSON_OBJECT_AS_ARRAY, PHP_JSON_PARSER_DEFAULT_DEPTH);
    rwikitext_v1_free(response);
    if (status != SUCCESS || Z_TYPE_P(return_value) != IS_ARRAY) {
        zval_ptr_dtor(return_value);
        zend_throw_exception(NULL, "Invalid JSON response from rWikiText", 0);
        RETURN_THROWS();
    }
}

static const zend_function_entry rwikitext_functions[] = {
    PHP_FE(rwikitext_convert, arginfo_rwikitext_convert)
    PHP_FE_END
};
zend_module_entry rwikitext_module_entry = {
    STANDARD_MODULE_HEADER, "rwikitext", rwikitext_functions, NULL, NULL, NULL, NULL, NULL,
    "0.1.0", STANDARD_MODULE_PROPERTIES
};
#ifdef COMPILE_DL_RWIKITEXT
#ifdef ZTS
ZEND_TSRMLS_CACHE_DEFINE()
#endif
ZEND_GET_MODULE(rwikitext)
#endif
