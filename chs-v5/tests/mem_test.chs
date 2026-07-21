import "io"
import "mem"

fn main() {
    puts("test_mem_module:");

    // Test alloc_zero
    var p1 = alloc_zero(8);
    var p1_ints = cast(*int) p1;
    if *p1_ints == 0 {
        puts("  alloc_zero initialized to 0 succeeded.");
    } else {
        puts("  alloc_zero initialized to 0 failed!");
    };

    // Test set
    *p1_ints = 42;
    set(p1, 0, 8);
    if *p1_ints == 0 {
        puts("  set succeeded.");
    } else {
        puts("  set failed!");
    };

    // Test copy and clone
    var p2 = alloc(8);
    var p2_ints = cast(*int) p2;
    *p2_ints = 9999;

    copy(p1, p2, 8);
    if *p1_ints == 9999 {
        puts("  copy succeeded.");
    } else {
        puts("  copy failed!");
    };

    var p3 = clone(p1, 8);
    var p3_ints = cast(*int) p3;
    if *p3_ints == 9999 {
        puts("  clone succeeded.");
    } else {
        puts("  clone failed!");
    };

    // Test compare
    var cmp_same = compare(p1, p3, 8);
    if cmp_same == 0 {
        puts("  compare (same data) succeeded.");
    } else {
        puts("  compare (same data) failed!");
    };

    *p3_ints = 1234;
    var cmp_diff = compare(p1, p3, 8);
    if cmp_diff != 0 {
        puts("  compare (different data) succeeded.");
    } else {
        puts("  compare (different data) failed!");
    };

    dealloc(p1);
    dealloc(p2);
    dealloc(p3);
}
