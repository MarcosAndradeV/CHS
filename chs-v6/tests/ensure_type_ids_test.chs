import "io"
import "vec"

fn main() {
    print("Hello, CHS\n");
    // var xs: Vec[int] = Vec.{};
    // var xs = Vec[int].{};
    var xs = Vec.{};
    append(&xs, 1);
    defer reset(&xs);
    print("%\n", #anycast[xs]);
}
