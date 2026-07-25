import "io"
import "panic"

fn test_noreturn_func() -> int {
    panic("panicking now\n");
}

fn test_noreturn_args() -> bool {
    var code = 404;
    panic("not found with code %\n", #anycast[code]);
}

fn main() {
    print("starting panic test\n");
}
