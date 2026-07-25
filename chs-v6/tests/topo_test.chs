import "io"

type A struct {
    b: B,
}

type C struct {
    val: int,
}

type B struct {
    c: C,
}

fn main() {
    puts("Topological sort test running...");
}
