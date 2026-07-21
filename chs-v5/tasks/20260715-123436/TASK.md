# Add optional index variable in foreach

- STATUS: OPEN
- PRIORITY: 100

Add a optional index variable and a by reference iteration variable to foreach loops
```
// index variable
foreach i, x in [1, 2, 3] {
}
```

```
// by reference iteration variable
foreach *x in [1, 2, 3] {
}
```

```
// both
foreach i, *x in [1, 2, 3] {
}
```
