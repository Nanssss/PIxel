/*
* I'm using deps/wrapper.h to gather all headers.
* If other libraries have to be imported, only steps 1 to 3 have to be modified.
*/

use std::env;
use std::path::PathBuf;
use std::process::Command;

fn main() {
    // Project dirs
    let manifest_dir = env::var("CARGO_MANIFEST_DIR").unwrap();
    let deps_dir = PathBuf::from(&manifest_dir).join("deps");

    // Matrix library
    let matrix_dir = deps_dir.join("rpi-rgb-led-matrix");
    let include_dir = matrix_dir.join("include");
    let lib_dir = matrix_dir.join("lib");

    // 1. Automatically compile the C++ library using its internal Makefile
    // Disable LTO (LTO_FLAGS=) so that the static library contains native machine code
    // rather than GCC LTO bytecode, which rust-lld cannot read.
    let status = Command::new("make")
        .arg("-C")
        .arg(&lib_dir)
        .arg("librgbmatrix.a")
        // remove LTO optim because it requires all code to be compiled with same compiler
        // another fix would be to use clang for compiling the C++ code
        .arg("LTO_FLAGS=") 
        .status()
        .expect("Failed to execute 'make' for rgbmatrix library");

    if !status.success() {
        panic!("The hzeller rgbmatrix C library failed to compile.");
    }

    // 2. Tell Cargo where to find the generated static library and link it
    println!("cargo:rustc-link-search=native={}", lib_dir.display());
    println!("cargo:rustc-link-lib=static=rgbmatrix"); // Link to librgbmatrix.a (lib<name>.a)
    println!("cargo:rustc-link-lib=stdc++"); // Link C++ standard library (mandatory for hzeller)

    // Tell rust-lld not to garbage-collect custom sections bounded by __start_/__stop_
    println!("cargo:rustc-link-arg=-Wl,-z,nostart-stop-gc");

    // 3. Set up rerun triggers so Cargo recompiles if source files or headers change
    let wrapper_path = deps_dir.join("wrapper.h");
    println!("cargo:rerun-if-changed={}", wrapper_path.display());
    println!("cargo:rerun-if-changed={}", include_dir.join("led-matrix-c.h").display());
    println!("cargo:rerun-if-changed={}", lib_dir.join("Makefile").display());
    println!("cargo:rerun-if-changed={}", matrix_dir.join("config.mk").display());

    // 4. Generate bindings using bindgen
    let bindings = bindgen::Builder::default()
        .header(wrapper_path.to_str().unwrap()) // wrapper.h contains all headers we want to import (only
        // one in this case)
        .clang_arg(format!("-I{}", include_dir.display()))
        .parse_callbacks(Box::new(bindgen::CargoCallbacks::new()))
        .generate()
        .expect("Unable to generate bindings");

    // 5. Write the generated bindings to the OUT_DIR directory
    let out_path = PathBuf::from(env::var("OUT_DIR").unwrap());
    bindings
        .write_to_file(out_path.join("bindings.rs"))
        .expect("Couldn't write bindings!");
}
