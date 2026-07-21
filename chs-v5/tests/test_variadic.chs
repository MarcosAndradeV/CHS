import "runtime"

fn printf(fmt: *u8, ...) #foreign Runtime #link_name "printf"

fn main() {
    var a = 42;
    var b = 99;
    printf("%d %d\n".data, a, b);
}
