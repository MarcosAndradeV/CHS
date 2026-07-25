#library Libc {
    link_name = "c",
    kind = "dynlib",
}

fn strcmp(lhs: &u8, rhs: &u8) -> int #foreign Libc #link_name "strcmp"
fn printf(fmt: &u8, ...) #foreign Libc #link_name "printf"
fn abort() -> noreturn #foreign Libc #link_name "abort"
fn memcpy(dest: &void, src: &void, n: int) -> &void #foreign Libc #link_name "memcpy"
fn memmove(dest: &void, src: &void, n: int) -> &void #foreign Libc #link_name "memmove"
fn memset(s: &void, c: int, n: int) -> &void #foreign Libc #link_name "memset"
fn memcmp(s1: &void, s2: &void, n: int) -> int #foreign Libc #link_name "memcmp"
