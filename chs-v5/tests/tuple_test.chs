import "io"

fn get_tuple() -> (int, bool, string) {
    return 10, true, "hello";
}

fn test_var_decl() {
    print("test_var_decl:\n");
    var a, b, c = get_tuple();
    if a == 10 {
        print("  a is 10\n");
    } else {
        print("  a is NOT 10!\n");
    };
    if b {
        print("  b is true\n");
    } else {
        print("  b is NOT true!\n");
    };
    if c.len == 5 {
        print("  c len is 5\n");
    } else {
        print("  c len is NOT 5!\n");
    };
}

fn test_assign() {
    print("test_assign:\n");
    var x = 0;
    var y = false;
    x, y = 42, true;
    if x == 42 {
        print("  x is 42\n");
    } else {
        print("  x is NOT 42!\n");
    };
    if y {
        print("  y is true\n");
    } else {
        print("  y is NOT true!\n");
    };
}

fn test_grouping() {
    print("test_grouping:\n");
    var val = (2 + 3) * 4;
    if val == 20 {
        print("  (2 + 3) * 4 is 20\n");
    } else {
        print("  (2 + 3) * 4 is %d!\n", #anycast[val]);
    };
}

fn test_member_access() {
    print("test_member_access:\n");
    var t = (100, "world");
    if t.0 == 100 {
        print("  t.0 is 100\n");
    } else {
        print("  t.0 is NOT 100!\n");
    };
    if (t.1).len == 5 {
        print("  t.1 len is 5\n");
    } else {
        print("  t.1 len is NOT 5!\n");
    };
}

fn main() {
    test_var_decl();
    print("====================\n");
    test_assign();
    print("====================\n");
    test_grouping();
    print("====================\n");
    test_member_access();
}
