# Breaking changes

- STATUS: OPEN
- PRIORITY: 1000

After using the current iteration i found some points needing attention

- No module namespace : PRIORITY : 900
- Overloading does not pair well with default/label arguments : PRIORITY : 800
- Global variables are not supported : PRIORITY : 700
- No proper variadic arguments support in chs functions. Using #anycast[] is a hack! : PRIORITY : 100

# Proposed solutions

## Just support global variables
```
var global_x: int = 10

fn main() {
  var a = global_x;
  global_x += 1;
}
```
Maybe even add some thread_local support since qbe does support it
```
#thread_local {
  var a_memory_buffer: [1024]u8 = #default
}
```

## Module namespace
Modify the current importing syntax `import std/io` where the "path" is a identifiers separeted by `/` that matches a filesystem folder structure and the last identifier is the module name
Example:
```
import std/io

fn main() {
  io.print("Hello, world\n");
}
```

## Proper variadics
A proper variadic syntax 
```
fn print(fmt: string, args: ...Any) {}
```
where `...Type` means `[]Type` and the typechecking should handle propely
if `...int` only accept a variadic amount of `int`'s
allowing the syntax without `#anycast[]`
```
print("% + % = %", 1, 2, 3);
```

## Overloading 
Was fun to use and implement it but it usefullness is mostly seen in operator overloading
so we need to remove the ability of the user to define arbritraly overloading and focus on only operators with a proper arity check like binary operator should only be defined with 2 arguments unary with 1 argument.
