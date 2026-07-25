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

typedef void *(*AllocFn)(int);
typedef void *(*ReallocFn)(void *, int);
typedef void (*DeallocFn)(void *);

typedef struct {
  AllocFn alloc;
  ReallocFn realloc;
  DeallocFn dealloc;
} ChsAllocator;

typedef struct {
    ChsAllocator allocator;
} ChsContext;

static ChsContext chs_context = {};

void chs_print(chs_string_t m);

void *chs_alloc(int size);
void *chs_realloc(void *ptr, int size);
void chs_dealloc(void *ptr);

void chs__oob_check(chs_string_t message, int idx, int len);

#endif // CHS_RUNTIME_H
