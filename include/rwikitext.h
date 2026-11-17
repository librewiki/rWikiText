#ifndef RWIKITEXT_H
#define RWIKITEXT_H

#include <stddef.h>
#include <stdint.h>

#ifdef __cplusplus
extern "C" {
#endif

#define RWIKITEXT_ABI_VERSION 1

#define RWIKITEXT_V1_TEMPLATE_TEXT 1
#define RWIKITEXT_V1_PAGE_EXISTS 2
#define RWIKITEXT_V1_FILE_INFO 3

#define RWIKITEXT_V1_MISSING 0
#define RWIKITEXT_V1_FOUND 1
#define RWIKITEXT_V1_ERROR 2

typedef struct rwikitext_v1_reply rwikitext_v1_reply;

/* Synchronous on the conversion thread. Title is UTF-8 with the given byte
 * length. The title and reply are valid for this invocation only. A callback
 * that leaves reply untouched reports missing. A callback must return normally;
 * handle exceptions and nonlocal jumps inside the callback. */
typedef void (*rwikitext_v1_lookup_fn)(void *user_data, uint32_t operation,
                                     const char *title, size_t title_len,
                                     rwikitext_v1_reply *reply);

uint32_t rwikitext_v1_abi_version(void);

/* request_json is UTF-8 and NUL-terminated. A NULL lookup reports missing for
 * all titles. user_data is passed through unchanged. The returned JSON is owned
 * by the caller and must be released with rwikitext_v1_free. A NULL return
 * indicates an internal failure. See docs/native-bindings.md for the schema. */
const char *rwikitext_v1_convert(const char *request_json,
                                rwikitext_v1_lookup_fn lookup, void *user_data);

/* Accepts NULL. Each non-NULL response may be released once. */
void rwikitext_v1_free(const char *response);

/* Set the callback reply. MISSING takes NULL, FOUND takes a JSON value
 * (string for template text, boolean for existence, object for file info),
 * and ERROR takes a JSON string. Non-NULL value_json is UTF-8, NUL-terminated,
 * and copied before this function returns. Invalid status, UTF-8 or JSON
 * returns -1 and sets a lookup error for a valid reply. Success returns 0. */
int32_t rwikitext_v1_reply_set(rwikitext_v1_reply *reply, uint32_t status,
                              const char *value_json);

#ifdef __cplusplus
}
#endif

#endif
