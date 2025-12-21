
// io
extern int printf (const char *__restrict __format, ...);

void put_int(int x) { printf("%d", x); }
void putln_int(int x) { printf("%d\n", x); }

void put_char(char x) { printf("%c", x); }
void putln_char(char x) { printf("%c\n", x); }

void put_string(const char* x) { printf("%s", x); }
void putln_string(const char* x) { printf("%s\n", x); }

void put_bool(int x) { x ? printf("true") : printf("false"); }
void putln_bool(int x) { x ? printf("true\n") : printf("false\n"); }
