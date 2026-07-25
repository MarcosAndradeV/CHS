# Ergonomic notes

- STATUS: CLOSED
- PRIORITY: 0

## Notes

- Pointers syntax should use only '&' symbol for types and operators
    - example 1: `var x: &int = &a;`
    - example 2: `fn copy(dest: &void, src: &void, size: int);`

- Functions should allow label positional arguments
  - example 1: `copy(dest: &d, src: &s, size: 10)`

- Default values
  - example 1: `var x: int = #default;` value defaults to `0`
  - example 2: `var x: string = #default;` value defaults to `""`
  - example 3: In structs every field implies `#default` as value allowing `Point.{}` defaults to `Point.{ x: 0, y: 0 }`
```
  type Point struct {
    x: int,
    y: int
  }
```
refer: tasks/20260723-014411