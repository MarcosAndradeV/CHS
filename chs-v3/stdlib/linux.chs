
const STDIN  = 0;
const STDOUT = 1;
const STDERR = 2;

const SYS_READ  = 0;
const SYS_WRITE = 1;
const SYS_OPEN  = 2;
const SYS_CLOSE = 3;
const SYS_STAT  = 4;
const SYS_EXIT  = 60;

fn read(fd int, buf *char, count usize) -> usize {
    return syscall(SYS_READ, fd, buf, count);
}

fn write(fd int, buf *char, buf_len usize) -> usize {
    return syscall(SYS_WRITE, fd, buf, buf_len);
}

fn exit(code int) {
    syscall(SYS_EXIT, code);
}

fn open(path *char, flags int, mode int) -> int {
    return syscall(SYS_OPEN, path, flags, mode); // sys_open
}

fn close(fd int) -> int {
    return syscall(SYS_CLOSE, fd); // sys_close
}

fn stat(path *char, statbuf *void) -> int {
    return syscall(SYS_STAT, path, statbuf); // sys_stat
}

fn fstat(fd int, statbuf *void) -> int {
    return syscall(5, fd, statbuf); // sys_fstat
}

fn lseek(fd int, offset int, whence int) -> int {
    return syscall(8, fd, offset, whence); // sys_lseek
}

fn mmap(addr *void, length int, prot int, flags int, fd int, offset int) -> *void {
    return syscall(9, addr, length, prot, flags, fd, offset); // sys_mmap
}

fn munmap(addr *void, length int) -> int {
    return syscall(11, addr, length); // sys_munmap
}

fn brk(addr *void) -> int {
    return syscall(12, addr); // sys_brk
}

fn access(path *char, mode int) -> int {
    return syscall(21, path, mode); // sys_access
}

fn kill(pid int, sig int) -> int {
    return syscall(62, pid, sig); // sys_kill
}

fn getpid() -> int {
    return syscall(39); // sys_getpid
}

fn getppid() -> int {
    return syscall(110); // sys_getppid
}

fn clone(flags int, child_stack *void, ptid *int, ctid *int, newtls *void) -> int {
    return syscall(56, flags, child_stack, ptid, ctid, newtls); // sys_clone
}
