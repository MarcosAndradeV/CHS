#library Runtime {
    link_name = "chs_runtime",
    kind = "static",
}

type RawPtr &void
type byte u8

type AllocatorMode enum : byte {
	Alloc,
	Dealloc,
	Clear,
	Realloc,
}

type AllocatorError enum : byte {
	None                = 0,
	OutOfMemory         = 1,
	InvalidPointer      = 2,
	InvalidArgument     = 3,
	ModeNotImplemented  = 4,
}

type AllocatorImpl fn(
    allocator_data: RawPtr,
    mode: AllocatorMode,
    size: int,
    alignment: int,
    old_ptr: RawPtr,
    old_size: int
) -> ([]u8, AllocatorError)

type Allocator struct {
	data: RawPtr,
	impl: AllocatorImpl = default_allocator_impl,
}

type TypeEnumVariant struct {
    name: string,
    value: int,
}

type TypeField struct {
    name: string,
    offset: int,
    type_info: &TypeInfo,
}

type TypeKind enum : byte {
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

type TypeInfo struct {
    kind: TypeKind,
    name: string,
    size: int,
    align: int,
    element_type: &TypeInfo,
    array_len: int,
    fields: []TypeField,
    variants: []TypeEnumVariant,
}

type Any struct {
    value: &void,
    type_info: &TypeInfo
}

fn chs_alloc(size: int) -> &void
    #foreign Runtime
    #link_name "chs_alloc"
    #private
fn chs_realloc(ptr: &void, size: int) -> &void
    #foreign Runtime
    #link_name "chs_realloc"
    #private
fn chs_dealloc(ptr: &void)
    #foreign Runtime
    #link_name "chs_dealloc"
    #private

fn alloc(size: int, alignment: int = 8, allocator: Allocator = Allocator.{}) -> &u8 {
    var x, _ = allocator.impl(allocator.data, AllocatorMode.Alloc, size, alignment, null, 0);
    return x.data;
}

fn dealloc(ptr: RawPtr, size: int = 0, alignment: int = 8, allocator: Allocator = Allocator.{}) {
    allocator.impl(allocator.data, AllocatorMode.Dealloc, size, alignment, ptr, size);
}

fn realloc(old_ptr: &void, new_size: int, old_size: int = 0, alignment: int = 8, allocator: Allocator = Allocator.{}) -> &u8 {
    var x, _ = allocator.impl(allocator.data, AllocatorMode.Realloc, new_size, alignment, old_ptr, old_size);
    return x.data;
}

fn default_allocator_impl(
    allocator_data: RawPtr,
    mode: AllocatorMode,
    size: int,
    alignment: int,
    old_ptr: RawPtr,
    old_size: int,
    ) -> ([]u8, AllocatorError) {
    switch mode {
    	AllocatorMode.Alloc -> {
            var ptr = chs_alloc(size);
            if ptr == null {
                return (#default, AllocatorError.OutOfMemory);
            };
            var s: []u8 = #default;
            s.data = ptr;
            s.len = size;
            return (s, AllocatorError.None);
        };
	    AllocatorMode.Realloc -> {
            var new_ptr = chs_realloc(old_ptr, size);
            if new_ptr == null {
                return (#default, AllocatorError.OutOfMemory);
            };
            var s: []u8 = #default;
            s.data = new_ptr;
            s.len = size;
            return (s, AllocatorError.None);
		};
	    AllocatorMode.Dealloc -> {
            chs_dealloc(old_ptr);
            return (#default, AllocatorError.None)
		};
	    AllocatorMode.Clear -> return (#default, AllocatorError.None);
    };
    return (#default, AllocatorError.InvalidArgument)
}
