import "io"

type Point struct {
    x: int,
    y: int,
}

fn test_basic_refer_deref() {
    puts("test_basic_refer_deref:");
    var x = 42;
    var p = &x;
    if &p == 42 {
        puts("  Read via pointer succeeded.");
    } else {
        puts("  Read via pointer failed!");
    };

    &p = 99;
    if x == 99 {
        puts("  Write via pointer succeeded.");
    } else {
        puts("  Write via pointer failed!");
    };
}

fn test_struct_auto_deref() {
    puts("test_struct_auto_deref:");
    var p = Point.{ x: 10, y: 20 };
    var ptr = &p;

    // Test RHS auto-deref
    if ptr.x == 10 {
        puts("  RHS auto-deref struct pointer succeeded.");
    } else {
        puts("  RHS auto-deref struct pointer failed!");
    };

    // Test LHS auto-deref
    ptr.x = 200;
    if p.x == 200 {
        puts("  LHS auto-deref struct pointer succeeded.");
    } else {
        puts("  LHS auto-deref struct pointer failed!");
    };

    // Test nested pointer auto-deref
    var pptr = &ptr;
    pptr.y = 300;
    if p.y == 300 {
        puts("  Nested pointer auto-deref succeeded.");
    } else {
        puts("  Nested pointer auto-deref failed!");
    };
}

fn main() {
    test_basic_refer_deref();
    puts("====================");
    test_struct_auto_deref();
}
