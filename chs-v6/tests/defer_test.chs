import "io"

fn test_normal_defer() {
    puts("test_normal_defer start");
    defer puts("defer 1 (should be second)");
    defer puts("defer 2 (should be first)");
    puts("test_normal_defer end");
}

fn test_return_defer(x: int) {
    puts("test_return_defer start");
    defer puts("cleanup at function return");
    
    if x > 0 {
        puts("returning early");
        return;
    };
    
    puts("not returning early");
}

fn test_loop_defer() {
    puts("test_loop_defer start");
    var i = 0;
    for i < 3 {
        var dummy = i;
        puts("loop iteration start");
        defer puts("loop iteration defer");
        
        if i == 1 {
            puts("loop continue");
            i += 1;
            continue;
        };
        
        if i == 2 {
            puts("loop break");
            break;
        };
        
        puts("loop iteration end");
        i += 1;
    };
    puts("test_loop_defer end");
}

fn main() {
    test_normal_defer();
    puts("---");
    test_return_defer(1);
    puts("---");
    test_return_defer(0);
    puts("---");
    test_loop_defer();
}
