#library Libc {
    link_name = "c",
    kind = "dynlib",
}

fn strcmp(lhs: *u8, rhs: *u8) -> int #foreign Libc #link_name "strcmp"
fn printf(fmt: *u8, ...) #foreign Libc #link_name "printf"
fn abort() -> noreturn #foreign Libc #link_name "abort"
