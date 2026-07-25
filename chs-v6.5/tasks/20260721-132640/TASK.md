# Fix tests/newtype_test.chs segfault

- STATUS: CLOSED
- PRIORITY: 500

To reproduce the bug

## Build

```
cargo run -q -- build tests/newtype_test.chs -o main
```

## Run

```
./main
```

## Debug

```
gdb ./main
...
(gdb) run
Starting program: /home/marcos/Projects/chs-v6/main
Downloading separate debug info for system-supplied DSO at 0x7ffff7fc3000
Downloading 6.75 M separate debug info for /lib64/libc.so.6
[Thread debugging using libthread_db enabled]
Using host libthread_db library "/lib64/libthread_db.so.1".
seconds: 42
minutes: 42

Program received signal SIGSEGV, Segmentation fault.
Downloading 27.27 K source file /usr/src/debug/glibc-2.43-7.fc44.x86_64/string/../sysdeps/x86_64/multiarch/memmove-vec-unaligned-erms.S
__memcpy_avx_unaligned_erms () at ../sysdeps/x86_64/multiarch/memmove-vec-unaligned-erms.S:364
364		movq	-8(%rsi, %rdx), %rcx
```