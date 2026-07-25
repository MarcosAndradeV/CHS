import "io"

fn unused_private_helper() #private {
    puts("this function is unused and private, so it should be stripped");
}

fn main() {
    puts("strip_unused_test passed successfully");
}
