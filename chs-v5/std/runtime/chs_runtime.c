#include <stdio.h>
#include <stdlib.h>
#include "chs_runtime.h"

void chs_print(chs_string_t m) {
  printf("%.*s", m.len, m.data);
}

void chs_print_int(int x) {
    printf("%d", x);
}

void *chs_alloc(int size) {
    return malloc(size);
}

void *chs_realloc(void *ptr, int size) {
    return realloc(ptr, size);
}

void chs_dealloc(void *ptr) {
    free(ptr);
}

void chs__oob_check(chs_string_t m, int idx, int len) {
    bool is_neg = idx < 0;
    bool is_oob = idx >= len;
    bool is_invalid = is_neg || is_oob;
    if (is_invalid) {
        fprintf(stderr, "%.*s\n", m.len, m.data);
        abort();
    }
}
