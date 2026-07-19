use std::env;
use std::path::PathBuf;

fn main() {
    let sam_dir = PathBuf::from(env::var("CARGO_MANIFEST_DIR").unwrap()).join("../../ffi/sam");

    println!("cargo:rerun-if-changed={}", sam_dir.display());

    // Vendored C64 SAM is pre-C99 (implicit decls). Don't fail on -Werror=implicit.
    cc::Build::new()
        .files([
            sam_dir.join("reciter.c"),
            sam_dir.join("sam.c"),
            sam_dir.join("render.c"),
            sam_dir.join("lib.c"),
            sam_dir.join("debug.c"),
        ])
        .include(&sam_dir)
        .std("gnu89")
        .warnings(false)
        .flag_if_supported("-Wno-implicit-function-declaration")
        .flag_if_supported("-Wno-int-conversion")
        .compile("sam");

    let bindings = bindgen::Builder::default()
        .header(sam_dir.join("lib.h").to_string_lossy())
        .parse_callbacks(Box::new(bindgen::CargoCallbacks))
        .generate()
        .expect("failed to generate SAM bindings");

    let out_dir = PathBuf::from(env::var("OUT_DIR").unwrap());
    bindings
        .write_to_file(out_dir.join("bindings.rs"))
        .expect("failed to write SAM bindings");
}