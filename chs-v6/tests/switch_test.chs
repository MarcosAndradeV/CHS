import "io"

fn test_block_switch(x: int) {
    switch x {
        1 -> {
            puts("entering block 1");
            defer puts("exiting block 1 (defer)");
            puts("leaving block 1");
        };
        _ -> puts("matched default");
    };
}

fn main() {
    test_block_switch(1);
    puts("---");
    test_block_switch(2);
}
