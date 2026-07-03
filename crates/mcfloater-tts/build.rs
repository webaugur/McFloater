fn main() {
    cc::Build::new()
        .file("../../ffi/sam/debug.c")
        .file("../../ffi/sam/lib.c")
        .file("../../ffi/sam/main.c")
        .file("../../ffi/sam/reciter.c")
        .file("../../ffi/sam/render.c")
        .file("../../ffi/sam/sam.c")
        .include("../../ffi/sam")
        .compile("sam");
}
