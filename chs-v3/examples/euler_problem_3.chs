import "io.chs";

fn main() {
    var fac u64 = 2;
    var n u64 = 600851475143;
    while(n > 1) {
        if((n % fac) == 0) n = n / fac;
        else fac = fac + 1;
    }
    put_string("Answer: ");
    putln_int(fac);
}
