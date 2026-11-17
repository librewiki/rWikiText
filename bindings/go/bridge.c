#include <stdint.h>
#include "rwikitext.h"

extern void rwikitextGoLookup(uintptr_t handle, uint32_t operation,
                              char *title, size_t title_len,
                              rwikitext_v1_reply *reply);

static void lookup(void *user_data, uint32_t operation, const char *title,
                   size_t title_len, rwikitext_v1_reply *reply) {
    uintptr_t handle = *(uintptr_t *)user_data;
    rwikitextGoLookup(handle, operation, (char *)title, title_len, reply);
}

const char *rwikitext_go_convert(const char *request, uintptr_t handle) {
    if (handle == 0) {
        return rwikitext_v1_convert(request, NULL, NULL);
    }
    return rwikitext_v1_convert(request, lookup, &handle);
}
