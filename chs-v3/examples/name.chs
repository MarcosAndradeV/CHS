import "linux.chs";
import "io.chs";

var name[256] char;
fn main() -> int {
    put_string("What is your name?\n");
    var len int = read(STDIN, name, 256);
    if (len <= 0) {
        put_string("Could not read name\n");
        exit(1);
    }
    put_string("Hello "); put_string(name);
    return 0;
}
