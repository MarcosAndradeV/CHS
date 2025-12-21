import "io.chs";
import "talloc.chs";

fn main() {
    var a *int = talloc(4);
    *a = 10;
    var b *int = talloc(4);
    *b = 20;
    putln_int(*a);
    putln_int(*b);
}
