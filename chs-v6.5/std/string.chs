#operator ==(s1: string, s2: string) -> bool {
    if s1.len != s2.len {
        return false;
    };
    var i = 0;
    for i < s1.len {
        if s1.data[i] != s2.data[i] {
            return false;
        };
        i = i + 1;
    };
    return true;
}

fn is_empty(s: string) -> bool {
    return s.len == 0;
}

fn substring(s: string, start: int, len: int) -> string {
    if start < 0 || len < 0 || start + len > s.len {
        return string.{ data: null, len: 0 };
    };
    return string.{ data: s.data + start, len: len };
}

fn starts_with(s: string, prefix: string) -> bool {
    if s.len < prefix.len {
        return false;
    };
    var i = 0;
    for i < prefix.len {
        if s.data[i] != prefix.data[i] {
            return false;
        };
        i = i + 1;
    };
    return true;
}

fn ends_with(s: string, suffix: string) -> bool {
    if s.len < suffix.len {
        return false;
    };
    var offset = s.len - suffix.len;
    var i = 0;
    for i < suffix.len {
        if s.data[offset + i] != suffix.data[i] {
            return false;
        };
        i = i + 1;
    };
    return true;
}

fn index_of(s: string, substr: string) -> int {
    if substr.len == 0 {
        return 0;
    };
    if s.len < substr.len {
        return -1;
    };
    var i = 0;
    var limit = s.len - substr.len;
    for i <= limit {
        var match = true;
        var j = 0;
        for j < substr.len {
            if s.data[i + j] != substr.data[j] {
                match = false;
                break;
            };
            j = j + 1;
        };
        if match {
            return i;
        };
        i = i + 1;
    };
    return -1;
}

fn contains(s: string, substr: string) -> bool {
    return index_of(s, substr) != -1;
}

fn trim_left(s: string) -> string {
    var start = 0;
    for start < s.len {
        var c = s.data[start];
        if c != cast(u8) 32 && c != cast(u8) 9 && c != cast(u8) 10 && c != cast(u8) 13 {
            break;
        };
        start = start + 1;
    };
    return substring(s, start, s.len - start);
}

fn trim_right(s: string) -> string {
    var end = s.len;
    for end > 0 {
        var c = s.data[end - 1];
        if c != cast(u8) 32 && c != cast(u8) 9 && c != cast(u8) 10 && c != cast(u8) 13 {
            break;
        };
        end = end - 1;
    };
    return substring(s, 0, end);
}

fn trim(s: string) -> string {
    return trim_right(trim_left(s));
}
