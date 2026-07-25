# Add support for named parameters in functions

- STATUS: CLOSED
- PRIORITY: 200

The current default parameters are positional, i want then named parameters

Example:

```
fn foo(pos: int, def1 = 123) {...}

foo(123);

foo(123, def1: 432);
```