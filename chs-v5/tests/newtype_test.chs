import "io"

type Seconds #distinct int
type Minutes #distinct int

struct Point {
    x: int,
    y: int
}
type SecondsPoint #distinct Point

fn print_seconds(s: Seconds) {
    print("seconds: %\n", #anycast[cast(int) s]);
}

fn print_point(p: SecondsPoint) {
    puts("inside print_point");
    // Member access should transparently work on distinct type!
    print("point: % %\n", #anycast[p.x, p.y]);
}

fn main() {
    var s = cast(Seconds) 42;
    print_seconds(s);

    // Cast between distinct types sharing the same underlying type
    var m = cast(Minutes) s;
    print("minutes: %\n", #anycast[cast(int) m]);

    // Distinct struct wrapper
    var p = cast(SecondsPoint) Point.{ x: 10, y: 20 };
    print_point(p);
}
