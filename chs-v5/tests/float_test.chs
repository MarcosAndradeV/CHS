import "io"

fn main() {
    var a = 12.34;
    var b = 5.67;

    // Arithmetic
    var c = a + b;
    var d = a - b;
    var e = a * b;
    var f = a / b;

    print("a + b = %\n", #anycast[c]);
    print("a - b = %\n", #anycast[d]);
    print("a * b = %\n", #anycast[e]);
    print("a / b = %\n", #anycast[f]);

    // Negation
    var neg_a = -a;
    print("-a = %\n", #anycast[neg_a]);

    // Casting
    var i_val = cast(int) a;
    print("cast(int) a = %\n", #anycast[i_val]);

    var f_val = cast(float) 42;
    print("cast(float) 42 = %\n", #anycast[f_val]);

    // Relations
    if a > b {
        puts("a > b is true");
    } else {
        puts("a > b is false");
    };

    if a < b {
        puts("a < b is true");
    } else {
        puts("a < b is false");
    };
}
