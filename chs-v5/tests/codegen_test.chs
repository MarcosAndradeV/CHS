import "io"

enum Color {
    RED,
    GREEN,
    BLUE
}

struct Point {
    x: int,
    y: int,
    color: Color,
}

fn takes_color(c: Color) {
    puts("takes_color called successfully");
}

fn main() {
    puts("Codegen test running...");
    takes_color(Color.RED);
}
