import "io"

fn print_info(id: int, name: string = "Guest", active: bool = false) {
    print("id: %, name: %, active: %\n", #anycast[id, name, active]);
}

fn main() {
    print_info(100);
    print_info(200, name: "Alice");
    print_info(300, active: true);
    print_info(400, name: "Bob", active: true);
}
