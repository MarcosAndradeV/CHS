fn memset(p *u8, byte u8, size usize) -> *u8 {
    var i int = 0;
    while(i < size) {
        *(p + i) = byte;
        i = i + 1;
    }
    return p;
}

fn strlen(s *char) -> usize {
    var end *char = s;
    while((*end) != 0) {
        end = end + 1;
    }
    return end - s;
}
