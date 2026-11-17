#include "rwikitext.h"

#include <assert.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>

struct calls {
    unsigned templates;
    unsigned pages;
    unsigned files;
};

static _Thread_local struct calls *calling_thread;

static int is_title(const char *title, size_t length, const char *expected) {
    return length == strlen(expected) && memcmp(title, expected, length) == 0;
}

static void lookup(void *data, uint32_t operation, const char *title,
                   size_t length, rwikitext_v1_reply *reply) {
    struct calls *calls = data;
    assert(calls == calling_thread);
    int32_t status = 0;
    int32_t expected_status = 0;
    switch (operation) {
    case RWIKITEXT_V1_TEMPLATE_TEXT:
        ++calls->templates;
        if (is_title(title, length, "Template:Greeting")) {
            status = rwikitext_v1_reply_set(reply, RWIKITEXT_V1_FOUND, "\"Hello, {{{1}}}!\"");
        } else if (is_title(title, length, "Template:Offline")) {
            status = rwikitext_v1_reply_set(reply, RWIKITEXT_V1_ERROR, "\"wiki unavailable\"");
        } else if (is_title(title, length, "Template:Malformed")) {
            status = rwikitext_v1_reply_set(reply, RWIKITEXT_V1_FOUND, "{");
            expected_status = -1;
        }
        break;
    case RWIKITEXT_V1_PAGE_EXISTS:
        ++calls->pages;
        assert(is_title(title, length, "Absent"));
        status = rwikitext_v1_reply_set(reply, RWIKITEXT_V1_FOUND, "false");
        break;
    case RWIKITEXT_V1_FILE_INFO:
        ++calls->files;
        assert(is_title(title, length, "File:Example.png"));
        status = rwikitext_v1_reply_set(reply, RWIKITEXT_V1_FOUND,
                                        "{\"url\":\"https://example.org/image.png\",\"width\":300,\"height\":200}");
        break;
    }
    if (status != expected_status) {
        abort();
    }
}

static void contains(const char *json, const char *expected) {
    if (json == NULL || strstr(json, expected) == NULL) {
        fprintf(stderr, "Expected %s in response: %s\n", expected, json == NULL ? "(null)" : json);
        abort();
    }
}

int main(void) {
    struct calls calls = {0};
    calling_thread = &calls;
    assert(rwikitext_v1_abi_version() == RWIKITEXT_ABI_VERSION);
    const char *response = rwikitext_v1_convert(
        "{\"source\":\"wikitext\",\"target\":\"gfm\","
        "\"input\":\"{{Greeting|{{PAGENAME}}}} [[Absent]] [[File:Example.png|alt=Example]]\\n[[Category:Demo|Sort]]\","
        "\"context\":{\"page_title\":\"Guide\"}}", lookup, &calls);
    contains(response, "\"ok\":true");
    contains(response, "Hello\\\\, Guide\\\\!");
    contains(response, "https://example.org/image.png");
    contains(response, "\"categories\":[\"Demo\"]");
    contains(response, "\"category_sort_keys\":[\"Sort\"]");
    contains(response, "\"kind\":\"conversion_loss\"");
    assert(calls.templates == 1 && calls.pages == 1 && calls.files == 1);
    puts(response);
    rwikitext_v1_free(response);

    const char *missing_request =
        "{\"source\":\"wikitext\",\"target\":\"html\",\"input\":\"{{Absent}}\"}";
    response = rwikitext_v1_convert(missing_request, lookup, &calls);
    contains(response, "\"ok\":true");
    contains(response, "Template:Absent");
    assert(calls.templates == 2);
    const char *without_lookup = rwikitext_v1_convert(missing_request, NULL, NULL);
    if (without_lookup == NULL || strcmp(without_lookup, response) != 0) {
        abort();
    }
    rwikitext_v1_free(without_lookup);
    rwikitext_v1_free(response);

    response = rwikitext_v1_convert(
        "{\"source\":\"wikitext\",\"target\":\"gfm\",\"input\":\"{{Offline}}\"}", lookup, &calls);
    contains(response, "\"ok\":false");
    contains(response, "wiki unavailable");
    contains(response, "\"kind\":\"wiki_lookup\"");
    contains(response, "\"start\":0");
    contains(response, "\"end\":11");
    rwikitext_v1_free(response);

    response = rwikitext_v1_convert(
        "{\"source\":\"wikitext\",\"target\":\"html\",\"input\":\"{{Malformed}}\"}", lookup, &calls);
    contains(response, "\"ok\":false");
    contains(response, "\"kind\":\"wiki_lookup\"");
    contains(response, "invalid lookup reply");
    rwikitext_v1_free(response);

    response = rwikitext_v1_convert(
        "{\"source\":\"wikitext\",\"target\":\"gfm\",\"input\":\"[[Category:Demo]]\",\"mode\":\"strict\"}",
        NULL, NULL);
    contains(response, "\"ok\":false");
    contains(response, "\"kind\":\"conversion_loss\"");
    rwikitext_v1_free(response);
    return 0;
}
