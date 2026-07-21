import "io"
import "string"

fn test_eq() {
    puts("test_eq...");
    var s1 = "hello";
    var s2 = "hello";
    if s1 == s2 {
        puts("  s1 == s2 passed");
    } else {
        puts("  s1 == s2 failed");
    };

    var s3 = "hello world";
    var sub1 = substring(s3, 0, 5); // "hello"
    if sub1 == s1 {
        puts("  sub1 == s1 passed");
    } else {
        puts("  sub1 == s1 failed");
    };

    var s4 = "hell";
    if s1 == s4 {
        puts("  s1 == s4 failed!");
    } else {
        puts("  s1 != s4 passed");
    };
}

fn test_is_empty() {
    puts("test_is_empty...");
    var empty = "";
    var nonempty = "hello";
    if is_empty(empty) {
        puts("  empty passed");
    };
    if !is_empty(nonempty) {
        puts("  nonempty passed");
    };
}

fn test_substring() {
    puts("test_substring...");
    var s = "hello world";
    var sub = substring(s, 6, 5);
    if sub == "world" {
        puts("  substring passed");
    } else {
        puts("  substring failed");
    };
}

fn test_starts_ends_with() {
    puts("test_starts_ends_with...");
    var s = "hello world";
    if starts_with(s, "hello") {
        puts("  starts_with passed");
    } else {
        puts("  starts_with failed");
    };
    if ends_with(s, "world") {
        puts("  ends_with passed");
    } else {
        puts("  ends_with failed");
    };
}

fn test_index_of_contains() {
    puts("test_index_of_contains...");
    var s = "hello world";
    var idx = index_of(s, "world");
    if idx == 6 {
        puts("  index_of passed");
    } else {
        puts("  index_of failed");
    };
    if contains(s, "lo wo") {
        puts("  contains passed");
    } else {
        puts("  contains failed");
    };
}

fn test_trim() {
    puts("test_trim...");
    var s = " \n hello world \n ";
    var t = trim(s);
    if t == "hello world" {
        puts("  trim passed");
    } else {
        puts("  trim failed");
    };
}

fn main() {
    test_eq();
    test_is_empty();
    test_substring();
    test_starts_ends_with();
    test_index_of_contains();
    test_trim();
}
