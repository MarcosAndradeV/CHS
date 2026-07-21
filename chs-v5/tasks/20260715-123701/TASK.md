# Add panic function to std

- STATUS: CLOSED
- PRIORITY: 100

update libc.chs with
```
#library Libc {
    link_name = "c",
    kind = "dynlib",
}

fn strcmp(lhs: *u8, rhs: *u8) -> int #foreign Libc #link_name "strcmp"
fn printf(fmt: *u8, ...) #foreign Libc #link_name "printf"
fn abort() -> void #foreign Libc #link_name "abort"
```
create panic.chs in std
```
import "io"
import "libc"

fn panic(message: string) -> noreturn {}
fn panic(message: string, args: []Any) -> noreturn {
    print(message, args);
    abort();
}
```