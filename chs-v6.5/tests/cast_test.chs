import "io"

type LogLevel enum {
    Debug = 10,
    Info = 20,
    Warning = 30,
}

fn print_level_as_int(level: LogLevel) {
    var raw_value = cast(int) level;
    print("LogLevel int: %d\n", #anycast[raw_value]);
}

fn get_level_from_int(val: int) -> LogLevel {
    return cast(LogLevel) val;
}

fn test_explicit_casts() {
    puts("test_explicit_casts:");
    var level = LogLevel.Info;
    print_level_as_int(level);

    var raw = 30;
    var new_level = get_level_from_int(raw);
    if new_level == LogLevel.Warning {
        puts("  Explicit cast int -> LogLevel succeeded.");
    } else {
        puts("  Explicit cast int -> LogLevel failed!");
    };
}

fn print_int_val(x: int) {
    print("Value: %d\n", #anycast[x]);
}

fn test_auto_casts() {
    puts("test_auto_casts:");

    // Auto-cast u8 to int
    var a: u8 = cast(u8) 42;
    var b: int = autocast a; // Infers target type is int from variable type
    if b == 42 {
        puts("  Auto-cast variable initialization succeeded.");
    } else {
        puts("  Auto-cast variable initialization failed!");
    };

    // Auto-cast in function call
    puts("  Calling print_int_val with autocast a:");
    print_int_val(autocast a); // Infers target type is int from function signature
}

fn main() {
    test_explicit_casts();
    puts("====================");
    test_auto_casts();
}
