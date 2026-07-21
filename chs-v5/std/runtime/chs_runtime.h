#ifndef CHS_RUNTIME_H
#define CHS_RUNTIME_H

#include <stdbool.h>
#include <stdint.h>
#include <stdio.h>
#include <string.h>

#define CHS_NULL NULL

#define CHS_MAIN                                                               \
  int main(int argc, char **argv) {                                            \
    chs_main();                                                                \
    return 1;                                                                  \
  }

typedef struct {
  char *data;
  int len;
} chs_string_t;

void chs_print(chs_string_t m);

void *chs_alloc(int size);
void *chs_realloc(void *ptr, int size);
void chs_dealloc(void *ptr);

void chs__oob_check(chs_string_t message, int len, int idx);

#endif // CHS_RUNTIME_H
