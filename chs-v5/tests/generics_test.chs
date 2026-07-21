import "io"

struct Pair[$T] {
    first: $T,
    second: $T,
}

fn identity[$T](val: $T) -> $T {
    return val;
}

fn test_identity() {
    puts("test_identity:");
    var x = identity(42);
    if x == 42 {
        puts("  identity(42) succeeded.");
    } else {
        puts("  identity(42) failed!");
    };

    var y = identity$[int](100);
    if y == 100 {
        puts("  identity$[int](100) succeeded.");
    } else {
        puts("  identity$[int](100) failed!");
    };

    var s = identity("hello");
    var target = "hello";
    if s.data == target.data {
        puts("  identity(\"hello\") succeeded.");
    } else {
        puts("  identity(\"hello\") failed!");
    };
}

fn test_generic_struct() {
    puts("test_generic_struct:");
    var pair = Pair$[int].{ first: 10, second: 20 };
    if pair.first == 10 {
        if pair.second == 20 {
            puts("  Pair[int] initialization succeeded.");
        } else {
            puts("  Pair[int] second field incorrect!");
        };
    } else {
        puts("  Pair[int] first field incorrect!");
    };
}

fn main() {
    test_identity();
    puts("====================");
    test_generic_struct();
}
