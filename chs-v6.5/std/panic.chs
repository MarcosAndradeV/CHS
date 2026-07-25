import "io"
import "libc"

fn panic(message: string, args: ...Any) -> noreturn {
    print(message, args);
    abort();
}
