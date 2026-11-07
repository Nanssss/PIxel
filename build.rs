use std::env;
use std::path::PathBuf;
use std::process::Command;

fn main() {
    let manifest_dir = env::var("CARGO_MANIFEST_DIR").unwrap();
    let matrix_dir = PathBuf::from(&manifest_dir).join("extern/rpi-rgb-led-matrix");
    let include_dir = matrix_dir.join("include");
    let lib_dir = matrix_dir.join("lib");

    // 1. Automatically compile the C++ library using its internal Makefile
    let status = Command::new("make")
        .arg("-C")
        .arg(&lib_dir)
        .status()
        .expect("Failed to execute 'make' for rgbmatrix library");

    if !status.success() {
        panic!("The hzeller rgbmatrix C library failed to compile.");
    }

    // 2. Tell Cargo where to find the generated static library and link it
    println!("cargo:rustc-link-search=native={}", lib_dir.display());
    println!("cargo:rustc-link-lib=static=rgbmatrix");
    println!("cargo:rustc-link-lib=stdc++"); // Link C++ standard library (mandatory for hzeller)

    // 3. Set up rerun triggers so Cargo recompiles if source files or headers change
    println!("cargo:rerun-if-changed={}", include_dir.join("led-matrix-c.h").display());
    println!("cargo:rerun-if-changed={}", lib_dir.join("Makefile").display());

    // 4. Generate bindings using bindgen
    let bindings = bindgen::Builder::default()
        .header(include_dir.join("led-matrix-c.h").to_str().unwrap())
        .parse_callbacks(Box::new(bindgen::CargoCallbacks::new()))
        .generate()
        .expect("Unable to generate bindings");

    // 5. Write the generated bindings to the OUT_DIR directory
    let out_path = PathBuf::from(env::var("OUT_DIR").unwrap());
    bindings
        .write_to_file(out_path.join("bindings.rs"))
        .expect("Couldn't write bindings!");
}
