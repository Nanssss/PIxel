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
    let wrapper_path = deps_dir.join("wrapper.h");

    // `cross` sets CROSS_TOOLCHAIN_PREFIX (e.g. "arm-unknown-linux-gnueabihf-").
    // make only knows CC/CXX, so we bridge them manually: without this, make
    // would use the host x86 gcc/g++ that also live in the container.
    let prefix = env::var("CROSS_TOOLCHAIN_PREFIX").unwrap_or_default();
    let cc = env::var("CC").unwrap_or_else(|_| format!("{prefix}gcc"));
    let cxx = env::var("CXX").unwrap_or_else(|_| format!("{prefix}g++"));

    // Conditionally apply ARMv6 flags only when targeting Raspberry Pi Zero / armv6
    let target = env::var("TARGET").unwrap_or_default();
    let is_armv6 = target.contains("arm-unknown-linux-gnueabihf") || target.contains("armv6");

    // Compile the C++ library. LTO is disabled so the static lib contains native
    // machine code rather than GCC LTO bytecode (which rust-lld cannot read).
    let status = Command::new("make")
        .arg("-C")
        .arg(&matrix_dir)
        .arg("lib/librgbmatrix.a")
        .arg("LTO_FLAGS=")
        .arg(format!("CC={cc}"))
        .arg(format!("CXX={cxx}"))
        .args(is_armv6.then_some("CPU_ARCH_FLAGS=-march=armv6"))
        .status()
        .expect("Failed to run make for rgbmatrix");
    assert!(status.success(), "rgbmatrix make failed");

    // Tell Cargo where to find the generated static library and link it
    println!("cargo:rustc-link-search=native={}", lib_dir.display());
    println!("cargo:rustc-link-lib=static=rgbmatrix"); // Link to librgbmatrix.a (lib<name>.a)
    println!("cargo:rustc-link-lib=stdc++"); // Link C++ standard library (mandatory for hzeller)

    // Tell rust-lld not to garbage-collect custom sections bounded by __start_/__stop_
    println!("cargo:rustc-link-arg=-Wl,-z,nostart-stop-gc");

    // Set up rerun triggers so Cargo recompiles if source files or headers change
    println!("cargo:rerun-if-changed={}", wrapper_path.display());
    println!("cargo:rerun-if-changed={}", include_dir.join("led-matrix-c.h").display());
    println!("cargo:rerun-if-changed={}", lib_dir.join("Makefile").display());
    println!("cargo:rerun-if-changed={}", matrix_dir.join("config.mk").display());

    // Generate bindings using bindgen
    // bindgen reads TARGET and BINDGEN_EXTRA_CLANG_ARGS_<target> automatically
    // via CargoCallbacks, so no manual cross-compilation configuration needed.
    let bindings = bindgen::Builder::default()
        .header(wrapper_path.to_str().unwrap())
        .clang_arg(format!("-I{}", include_dir.display()))
        .parse_callbacks(Box::new(bindgen::CargoCallbacks::new()))
        .generate()
        .expect("Unable to generate bindings");

    // Write the generated bindings to the OUT_DIR directory
    let out_path = PathBuf::from(env::var("OUT_DIR").unwrap());
    bindings
        .write_to_file(out_path.join("bindings.rs"))
        .expect("Couldn't write bindings!");
}
