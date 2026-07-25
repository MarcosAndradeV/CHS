import "io"

fn test_opt(cond: int) -> int {
    // Pure expression statements with no side effects (should be eliminated by DCE)
    10 + 20;
    cond * 5;

    // 1. Dead/unused instructions in a reachable block
    var dead_calc = 50 + 100;
    var unused_var = dead_calc * 2;

    // 2. Unreachable block generation using if-else with returns
    if cond == 1 {
        return 42;
    } else {
        return 24;
    };

    // Everything below is in the merge block, which is completely unreachable!
    var unreachable_dead = 999 + 888;
    puts("This should never be printed or even compiled into the binary!");
    return unreachable_dead;
}

fn main() {
    var res = test_opt(1);
    print("test_opt(1) = %d\n", #anycast[res]);

    res = test_opt(0);
    print("test_opt(0) = %d\n", #anycast[res]);
}
