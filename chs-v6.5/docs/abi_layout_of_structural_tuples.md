# ABI & Layout of Structural Tuples

Structural tuples in CHS are compiled and lowered according to the following layout and ABI constraints:

## 1. Type Lowering & Layout
Every unique structural tuple type (e.g. `(int, bool, string)`) is lowered as an anonymous, flat, byte-aligned struct definition in the generated QBE SSA module.
- **Name Mangling**: Tuple names are mangled using the format `:chs_tuple_<element_types>` (e.g. `:chs_tuple_int_bool_string`).
- **Memory Alignment**: Layout offsets and alignments are computed exactly like standard structs. Each element is placed at the next offset satisfying its natural alignment, and the total struct size is padded to the maximum alignment of its constituent elements.

## 2. ABI & Parameter/Return Passing
When passing or returning a tuple, the CHS compiler leverages QBE's user-defined structure type syntax (`:type_name` parameters and return signatures).
- **Small Tuples**: Small tuples that fit within the target system's integer/vector registers (determined by the target C ABI, e.g. System V AMD64 or Windows x64) are passed and returned directly in registers.
- **Large Tuples / Aggregates**: For larger tuples, QBE's backend automatically complies with the standard C ABI by allocating storage on the caller's stack frame and transparently passing a pointer to this buffer as an implicit first argument.
- **Memory Copying**: Local assignments and returns copy the tuple data via `memcpy` or direct register copies where appropriate.
