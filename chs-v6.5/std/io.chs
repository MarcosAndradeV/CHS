import string
import libc

fn chs_print(x: string)
    #foreign Runtime
    #link_name "chs_print"
    #private

fn puts(s: string) {
    libc.printf(s.data);
    chs_print("\n");
}

fn print_info(arg: Any) #private {
    var info = arg.type_info;
    if info.name == "int" {
        var val_ptr = cast(&int) arg.value;
        libc.printf("%d".data, &val_ptr);
    } else if info.name == "u8" {
        var val_ptr = cast(&u8) arg.value;
        libc.printf("%u".data, cast(int) &val_ptr);
    } else if info.name == "bool" {
        var val_ptr = cast(&bool) arg.value;
        if &val_ptr {
            libc.printf("true".data);
        } else {
            libc.printf("false".data);
        }
    } else if info.name == "float" {
        var val_ptr = cast(&float) arg.value;
        libc.printf("%lg".data, &val_ptr);
    } else if info.kind == TypeKind.String {
        var val_ptr = cast(&string) arg.value;
        var s = &val_ptr;
        foreach char in s {
            libc.printf("%c".data, cast(int) char);
        };
    } else if info.kind == TypeKind.Pointer {
        var val_ptr = cast(&&void) arg.value;
        libc.printf("%p".data, &val_ptr);
    } else if info.kind == TypeKind.Struct {
        libc.printf("{".data);
        var i = 0;
        foreach field in info.fields {
            if i > 0 {
                libc.printf(", ".data);
            };
            i += 1;
            print("%: ", #anycast[field.name]);
            print_info(Any.{
                value: arg.value + field.offset,
                type_info: field.type_info,
            });
        };
        libc.printf("}".data);
    } else if info.kind == TypeKind.Enum {
        var val_ptr = cast(&int) arg.value;
        var val = &val_ptr;
        var found = false;
        foreach variant in info.variants {
            if variant.value == val {
                print("%", #anycast[variant.name]);
                found = true;
            }
        };
        if !found {
            libc.printf("%d".data, val);
        }
    } else if info.kind == TypeKind.Array {
        libc.printf("[".data);
        var elem_size = info.element_type.size;
        var i = 0;
        for i < info.array_len {
            if i > 0 {
                libc.printf(", ".data);
            };
            print_info(Any.{
                value: arg.value + i * elem_size,
                type_info: info.element_type,
            });
            i = i + 1;
        };
        libc.printf("]".data);
    } else if info.kind == TypeKind.Slice {
        libc.printf("[".data);
        var data_ptr_ptr = cast(&&void) arg.value;
        var data_ptr = &data_ptr_ptr;
        var len_ptr = cast(&int) (arg.value + 8);
        var len = &len_ptr;
        var elem_size = info.element_type.size;

        var i = 0;
        for i < len {
            if i > 0 {
                libc.printf(", ".data);
            };
            print_info(Any.{
                value: data_ptr + i * elem_size,
                type_info: info.element_type,
            });
            i = i + 1;
        };
        libc.printf("]".data);
    } else if info.kind == TypeKind.Any {
        var any_val_ptr = cast(&&void) arg.value;
        var any_info_ptr = cast(&&TypeInfo) (arg.value + 8);
        print_info(Any.{
            value: &any_val_ptr,
            type_info: &any_info_ptr,
        });
    } else {
        libc.printf("<unknown>".data, 0);
    }
}

fn print(fmt: string, args: ...Any) {
    if args.len == 0 {
        chs_print(fmt);
        return;
    };
    var arg_idx = 0;
    foreach c in fmt {
        if cast(int) c == 37 {
            if arg_idx < args.len {
                var arg = args[arg_idx];
                arg_idx = arg_idx + 1;
                print_info(arg);
            } else {
                libc.printf("%%".data, 0);
            }
        } else {
            libc.printf("%c".data, cast(int) c);
        }
    }
}
