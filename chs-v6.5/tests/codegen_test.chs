import "io"

type Color enum {
    RED,
    GREEN,
    BLUE
}

type Point struct {
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
