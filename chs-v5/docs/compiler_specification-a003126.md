# CHS Compiler Project: Technical Architecture & Language Specification

## 1. Executive Summary & Design Philosophy

### Target Domain
**CHS** is a high-performance, strongly-typed systems programming language designed to combine low-level control with modern developer conveniences (such as generics, reflection, structural tuples, and operator overloading) without the overhead of a garbage collector or complex borrow checking. 

Key objectives include:
* **Manual & Custom Memory Management:** Explicit memory management built around modular allocators (defined in the language itself via `std/mem.chs`). Pointers can be cast and incremented using explicit pointer arithmetic.
* **C Interoperability:** Zero-overhead bindings to external C code via foreign declarations, link name overrides, and shared type structures (e.g., matching the memory layout of C strings and arrays).
* **Compiling for Simplicity:** A clear compilation pipeline that targets lightweight assembly backends, offering extremely fast compile times and compact binaries.

### Syntax Paradigms
* **Imperative with Expression-Oriented Extensions:** Commands, blocks, and statements dictate execution flow. However, control structures like variable declarations and function calls treat blocks and operations as expressions.
* **Strong Static Typing with Local Inference:** Variables are strongly typed. Explicit casting (`cast(T) expr`) and automatic casting (`autocast expr`) allow clear layout transitions, while the type checker infers types for local variables and generic parameters.
* **Monomorphized Generics:** Generics (parameterized types and functions) are declared using brackets `[$T]`. Generic code is type-checked and compiled by generating specialized copies (monomorphization) for each unique set of type arguments during semantic analysis.
* **Metaprogramming & Directives:** Clean compile-time metadata operations (e.g., `#type_info(T)` for reflection, `#anycast[args]` or `#anycast expr` for dynamic type wrappers, and `#operator` for operator overloading) are supported as first-class language constructs.

### Execution Model
The compiler pipeline follows a strict multi-pass design:
1. **Lexical Analysis & Parsing:** Source files (`.chs`) are tokenized and parsed into a module-level AST using a parser written in Rust, leveraging `lex-just-parse`.
2. **Semantic Analysis & Typecheck:** The type checker merges ASTs, performs scope resolution, enforces type safety, resolves overloaded functions and operators, and monomorphizes generic instantiations.
3. **Intermediate Representation (IR) Translation:** The typed AST is lowered into a linear control-flow graph of basic blocks containing Static Single Assignment (SSA) instructions.
4. **IR Optimization:** The IR is optimized in-place via passes (Constant Folding, Unreachable Block Elimination, Dead Instruction Elimination).
5. **Code Generation:** The optimized SSA IR is lowered into QBE SSA assembly code (`.ssa`).
6. **Backend Compilation & Linking:** The `qbe` compiler backend compiles the `.ssa` files to target-specific assembly (`.s`), which `gcc` then links with the runtime static library (`libchs_runtime.a`) to output the final binary.

---

## 2. Grammar & Syntax Specification (EBNF)

### Lexical Grammar
```ebnf
Letter             = "A" | ... | "Z" | "a" | ... | "z" | "_" ;
Digit              = "0" | ... | "9" ;
HexDigit           = Digit | "A" | "B" | "C" | "D" | "E" | "F" | "a" | "b" | "c" | "d" | "e" | "f" ;

Identifier         = Letter , { Letter | Digit } ;
IntegerLiteral     = Digit , { Digit } 
                   | "0x" , HexDigit , { HexDigit } ;
FloatLiteral       = Digit , { Digit } , "." , Digit , { Digit } ;
StringLiteral      = '"' , { any_character_except_quote | escape_sequence } , '"' ;
```

### Syntactic Grammar
```ebnf
File               = { FileItem } ;
FileItem           = FunctionDecl 
                   | ImportDecl 
                   | Directive 
                   | StructDecl 
                   | EnumDecl 
                   | TypeDecl ;

ImportDecl         = "import" , StringLiteral ;
TypeDecl           = "type" , Identifier , [ "#distinct" ] , Type ;

StructDecl         = "struct" , Identifier , [ GenericParams ] , "{" , { Field , "," } , "}" ;
EnumDecl           = "enum" , Identifier , [ GenericParams ] , [ Type ] , "{" , { EnumVariant , "," } , "}" ;
EnumVariant        = Identifier 
                   | Identifier , "=" , IntegerLiteral 
                   | Identifier , "(" , Type , { "," , Type } , ")" 
                   | Identifier , "{" , Field , { "," , Field } , "}" ;

FunctionDecl       = "fn" , Identifier , [ GenericParams ] , Parameters , [ "->" , Type ] , { FunctionDirective } , [ BlockStmt ] ;
GenericParams      = "[" , Identifier , { "," , Identifier } , "]" ;
Parameters         = "(" , [ ParameterList ] , ")" ;
ParameterList      = Parameter , { "," , Parameter } , [ "," , "..." ] ;
Parameter          = Identifier , ":" , Type , [ "=" , Expression ] ;
Field              = Identifier , ":" , Type , [ "=" , Expression ] ;

FunctionDirective  = "#foreign" , Identifier
                   | "#link_name" , StringLiteral
                   | "#private" ;

(* Type Grammar *)
Type               = ScalarType
                   | PointerType
                   | ArrayType
                   | SliceType
                   | GenericInstType
                   | TupleType
                   | FnPointerType ;

ScalarType         = Identifier ;
PointerType        = "*" , Type ;
ArrayType          = "[" , IntegerLiteral , "]" , Type ;
SliceType          = "[" , "]" , Type ;
GenericInstType    = Identifier , "$" , "[" , Type , { "," , Type } , "]" ;
TupleType          = "(" , Type , { "," , Type } , ")" ;
FnPointerType      = "fn" , Parameters , [ "->" , Type ] ;

(* Statements *)
Stmt               = ExprStmt
                   | BlockStmt
                   | VarDeclStmt
                   | ReturnStmt
                   | ForStmt
                   | ForEachStmt
                   | IfStmt
                   | BreakStmt
                   | ContinueStmt
                   | DeferStmt
                   | SwitchStmt ;

ExprStmt           = Expression , ";" ;
BlockStmt          = "{" , { Stmt } , "}" ;
VarDeclStmt        = "var" , Identifier , { "," , Identifier } , [ ":" , Type ] , "=" , Expression , ";" ;
ReturnStmt         = "return" , [ ExpressionList ] , ";" ;
ForStmt            = "for" , [ Expression ] , BlockStmt ;
ForEachStmt        = "foreach" , Identifier , "in" , Expression , BlockStmt ;
IfStmt             = "if" , Expression , BlockStmt , [ "else" , BlockStmt ] ;
BreakStmt          = "break" , ";" ;
ContinueStmt       = "continue" , ";" ;
DeferStmt          = "defer" , Stmt ;
SwitchStmt         = "switch" , Expression , "{" , { SwitchBranch } , [ "_" , "->" , Stmt ] , "}" ;
SwitchBranch       = Expression , "->" , Stmt ;

(* Expressions *)
ExpressionList     = Expression , { "," , Expression } ;
Expression         = BinaryExpr | AssignExpr | UnaryExpr | PrimaryExpr ;
BinaryExpr         = Expression , BinaryOp , Expression ;
AssignExpr         = Expression , AssignOp , Expression ;
UnaryExpr          = UnaryOp , Expression ;

PrimaryExpr        = Identifier
                   | IntegerLiteral
                   | FloatLiteral
                   | StringLiteral
                   | "true" | "false" | "null"
                   | StructLiteral
                   | ArrayLiteral
                   | TupleLiteral
                   | CallExpr
                   | MemberExpr
                   | IndexExpr
                   | CastExpr
                   | AutoCastExpr
                   | AnyCastExpr
                   | TypeInfoExpr
                   | "(" Expression ")" ;

StructLiteral      = [ Identifier | GenericInstType ] , "." , "{" , [ FieldInitList ] , "}" ;
FieldInitList      = FieldInit , { "," , FieldInit } ;
FieldInit          = Identifier , ":" , Expression ;

ArrayLiteral       = [ Type , "." ] , "[" , [ ExpressionList ] , "]" ;
TupleLiteral       = "(" , Expression , { "," , Expression } , ")" ;

CallExpr           = Expression , "(" , [ ExpressionList ] , ")" ;
MemberExpr         = Expression , "." , ( Identifier | IntegerLiteral ) ;
IndexExpr          = Expression , "[" , Expression , "]" ;

CastExpr           = "cast" , "(" , Type , ")" , Expression ;
AutoCastExpr       = "autocast" , Expression ;
AnyCastExpr        = "#anycast" , ( Expression | "[" , ExpressionList , "]" ) ;
TypeInfoExpr       = "#type_info" , "(" , Type , ")" ;

(* Operators *)
BinaryOp           = "+" | "-" | "*" | "/" | "%" | "==" | "!=" | "<" | "<=" | ">" | ">=" | "&&" | "||" | "&" | "|" | "^" ;
AssignOp           = "=" | "+=" | "-=" | "*=" | "/=" ;
UnaryOp            = "-" | "!" | "*" | "&" ;
```

### "Hello World" Snippet Mapping
```chs
import "io"

fn main() {
    print("Hello, World!\n");
}
```

#### EBNF Syntactic Rule Mapping:
* **Line 1 (`import "io"`):**
  Matches `ImportDecl = "import" , StringLiteral ;` where the `StringLiteral` is `"io"`. Lowered to a `FileItem::Import` AST node.
* **Line 3 (`fn main() { ... }`):**
  Matches `FunctionDecl = "fn" , Identifier , [ GenericParams ] , Parameters , [ "->" , Type ] , { FunctionDirective } , [ BlockStmt ] ;`
  * `Identifier` $\to$ `main`
  * `GenericParams` $\to$ empty (omitted)
  * `Parameters` $\to$ `()` (matches `Parameters = "(" , [ ParameterList ] , ")" ;` where list is empty)
  * `Type` (return) $\to$ empty (implicitly returns `void`)
  * `FunctionDirective` $\to$ empty (omitted)
  * `BlockStmt` $\to$ `{ ... }` (matches lines 3–5)
* **Line 4 (`print("Hello, World!\n");`):**
  Matches `ExprStmt = Expression , ";" ;` inside the function's block.
  * `Expression` $\to$ `CallExpr`
  * `CallExpr` matches `Expression , "(" , [ ExpressionList ] , ")" ;` where:
    * Callee `Expression` $\to$ `Identifier` `"print"`
    * `ExpressionList` $\to$ single `PrimaryExpr` $\to$ `StringLiteral` `"Hello, World!\n"`

---

## 3. Lexical Analysis (Scanner) & Parsing (AST)

### Scanner Strategy
The lexical analyzer is implemented in Rust using the `lex-just-parse` tokenization engine.
* **Whitespace & Indentation:** Whitespaces, tabs, carriage returns, and newlines are discarded as trivia, serving only as token delimiters.
* **Comments:** Single-line comments starting with `//` are skipped until the end of the line. Block comments are not currently supported.
* **Lexical Identifiers & Keywords:** A prefix match is performed on source bytes. If an identifier matches one of the registered keywords (e.g., `fn`, `var`, `struct`, `enum`, `cast`, `autocast`, `null`, `import`, `type`, `switch`, `defer`), it is tokenized as a `TokenKind::Keyword`. Directives starting with `#` (e.g., `#anycast`, `#type_info`, `#distinct`, `#operator`, `#foreign`, `#link_name`, `#private`) are tokenized as `TokenKind::Directive`.
* **Error Recovery:** If an invalid byte sequence is encountered (e.g., unmatched symbols), the scanner emits a lex error but attempts to continue tokenization by skipping the invalid byte, allowing diagnostic reports for downstream passes.

### Parser Strategy
The parser uses a hybrid approach:
1. **Recursive Descent:** Structural syntax elements (declarations, imports, statements, types, struct/enum layouts) are parsed using recursive descent parser combinators in `compiler/src/syntax/mod.rs`. This handles blocks, parameter loops, and keywords with simple lookahead (1 token).
2. **Pratt Parsing (Top-Down Operator Precedence):** Mathematical and logical expressions are parsed using a Pratt parser in `parse_expr_with_precedence`. The tokenizer translates operator tokens into precedence values (from `Precedence::Lowest` to `Precedence::Index`). This avoids infinite recursion in left-associative expressions and correctly resolves operator associativity.

### AST Schema
The parsed structures are organized into the following node layout:

```
FileAst
 └── items: Vec<FileItem>
      ├── Import(ImportDecl)
      │    └── path: Token [StringLiteral]
      ├── TypeDecl(TypeDecl)
      │    ├── name: Token
      │    ├── is_distinct: bool
      │    └── base_type: Type
      ├── Struct(StructDecl)
      │    ├── name: Token
      │    ├── generic_params: Option<Vec<Token>>
      │    ├── directives: Vec<StructDirective>
      │    └── fields: Vec<VarTypeValue>
      ├── Enum(EnumDecl)
      │    ├── name: Token
      │    ├── generic_params: Option<Vec<Token>>
      │    ├── inner_type: Option<Type>
      │    └── variants: Vec<EnumVariant>
      └── FunctionDecl(FunctionDecl)
           ├── signature: FunctionSignature
           │    ├── name: Token
           │    ├── parameters: Vec<VarTypeValue>
           │    ├── return_type: Option<Type>
           │    └── va_args: bool
           ├── generic_params: Option<Vec<Token>>
           ├── directives: Vec<FunctionDirective>
           └── body: Option<BlockStmt>
                └── stmts: Vec<Stmt>
                     ├── ExprStmt(Expr)
                     ├── Call(CallExpr)
                     ├── VarDecl(VarDeclStmt)
                     │    ├── names: Vec<Token>
                     │    ├── var_type: Option<Type>
                     │    └── expr: Expr
                     ├── Return(Loc, Option<Expr>)
                     ├── ForStmt(ForStmt)
                     │    ├── ForLoop(BlockStmt)
                     │    └── ForCond { cond: Expr, body: BlockStmt }
                     ├── ForEach(ForEachStmt)
                     │    ├── var_name: Token
                     │    ├── iter_expr: Expr
                     │    └── body: BlockStmt
                     ├── IfStmt(IfStmt)
                     │    ├── If { cond: Expr, true_body: BlockStmt }
                     │    └── IfElse { cond: Expr, true_body, false_body }
                     ├── Defer(Loc, Box<Stmt>)
                     └── Switch(SwitchStmt)
                          ├── cond: Expr
                          ├── branches: Vec<SwitchBranch>
                          └── default: Option<Box<Stmt>>
```

---

## 4. Semantic Analysis & Type System

### Type Checking Strategy
The type system is statically resolved during compile time.
* **Centralized Registry:** The `TypeDatabase` acts as the single source of truth for all mapped types. Every unique type is assigned a copyable `TypeID` handle.
* **Type Invariance & Coercions:** Types must match exactly during assignments, arguments, and return checks. However, implicit coercion is allowed for fixed-size arrays converting to slice types (e.g. converting `[N]T` to `[]T`).
* **Distinct Newtypes:** Defining `type Seconds #distinct int` registers a new type. A distinct type retains the underlying size and memory offset behaviors of its base type, but requires explicit casting (`cast(Seconds)`) for values and assignments. Member accesses on distinct struct types (e.g., `SecondsPoint #distinct Point`) are transparently forwarded to the fields of the base struct.
* **Tuple Types:** Tuples have structural typing. A tuple type `(int, bool)` is unified structurally based on its element sequence.
* **Generics & Monomorphization:** When a parameterized declaration (like `struct Vec[$T]` or `fn append[$T]`) is parsed, it is treated as a template. When referenced with type arguments (e.g., `Vec$[int]`), the type checker:
  1. Computes a unique mangled name: `BaseName_Arg1Type_Arg2Type...` (using `mangle_instantiation_name`).
  2. Duplicates the template AST nodes, replacing generic parameters with concrete type arguments.
  3. Recursively typechecks the concrete instantiation and registers it as a normal, non-generic declaration.

### Symbol Table Architecture
The symbol table maps identifiers to types across multiple scopes:
* **Lexical Scope Resolution:** Scopes are nested hierarchically (Global $\to$ Function $\to$ Local block). Resolution traverses upward from the current scope to the parent until it finds the symbol.
* **Shadowing:** Variable shadowing is permitted within nested blocks. Declaring `var x = ...` in an inner block shadows any `x` declared in outer blocks.
* **Overloading Resolution:** Both functions and operator implementations (declared using `#operator`) support overloading. When resolving a function or operator call:
  1. The compiler gathers all overloading declarations matching the target name/operator symbol.
  2. It evaluates parameter type patterns against the caller's arguments.
  3. If exactly one overload matches, that overload is selected. If multiple overloads match, it emits an `AmbiguousFunctionCall` error. If none match, it emits a `NoOverloadFound` error.

### Semantic Passes
1. **Scope and Symbol Declaration Pass:** Iterates through top-level structs, enums, types, and function signatures. Enters them into the symbol table to resolve forward declarations.
2. **Type Checking & Expression Inference Pass:** Infers and verifies expression types using `infer_expr`, checking constraints and inserting implicit coercions.
3. **Monomorphization Pass:** Triggers instantiation of generic functions and structures, verifying them as concrete nodes.
4. **Control Flow & Semantic Invariant Checks:**
   * **Constant Folding:** Simplifies arithmetic on constants.
   * **Dead Code Analysis:** Identifies unreachable statements following returns, breaks, or continues.
   * **Defer Resolution:** Validates that statements under `defer` are evaluated at block boundaries.

---

## 5. Intermediate Representation (IR) & Optimization

### IR Structure
The Intermediate Representation (IR) is designed to match QBE SSA invariants.
* **Modules:** A translation module consists of global structures, constants, and a map of functions.
* **Functions:** Functions contain basic blocks (`BasicBlock`) starting with an entry block identifier.
* **Basic Blocks:** Linear sequences of instructions ending in a terminator. Control flow forms a directed graph.
* **Operands:** Instructions accept operands (`Operand`):
  * `Null`: represents null pointers.
  * `Reg(InstId)`: represents target register identifiers.
  * `Int(u64)`, `Bool(bool)`, `Float(f64)`: literal constants.
  * `String(Rc<str>)`: read-only global string literals.
  * `Param(u32)`: references to function parameter registers.
  * `Global(Rc<str>)`: references to global variables or functions.

### IR Instructions Layout
The precise structure of CHS IR instructions corresponds to the Rust definitions in `compiler/src/ir/inst.rs`:

```rust
pub enum Instruction {
    // Arithmetic & Logic
    Add(Operand, Operand),
    Sub(Operand, Operand),
    Mul(Operand, Operand),
    Div(Operand, Operand),
    Mod(Operand, Operand),

    // Relational
    Eq(Type, Operand, Operand),
    NotEq(Type, Operand, Operand),
    Lt(Type, Operand, Operand),
    LtEq(Type, Operand, Operand),
    Gt(Type, Operand, Operand),
    GtEq(Type, Operand, Operand),

    // Logical & Bitwise
    And(Operand, Operand),
    Or(Operand, Operand),
    BitAnd(Operand, Operand),
    BitOr(Operand, Operand),
    BitXor(Operand, Operand),

    // Unary
    Neg(Operand),
    Not(Operand),
    Cast(Operand),

    // Memory
    Alloca(Type),
    Load(Operand),
    Store(Type, Operand, Operand),
    Index(Operand, Operand),

    // Calls
    Call(Operand, Box<[Operand]>),

    // Control Flow (Terminators)
    Br(BlockId),
    CondBr(Operand, BlockId, BlockId),
    Return(Option<Operand>),

    // Pointer Projections
    GetMemberPtr(Operand, u32),
    GetIndexPtr(Operand, Operand),
}
```

### Key Optimization Passes
The optimized passes are applied sequentially to function blocks in `compiler/src/ir/opt.rs`:

#### 1. Constant Folding
Traverses instruction sequences to compute outcomes for expressions whose operands are constants.
* *Example:* `Add(Int(10), Int(20))` is replaced by `Int(30)`.
* Computes relational and arithmetic operations, propagating static values and reducing execution overhead.

#### 2. Unreachable Block Elimination
Builds a reachability graph starting from the entry block.
* Traverses jumps (`Br`) and branch instructions (`CondBr`).
* Identifies blocks not reachable from the entry block and deletes them from the function, updating block ID mappings.

#### 3. Dead Instruction Elimination (DCE)
Identifies instructions whose results are never read.
* Employs a liveness worklist. Instructions that write to registers are considered "dead" unless their registers are used as operands in other live instructions.
* Instructions with side effects (like `Store`, `Call`, and terminators) are considered intrinsically alive. All other unused instructions are pruned.

---

## 6. Code Generation & Target Runtime

### Codegen Strategy
The code generator (`codegen/qbe.rs`) lowers CHS IR into QBE SSA code.
* **SSA Registries:** Maps CHS IR registers directly to QBE temporary registers (e.g. `%v1`, `%v2`).
* **Lowering Types:** Types are translated to QBE types:
  * Integers and bools $\to$ `w` (word, 32-bit) or `l` (long, 64-bit).
  * Floats $\to$ `d` (double, 64-bit) or `s` (single-float, 32-bit).
  * Pointers $\to$ `l` (64-bit pointers).
* **Control Flow lowering:** Basic blocks are written as QBE labels (e.g., `@block0`). Terminators are lowered directly to QBE jumps (`jmp @target` or `jnz %cond, @true_target, @false_target`).
* **Compilation Command Execution:** The generated `.ssa` file is compiled to assembly via:
  ```bash
  qbe -o .build/target.s .build/target.ssa
  ```
  The assembly is then compiled and linked to an executable using:
  ```bash
  gcc .build/target.s -o .build/target -Lstd/runtime -lchs_runtime
  ```

### Memory Management & Runtime Layout
* **Stack vs Heap:**
  * **Stack Allocation:** Done via the `Alloca` instruction, which translates to stack-allocated variables inside function frames.
  * **Heap Allocation:** Heap allocation is performed using wrapping allocators linked to the runtime. The standard library provides the `Allocator` interface (`std/mem.chs`):
    ```chs
    struct Allocator {
        alloc: AllocFn = alloc,
        realloc: ReallocFn = realloc,
        dealloc: DeallocFn = dealloc,
    }
    ```
* **String and Slice layouts:**
  * **Slices (`[]T`):** Lowered to a 16-byte structure containing:
    1. `data`: a pointer to the element array (`*void`, 8 bytes).
    2. `len`: an integer representing element count (`int`, 8 bytes, matching the target pointer size).
  * **Strings (`string`):** Lowered to a 16-byte structure containing:
    1. `data`: a pointer to the character bytes (`*u8`, 8 bytes).
    2. `len`: string byte length (`int`, 8 bytes).
* **C Runtime Integration (`chs_runtime.h`):**
  ```c
  typedef struct {
    char *data;
    int len;
  } chs_string_t;

  void chs_print(chs_string_t m);
  void *chs_alloc(int size);
  void *chs_realloc(void *ptr, int size);
  void chs_dealloc(void *ptr);
  ```

### Error Reporting & Diagnostic Harness
The compiler includes a robust diagnostic harness in `compiler/src/diag.rs`:
* **Location representation (`Loc`):** Tracks character range offset mapping back to `file`, `line`, and `column` numbers.
* **Diagnostic schema:**
  ```rust
  pub struct Diagnostic {
      pub loc: Loc,
      pub message: String,
  }
  ```
* **Reporter Output Format:** Emits warnings and errors to standard output, highlighting file location:
  ```
  Error at tests/generics_test.chs:24:12: Type mismatch: expected "int", found "string"
  ```
* The type checker collects all semantic errors (defined in `compiler/src/semantics/errors.rs`) into the `DiagnosticReporter`. If errors exist, compiling aborts before intermediate translation.

---

## 7. Verification Plan

The compiler's correctness will be verified through a multi-tiered test suite targeting each stage of the compilation pipeline.

### Test Matrix

| Test Suite / Area | Verification Objective | Source File / Test Target | Expected Outcome |
| :--- | :--- | :--- | :--- |
| **Lexer & Parser Accuracy** | Verify parsing of complex tokens, nested expressions, operators, and grouping brackets. | `tests/tuple_test.chs`, `tests/array_test.chs` | Successful parse tree and generation of AST. |
| **Precedence Parsing** | Validate correct operator binding according to the Pratt parser precedence rules. | `tests/tuple_test.chs` (`test_grouping`) | Evaluates `(2 + 3) * 4` as `20`, not `14`. |
| **Casting Validation** | Check explicit casts (`cast`), implicit conversions (array $\to$ slice), and type conversions. | `tests/cast_test.chs` | Compiles without errors; values match expected widths. |
| **Distinct Types** | Verify `#distinct` types block direct assignment while preserving underlying field layouts. | `tests/newtype_test.chs` | Rejects direct `Seconds = Minutes` assignment but resolves `SecondsPoint.x`. |
| **Function/Operator Overloads** | Verify overloaded resolution. Ensure ambiguous calls are flagged and valid overloads link. | `tests/overload_operators.chs` | Resolves custom `==` string operator; links successfully. |
| **Generics Monomorphization** | Validate structure and function instantiation across parameterized types. | `tests/generics_test.chs` | Instantiates `Pair_int` and `identity_int`, generating type-correct code. |
| **Control Flow Invariants** | Check `defer` statement execution in reverse scope order, and test `switch` branches. | `tests/defer_test.chs`, `tests/switch_test.chs` | Executes defer actions upon block exit. Switches evaluate correctly. |
| **Pointer Arithmetic** | Ensure manual pointer offsets and raw memory updates execute without segfaults. | `tests/pointer_arithmetic_test.chs` | Directly manipulates heap buffers and index addresses. |
| **IR Optimizations** | Verify IR optimization passes rewrite code blocks and prune dead instructions. | `tests/optimization_test.chs` | Output binary bypasses dead blocks; folded constants are hardcoded. |
| **End-to-End Execution** | Compile, link, and run full files using the runtime library; verify output. | `runtest.sh` | 26/26 tests compile to QBE, link with GCC, and run to completion. |

### Running the Verification Suite
To execute the complete test suite:
1. Ensure the QBE compiler backend and GCC are installed on your system.
2. Run the test script in the workspace root:
   ```bash
   ./runtest.sh
   ```
This script compiles the CHS compiler, compiles every test file in `tests/` into executable form, and executes them to assert target functionality.
