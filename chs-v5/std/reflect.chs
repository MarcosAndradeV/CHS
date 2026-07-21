import "runtime"

struct TypeEnumVariant {
    name: string,
    value: int,
}

struct TypeField {
    name: string,
    offset: int,
    type_info: *TypeInfo,
}

enum TypeKind {
    Primitive = 1,
    Pointer   = 2,
    Array     = 3,
    Slice     = 4,
    Struct    = 5,
    Enum      = 6,
    String    = 7,
    Any       = 8,
    FnPointer = 9,
}

struct TypeInfo {
    kind: TypeKind,
    name: string,
    size: int,
    align: int,
    element_type: *TypeInfo,
    array_len: int,
    fields: []TypeField,
    variants: []TypeEnumVariant,
}
