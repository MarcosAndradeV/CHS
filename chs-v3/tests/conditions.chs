import "stdlib/io.chs";

const N = 10;

fn main() {
    var i int = 0;
    while(i < N) {
        if((i % 2) == 1)
            put_string("Even\n");
        else
            put_string("Odd\n");

        i = i + 1;
    }
}
