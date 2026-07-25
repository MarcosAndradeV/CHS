import "mem"

type Vec struct [$T] {
    items: &$T,
    count: int,
    capacity: int,
}

fn append[$T](xs: &Vec[$T], x: $T, allocator: Allocator = Allocator.{}) {
    if xs.count >= xs.capacity {
        if xs.capacity == 0 {
            xs.capacity = 256;
        } else {
            xs.capacity = xs.capacity * 2;
        };
        var elem_size = #type_info($T).size;
        xs.items = cast(&$T) realloc(cast(&void) xs.items, elem_size * xs.capacity, allocator: allocator);
    };
    xs.items[xs.count] = x;
    xs.count += 1;
}

fn reset[$T](xs: &Vec[$T], allocator: Allocator = Allocator.{}) {
    if xs.items != null {
        dealloc(cast(&void) xs.items, allocator: allocator);
        xs.items = null;
    };
    xs.count = 0;
    xs.capacity = 0;
}

fn get[$T](xs: &Vec[$T], idx: int) -> &$T {
    if xs.items == null || xs.count < idx {
        return null;
    };
    return &xs.items[idx];
}

fn slice[$T](xs: &Vec[$T]) -> []$T {
    var s: []$T = [];
    s.data = xs.items;
    s.len = xs.count;
    return s;
}

fn append_slice[$T](xs: &Vec[$T], sx: []$T, allocator: Allocator = Allocator.{}) {
    foreach x in sx {
        append(xs, x, allocator);
    }
}
