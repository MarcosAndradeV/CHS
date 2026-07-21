import "io"

struct Point {
    x: int,
    y: int,
}

fn test_structs() {
    var p = Point.{ x: 10, y: 20 };
    puts("Struct literal initialized. Accessing fields...");

    // Testing complex assignment (LValue resolution)
    p.x = 100;
    p.y = 200;

    if p.x == 100 {
        puts("Complex assignment for x succeeded.");
    } else {
        puts("Complex assignment for x failed!");
    };

    if p.y == 200 {
        puts("Complex assignment for y succeeded.");
    } else {
        puts("Complex assignment for y failed!");
    };
}

fn test_compound_assignments() {
    var x = 10;
    x += 5;
    if x == 15 {
        puts("Compound assignment (+=) succeeded.");
    } else {
        puts("Compound assignment (+=) failed!");
    };

    x -= 3;
    if x == 12 {
        puts("Compound assignment (-=) succeeded.");
    } else {
        puts("Compound assignment (-=) failed!");
    };
}

fn test_loops() {
    var i = 0;
    puts("Starting for loop...");
    for i < 5 {
        i += 1;
        if i == 3 {
            continue;
        };
        if i == 5 {
            break;
        };
    };
    if i == 5 {
        puts("Loops and loop controls break/continue succeeded.");
    } else {
        puts("Loops and loop controls failed!");
    };
}

fn main() {
    test_structs();
    test_compound_assignments();
    test_loops();
}
