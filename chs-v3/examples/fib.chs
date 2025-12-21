import "io.chs";

fn fib(x int) {
    var a int = 0;
    var b int = 1;
    while(a <= x) {
        put_int(a);
        var c int = b;
        b = a + b;
        a = c;
        if(a < x){
            put_string(",");
        }
        put_string(" ");
    }
    putln_string("");
}

fn main() {
    fib(100);
}
