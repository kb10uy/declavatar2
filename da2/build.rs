fn main() {
    #[cfg(feature = "bindings")]
    generate_header();
}

#[cfg(feature = "bindings")]
fn generate_header() {
    let crate_dir = std::env::var("CARGO_MANIFEST_DIR").expect("cargo sets the manifest directory");
    println!("cargo:rerun-if-changed=src/lib.rs");
    println!("cargo:rerun-if-changed=cbindgen.toml");
    cbindgen::generate(&crate_dir)
        .expect("the header should be generated")
        .write_to_file(std::path::Path::new(&crate_dir).join("include/da2.h"));
}
