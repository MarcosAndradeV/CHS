import "io"

fn main() {
    var x = 5;
    var y = 10;
    var p = &x;
    var q = &y;
    var z = *p * *q;
    print("z is %\n", #anycast[z]);
}
