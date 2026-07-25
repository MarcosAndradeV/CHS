import "io"

type Config struct {
    host: string = "localhost",
    port: int = 8080,
    active: bool,
}

fn test_defaults() {
    puts("test_defaults:");
    
    // Explicit #default
    var x: int = #default;
    var f: float = #default;
    var b: bool = #default;
    var s: string = #default;
    var p: &int = #default;

    if x == 0 {
        puts("  #default int is 0");
    };
    if f == 0.0 {
        puts("  #default float is 0.0");
    };
    if b == false {
        puts("  #default bool is false");
    };
    if s.len == 0 {
        puts("  #default string has len 0");
    };
    if p == null {
        puts("  #default pointer is null");
    };

    // Implicit struct defaults
    var c = Config.{ host: "127.0.0.1" };
    print("  c.host: %\n", #anycast[c.host]);
    print("  c.port: %\n", #anycast[c.port]);
    print("  c.active: %\n", #anycast[c.active]);
}

fn my_func(x: int, y: string, z: bool) {
    print("  x: %, y: %, z: %\n", #anycast[x, y, z]);
}

fn test_labeled_arguments() {
    puts("test_labeled_arguments:");
    
    // Mix and match named / positional arguments
    my_func(y: "hello", x: 42, z: true);
    my_func(42, z: true, y: "world");
}

fn main() {
    test_defaults();
    puts("====================");
    test_labeled_arguments();
}
