import "io"

struct Person {
    name: string,
    age: int,
}

enum Status {
    Active = 1,
    Inactive = 2,
}

fn test_primitive_reflection() {
    puts("test_primitive_reflection:");
    var info = #type_info(int);
    print("  kind: %d\n", #anycast[cast(int)info.kind]);
    print("  size: %d\n", #anycast[info.size]);
    print("  align: %d\n", #anycast[info.align]);
    print("  name: %s\n", #anycast[info.name]);
}

fn test_struct_reflection() {
    puts("test_struct_reflection:");
    var info = #type_info(Person);
    print("  kind: %d\n", #anycast[cast(int)info.kind]);
    print("  size: %d\n", #anycast[info.size]);
    print("  align: %d\n", #anycast[info.align]);
    print("  name: %s\n", #anycast[info.name]);
    print("  fields len: %d\n", #anycast[info.fields.len]);

    var fields = info.fields;
    if fields.len > 0 {
        print("    field 0 offset: %d\n", #anycast[fields[0].offset]);
        print("    field 0 name: %s\n", #anycast[fields[0].name]);
    };
    if fields.len > 1 {
        print("    field 1 offset: %d\n", #anycast[fields[1].offset]);
        print("    field 1 name: %s\n", #anycast[fields[1].name]);
    };
}

fn test_enum_reflection() {
    puts("test_enum_reflection:");
    var info = #type_info(Status);
    print("  kind: %d\n", #anycast[cast(int)info.kind]);
    print("  name: %s\n", #anycast[info.name]);
    print("  variants len: %d\n", #anycast[info.variants.len]);

    var variants = info.variants;
    if variants.len > 0 {
        print("    variant 0 name: %s\n", #anycast[variants[0].name.data]);
        print("    variant 0 value: %d\n", #anycast[variants[0].value]);
    };
    if variants.len > 1 {
        print("    variant 1 name: %s\n", #anycast[variants[1].name]);
        print("    variant 1 value: %d\n", #anycast[variants[1].value]);
    };
}

fn main() {
    test_primitive_reflection();
    test_struct_reflection();
    test_enum_reflection();
}
