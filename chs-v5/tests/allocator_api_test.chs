import "io"
import "vec"
import "mem"
import "libc"

fn my_alloc(size: int) -> *void {
    printf("my_alloc called with size: %d\n".data, size);
    return alloc(size);
}

fn my_realloc(ptr: *void, size: int) -> *void {
    printf("my_realloc called with size: %d\n".data, size);
    return realloc(ptr, size);
}

fn my_dealloc(ptr: *void) {
    printf("my_dealloc called\n".data);
    dealloc(ptr);
}

fn main() {
    var my_allocator = Allocator.{
        alloc: my_alloc,
        realloc: my_realloc,
        dealloc: my_dealloc,
    };

    var xs = Vec.{ allocator: my_allocator };
    append(&xs, 42);
    append(&xs, 100);
    reset(&xs);
}
