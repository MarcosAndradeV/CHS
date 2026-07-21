import "io"

struct A {
    b: B,
}

struct C {
    val: int,
}

struct B {
    c: C,
}

fn main() {
    puts("Topological sort test running...");
}
