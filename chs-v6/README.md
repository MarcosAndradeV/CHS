# CHS (chs-v6) Programming Language

**CHS** is an experimental, high-performance, strongly-typed systems programming language. It is designed to combine low-level control (manual memory management, pointer arithmetic, and zero-overhead C interoperability) with modern developer conveniences (generics, reflection, structural tuples, and operator overloading) without the overhead of a garbage collector or a complex borrow checker.

The compiler is written in Rust and targets the lightweight [QBE compiler backend](https://c9x.me/compile/), offering extremely fast compile times and compact native binaries.

---

## Key Features

* **Manual & Custom Memory Management:** Explicit memory management built around modular, custom allocators defined in the language itself (`std/mem.chs`). Supports explicit pointer arithmetic.
* **C Interoperability:** Zero-overhead bindings to external C code via `#foreign` declarations, `#link_name` overrides, and shared memory structures.
* **Monomorphized Generics:** Bracketed parameter syntax `[$T]` for types and functions, compiled by generating specialized copies during semantic analysis.
* **Compile-Time Metaprogramming:** Directives such as `#type_info(T)` for reflection, `#anycast[args]` for type-safe dynamic wrappers, and `#operator` for operator overloading.
* **Expression-Oriented Control Flow:** Simple block expressions and control flow structures (if/for/switch) that evaluate to values.

---

## Repository Structure

The project is structured as a Cargo workspace with the following modules:
* [cli/](file:///home/marcos/Projects/chs-v6/cli): Command-line interface wrapper for the compiler.
* [compiler/](file:///home/marcos/Projects/chs-v6/compiler): Core compiler orchestration and pipeline.
* [syntax/](file:///home/marcos/Projects/chs-v6/syntax): Lexical analysis, parser, and AST definition.
* [semantics/](file:///home/marcos/Projects/chs-v6/semantics): Semantic analysis, type checking, scope resolution, and monomorphization.
* [ir/](file:///home/marcos/Projects/chs-v6/ir): SSA-based Intermediate Representation (IR) generation and optimization.
* [codegen/](file:///home/marcos/Projects/chs-v6/codegen): Lowers SSA IR to QBE assembly code.
* [std/](file:///home/marcos/Projects/chs-v6/std): Standard library written in CHS (allocators, string formatting, io, vectors, etc.).
* [tests/](file:///home/marcos/Projects/chs-v6/tests): Integration test suite.
* [docs/](file:///home/marcos/Projects/chs-v6/docs): Technical specifications and designs.

---

## Requirements

To build and run CHS programs, you need:
* **QBE Backend:** The compiler outputs `.ssa` assembly, which must be compiled by [QBE](https://c9x.me/compile/).
* **C Linker/Compiler:** `gcc` or `clang` to link the final executable with the CHS runtime library.
* **Rust Toolchain:** Cargo to build the Rust compiler.
* **`proj` CLI (Optional):** Project manager and task runner.

---

## Getting Started

### 1. Build the Compiler

To build the compiler CLI using Cargo:
```bash
cargo build --release
```
Or, if you have `proj` installed:
```bash
proj run build
```
This builds the compiler binary in `target/release/compiler` (or `target/debug/compiler` without `--release`).

### 2. Write Your First CHS Program

Create a file named `hello.chs`:
```chs
import "io"

fn main() {
    var greeting = "Hello, World!";
    puts(greeting);
}
```

Run the program directly using the compiler:
```bash
cargo run -- run hello.chs
```
Or build it to a target executable:
```bash
cargo run -- build hello.chs -o my_executable
./my_executable
```

---

## Compiler CLI Usage

```text
Usage: chs <COMMAND>

Commands:
  build    Compile module and dependencies [aliases: b]
  run      Compile and run program [aliases: r]
  clear    Clear the project artifacts
  version  Print version
  help     Print this message or the help of the given subcommand(s)
```

### Subcommands and Flags
* **`build` (or `b`)**:
  - `path`: The input `.chs` source file path.
  - `-v`: Enable verbose output.
  - `-o, --output <output>`: Specify the output executable name (defaults to `out`).
* **`run` (or `r`)**:
  - `path`: Compile and run target program directly.
* **`clear`**: Removes compilation artifacts in `.build/`.

---

## Testing

Integration tests are located in [tests/](file:///home/marcos/Projects/chs-v6/tests):
* `*.chs` files are tests expected to pass.
* `*.fail` files are tests expected to fail compiler or runtime checks.

To run the complete test suite:
```bash
./runtest.sh
```
Or using `proj`:
```bash
proj run test
```
