import "io"

struct Inner {
    id: int,
    active: bool,
}

struct Big {
    name: string,
    inner: Inner,
    scores: [3]int = int.[0, 0, 0],
    ptr: *int = null,
}

enum Color {
    Red = 1,
    Green = 2,
    Blue = 3,
}

fn test_print() {
    var a = 42;
    var name = "Antigravity";
    print("Hello, %! The answer is %.\n", #anycast[name, a]);
}

fn test_struct_printing() {
    puts("test_struct_printing:");

    var val = 999;
    var big = Big.{
        name: "Antigravity Test",
        inner: Inner.{ id: 7, active: true },
    };
    big.scores[0] = 90;
    big.scores[1] = 95;
    big.scores[2] = 100;
    big.ptr = &val;

    print("  Struct: %\n", #anycast[big]);

    // print enum
    var color = Color.Green;
    print("  Enum: %\n", #anycast[color]);
}

fn main() {
    test_print();
    puts("====================");
    test_struct_printing();
}
