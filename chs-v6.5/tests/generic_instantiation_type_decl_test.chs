import "io"
import "vec"

type StringBuilder struct {
    data: Vec[string],
}

fn main() {
    var sb: StringBuilder = StringBuilder.{ data: Vec[string].{} };
    append(&sb.data, "test");
    print("Vec capacity: %\n", #anycast[sb.data.capacity]);
}
