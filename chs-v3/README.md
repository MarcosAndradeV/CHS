# CHS Compiler

> [!WARNING]
> The Compiler is not fully implemented yet.

Previous version https://github.com/MarcosAndradeV/chs-lang

## Dependencies

- [Rust](https://www.rust-lang.org/) and Cargo.
- [FASM](https://flatassembler.net/).

## Quick Start

```console
$ cargo build --release
$ ./target/release/chsc compile-run ./examples/hello_world.chs
```

## Usage

```console
Usage: chsc <command>
Commands:
   compile <file>      Compile the program
   compile-run <file>  Compile and run the program
   version             Show chsc version
   help                Show help information
Aliases:
   c                   Alias For compile
   cr                  Alias For compile-run
```
