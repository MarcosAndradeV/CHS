import "io"

struct Point {
    x: int,
    y: int,
}

fn main() {
    print("1. Array iteration:\n");
    var arr = [10, 20, 30];
    foreach x in arr {
        print("%d\n", #anycast[x]);
    };

    print("2. Slice iteration:\n");
    var slice: []int = [40, 50, 60];
    foreach x in slice {
        print("%d\n", #anycast[x]);
    };

    print("3. Pointer to array iteration:\n");
    var arr2 = [70, 80, 90];
    foreach x in &arr2 {
        print("%d\n", #anycast[x]);
    };

    print("4. Struct array iteration:\n");
    var points = [Point.{x: 1, y: 2}, Point.{x: 3, y: 4}];
    foreach p in points {
        print("%d %d\n", #anycast[p.x, p.y]);
    };

    print("5. Break and Continue:\n");
    var arr3 = [1, 2, 3, 4, 5];
    foreach x in arr3 {
        if x == 2 {
            continue;
        };
        if x == 4 {
            break;
        };
        print("%d\n", #anycast[x]);
    };
}
