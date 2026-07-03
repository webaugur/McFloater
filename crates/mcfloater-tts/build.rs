use std::env;
use std::path::PathBuf;

fn main() {
    let manifest_dir = PathBuf::from(env::var("CARGO_MANIFEST_DIR").unwrap());
    let sam_dir = manifest_dir
        .join("..")
        .join("..")
        .join("ffi")
        .join("sam");

    println!("cargo:rerun-if-changed={}", sam_dir.display());

    cc::Build::new()
        .file(sam_dir.join("sam.c"))
        .file(sam_dir.join("render.c"))
        .file(sam_dir.join("reciter.c"))
        .file(sam_dir.join("debug.c"))
        .file(sam_dir.join("lib.c"))
        .include(&sam_dir)
        .warnings(false)
        .compile("sam");

    // ReciterTables / RenderTables / SamTabs are header-only tables included by the C sources.
    for header in ["ReciterTabs.h", "RenderTabs.h", "SamTabs.h", "sam.h", "render.h", "reciter.h", "debug.h", "lib.h"] {
        println!("cargo:rerun-if-changed={}", sam_dir.join(header).display());
    }
}
