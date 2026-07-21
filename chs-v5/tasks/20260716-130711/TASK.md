# Add out of bounds check

- STATUS: CLOSED
- PRIORITY: 200

Should panic in runtime

```
import io

fn print_arr(a: []int) {
    print(%n, #anycast[a[4]]); // <- Should panic in runtime
}
```

If in compile time the expr length and the index are know constants it should not compile

```
import io

fn main() {
    var a = [1, 2, 3];
    print(%n, #anycast[a[4]]); // <- Should not compile
}
```