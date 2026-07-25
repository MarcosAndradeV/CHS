import "libc"

fn alloc_zero(size: int, allocator: Allocator = Allocator.{}) -> &void {
    var ptr = alloc(size, allocator: allocator);
    if ptr != null {
        memset(ptr, 0, size);
    };
    return ptr;
}

fn copy(dest: &void, src: &void, size: int) {
    memcpy(dest, src, size);
}

fn move(dest: &void, src: &void, size: int) {
    memmove(dest, src, size);
}

fn set(ptr: &void, val: int, size: int) {
    memset(ptr, val, size);
}

fn compare(ptr1: &void, ptr2: &void, size: int) -> int {
    return memcmp(ptr1, ptr2, size);
}

fn clone(ptr: &void, size: int, allocator: Allocator = Allocator.{}) -> &void {
    var new_ptr = alloc(size, allocator: allocator);
    if new_ptr != null {
        memcpy(new_ptr, ptr, size);
    };
    return new_ptr;
}
