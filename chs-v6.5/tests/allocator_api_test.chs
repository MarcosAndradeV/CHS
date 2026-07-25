import "io"
import "vec"
import "mem"
import "libc"

fn my_alloc(size: int) -> &void {
    printf("my_alloc called with size: %d\n".data, size);
    return alloc(size);
}

fn my_realloc(ptr: &void, size: int) -> &void {
    printf("my_realloc called with size: %d\n".data, size);
    return realloc(ptr, size);
}

fn my_dealloc(ptr: &void) {
    printf("my_dealloc called\n".data);
    dealloc(ptr);
}

fn my_allocator_impl(
    allocator_data: RawPtr,
    mode: AllocatorMode,
    size: int,
    alignment: int,
    old_ptr: RawPtr,
    old_size: int,
    ) -> ([]u8, AllocatorError) {
    switch mode {
    	AllocatorMode.Alloc -> {
            var ptr = my_alloc(size);
            var s: []u8 = #default;
            s.data = ptr;
            s.len = size;
            return (s, AllocatorError.None);
        };
	    AllocatorMode.Realloc -> {
            var new_ptr = my_realloc(old_ptr, size);
            var s: []u8 = #default;
            s.data = new_ptr;
            s.len = size;
            return (s, AllocatorError.None);
		};
	    AllocatorMode.Dealloc -> {
            my_dealloc(old_ptr);
            return (#default, AllocatorError.None)
		};
	    AllocatorMode.Clear -> return (#default, AllocatorError.ModeNotImplemented);
    };
    return (#default, AllocatorError.InvalidArgument)
}

fn main() {
    var my_allocator = Allocator.{
        impl: my_allocator_impl,
        data: null,
    };

    var xs = Vec.{};
    append(&xs, 42, allocator: my_allocator);
    append(&xs, 100, allocator: my_allocator);
    reset(&xs, allocator: my_allocator);
}
