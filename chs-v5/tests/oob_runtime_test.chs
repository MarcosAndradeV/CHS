import "io"
import "panic"

fn print_arr(a: []int, idx: int) {
    print("%\n", #anycast[a[idx]]);
}

fn main() {
    var a = [1, 2, 3];
    print_arr(a, -1);
}
