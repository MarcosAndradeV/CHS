#library Runtime {
    link_name = "chs_runtime",
    kind = "static",
}

fn chs__oob_check(message: string, len: int, idx: int) -> void #foreign Runtime #link_name "chs__oob_check"
