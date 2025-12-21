const TALLOC_BUF_CAP = 8000000;
var talloc_buf[TALLOC_BUF_CAP]u8;
var talloc_buf_len usize = 0;

fn talloc_restore(save_point usize) {
    talloc_buf_len = save_point;
}
fn talloc_save() -> usize {
    return talloc_buf_len;
}
fn talloc_reset() {
    talloc_buf_len = 0;
}
fn talloc(size usize) -> *void {
    talloc_buf_len = talloc_buf_len + size;
    if(talloc_buf_len >= TALLOC_BUF_CAP) {
        talloc_buf_len = 0;
        return talloc_buf;
    }
    return talloc_buf + talloc_buf_len;
}
