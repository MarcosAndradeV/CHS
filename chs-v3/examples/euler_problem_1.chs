import "io.chs";

fn main() {
    var acc int = 0;
    var i int = 0;
    while(i < 1000) {
        if( ((i % 3) == 0) || ((i % 5) == 0) ) {
            acc = acc + i;
        }
        i = i + 1;
    }
    put_string("The sum is: ");
    putln_int(acc);
}
