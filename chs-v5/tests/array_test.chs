import "io"

fn test_array_basic() {
    puts("test_array_basic:");

    var arr: [3]int = [10, 20, 30];

    if arr[0] == 10 {
        puts("arr[0] is 10");
    };
    if arr[1] == 20 {
        puts("arr[1] is 20");
    };
    if arr[2] == 30 {
        puts("arr[2] is 30");
    };

    arr[1] = 42;
    if arr[1] == 42 {
        puts("arr[1] modified to 42");
    };
}

fn print_slice(prefix: string, s: []int) {
    puts(prefix);
    if s[0] == 10 {
        puts("slice[0] is 10");
    };
    if s[1] == 20 {
        puts("slice[1] is 20");
    };
    if s[2] == 30 {
        puts("slice[2] is 30");
    };
    if s[1] == 42 {
        puts("slice[1] is 42");
    };
}

fn test_slices() {
    puts("test_slices:");

    var s: []int = [10, 20, 30];
    print_slice("slice from literal:", s);

    var arr: [3]int = [10, 42, 30];
    print_slice("slice from variable:", arr);
}

fn main() {
    test_array_basic();
    puts("====================");
    test_slices();
}
