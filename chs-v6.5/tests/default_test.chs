import "io"

type Config struct {
    port: int = 8080,
    host: string = "localhost",
}

fn test_struct_defaults() {
    puts("test_struct_defaults:");

    var c1 = Config.{ port: 3000, host: "127.0.0.1" };
    puts(c1.host);
    if c1.port == 3000 {
        puts("c1 port is 3000");
    };

    var c2 = Config.{ host: "google.com" };
    puts(c2.host);
    if c2.port == 8080 {
        puts("c2 port is default (8080)");
    };

    var c3 = Config.{};
    puts(c3.host);
    if c3.port == 8080 {
        puts("c3 port is default (8080)");
    };
}

fn greet(name: string = "Visitor", greet_word: string = "Hello") {
    puts(greet_word);
    puts(name);
}

fn test_fn_defaults() {
    puts("test_fn_defaults:");
    greet(name: "Alice", greet_word: "Welcome");
    puts("---");
    greet(name: "Bob");
    puts("---");
    greet();
}

fn main() {
    test_struct_defaults();
    puts("====================");
    test_fn_defaults();
}
