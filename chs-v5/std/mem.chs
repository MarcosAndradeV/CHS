import "runtime"

// Raw libc memory functions (private to the module)
fn libc_memcpy(dest: *void, src: *void, n: int) -> *void #foreign Runtime #link_name "memcpy" #private
fn libc_memmove(dest: *void, src: *void, n: int) -> *void #foreign Runtime #link_name "memmove" #private
fn libc_memset(s: *void, c: int, n: int) -> *void #foreign Runtime #link_name "memset" #private
fn libc_memcmp(s1: *void, s2: *void, n: int) -> int #foreign Runtime #link_name "memcmp" #private

// Core allocation (defined in runtime)
fn alloc(size: int) -> *void #foreign Runtime #link_name "chs_alloc"
fn realloc(ptr: *void, size: int) -> *void #foreign Runtime #link_name "chs_realloc"
fn dealloc(ptr: *void) #foreign Runtime #link_name "chs_dealloc"

type AllocFn fn(size: int) -> *void
type ReallocFn fn(ptr: *void, size: int) -> *void
type DeallocFn fn(ptr: *void)

struct Allocator {
    alloc: AllocFn = alloc,
    realloc: ReallocFn = realloc,
    dealloc: DeallocFn = dealloc,
}

// CHS Safe/Clean Utility Wrapper Functions
fn alloc_zero(size: int) -> *void {
    var ptr = alloc(size);
    if ptr != null {
        libc_memset(ptr, 0, size);
    };
    return ptr;
}

fn copy(dest: *void, src: *void, size: int) {
    libc_memcpy(dest, src, size);
}

fn move(dest: *void, src: *void, size: int) {
    libc_memmove(dest, src, size);
}

fn set(ptr: *void, val: int, size: int) {
    libc_memset(ptr, val, size);
}

fn compare(ptr1: *void, ptr2: *void, size: int) -> int {
    return libc_memcmp(ptr1, ptr2, size);
}

fn clone(ptr: *void, size: int) -> *void {
    var new_ptr = alloc(size);
    if new_ptr != null {
        libc_memcpy(new_ptr, ptr, size);
    };
    return new_ptr;
}
