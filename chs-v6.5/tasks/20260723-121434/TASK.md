# Fix instansiation in type declaration

- STATUS: CLOSED
- PRIORITY: 900

## File *temp/temp.chs*
```chs
import "vec"

type StringBuilder struct {
    data: Vec[string],
}

fn main() {

}
```

## Output

```console
$ cargo r -q -- run temp/temp.chs

thread 'main' (76778) panicked at ir/src/types.rs:82:18:
Expected fully defined struct or tuple type
note: run with `RUST_BACKTRACE=1` environment variable to display a backtrace
```