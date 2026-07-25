import "io"

fn private_func() #private {
    puts("private_func called");
}

fn public_func() {
    private_func(); // Local calls should succeed
}
