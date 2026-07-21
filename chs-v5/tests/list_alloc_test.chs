import "io"
import "mem"

struct List[$T] {
    items: *$T = null,
    count: int = 0,
    capacity: int = 0,
}

fn list_init[$T](xs: *List$[$T]) {
    xs.capacity = 2;
    var elem_size = #type_info($T).size;
    xs.items = cast(*$T) alloc(elem_size * xs.capacity);
    xs.count = 0;
}

fn list_append[$T](xs: *List$[$T], x: $T) {
    if xs.count == xs.capacity {
        xs.capacity = xs.capacity * 2;
        var elem_size = #type_info($T).size;
        xs.items = cast(*$T) realloc(cast(*void) xs.items, elem_size * xs.capacity);
    };
    xs.items[xs.count] = x;
    xs.count = xs.count + 1;
}

fn list_free[$T](xs: *List$[$T]) {
    if xs.items != null {
        dealloc(cast(*void) xs.items);
        xs.items = null;
    };
    xs.count = 0;
    xs.capacity = 0;
}

fn main() {
    puts("Initializing list...");
    var xs = List$[int].{};
    list_init(&xs);
    defer list_free(&xs);

    puts("Appending elements...");
    list_append(&xs, 10);
    list_append(&xs, 20);
    list_append(&xs, 30);
    list_append(&xs, 40);
    list_append(&xs, 50);

    print("xs count: %\n", #anycast[xs.count]);
    print("xs capacity: %\n", #anycast[xs.capacity]);

    var i = 0;
    for i < xs.count {
        print("xs[%] = %\n", #anycast[i, xs.items[i]]);
        i = i + 1;
    };
    puts("Done!");
}
