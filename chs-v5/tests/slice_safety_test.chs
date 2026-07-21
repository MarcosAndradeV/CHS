import "io"

fn get_unsafe_slice() -> []int {
    var arr: [3]int = [10, 20, 30];
    return #unsafe arr;
}

fn main() {
    var s = get_unsafe_slice();
    if s[0] == 10 {
        puts("Bypassed safety check successfully with #unsafe.");
    };
}
