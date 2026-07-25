import "io"

fn test_pointer_arithmetic() {
    puts("test_pointer_arithmetic:");

    var arr: [5]int = int.[0, 0, 0, 0, 0];
    arr[0] = 100;
    arr[1] = 200;
    arr[2] = 300;
    arr[3] = 400;
    arr[4] = 500;

    var p0 = &arr[0];

    // addition
    var p1 = p0 + 1;
    var p2 = 2 + p0;
    var p3 = p0 + cast(u8) 3;

    print("  p0: %\n", #anycast[&p0]);
    print("  p0 + 1: %\n", #anycast[&p1]);
    print("  2 + p0: %\n", #anycast[&p2]);
    print("  p0 + 3 (u8 offset): %\n", #anycast[&p3]);

    // subtraction
    var p4 = p3 - 1;
    print("  (p0+3) - 1: %\n", #anycast[&p4]);

    // pointer diff
    var diff = p3 - p0;
    print("  (p0+3) - p0: %\n", #anycast[diff]);
}

fn test_pointer_comparisons() {
    puts("test_pointer_comparisons:");

    var x = 42;
    var y = 42;
    var px = &x;
    var py = &y;
    var px2 = &x;

    if px == px2 {
        puts("  px == px2 (same pointer) succeeded.");
    } else {
        puts("  px == px2 (same pointer) failed!");
    };

    if px != py {
        puts("  px != py (different pointers) succeeded.");
    } else {
        puts("  px != py (different pointers) failed!");
    };

    var pnull: &int = null;
    if pnull == null {
        puts("  pnull == null succeeded.");
    } else {
        puts("  pnull == null failed!");
    };
}

fn main() {
    test_pointer_arithmetic();
    puts("====================");
    test_pointer_comparisons();
}
