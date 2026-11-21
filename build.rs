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

    // `cross` sets CROSS_TOOLCHAIN_PREFIX (e.g. "arm-unknown-linux-gnueabihf-"), but make
    // only knows CC/CXX: without them, it would use the host x86 gcc/g++ of the container
    let prefix = env::var("CROSS_TOOLCHAIN_PREFIX").unwrap_or_default();
    let cc = env::var("CC").unwrap_or_else(|_| format!("{prefix}gcc"));
    let cxx = env::var("CXX").unwrap_or_else(|_| format!("{prefix}g++"));

    // ARMv6 flags only when targeting the Raspberry Pi Zero
    let target = env::var("TARGET").unwrap_or_default();
    let is_armv6 = target.contains("arm-unknown-linux-gnueabihf") || target.contains("armv6");

    // Build the C++ library out of tree, in OUT_DIR: one per target (host for
    // rust-analyzer, ARM for cross), so the two builds never overwrite each other's
    // objects. The submodule sources are only read.
    let build_dir = PathBuf::from(env::var("OUT_DIR").unwrap()).join("rgbmatrix");
    // The compiler doesn't create directories: rp1/*.o objects need rp1/ to exist
    std::fs::create_dir_all(build_dir.join("rp1")).expect("Failed to create rgbmatrix build dir");

    // LTO is disabled: the static lib must contain machine code, not GCC LTO bytecode
    // (which rust-lld can't read)
    let status = Command::new("make")
        // Share Cargo's job slots with make: the C++ files compile in parallel
        .env("MAKEFLAGS", env::var("CARGO_MAKEFLAGS").unwrap_or_default())
        .arg("-C") // run in the build dir: objects and librgbmatrix.a are written there
        .arg(&build_dir)
        .arg("-f") // ...but use the library's own Makefile
        .arg(lib_dir.join("Makefile"))
        .arg("-I") // where `include ../config.mk` is resolved from
        .arg(&lib_dir)
        // Sources and headers only: a plain VPATH would also pick up stale .o files
        // left by an in-place build of the submodule
        .arg(format!("--eval=vpath %.cc {}", lib_dir.display()))
        .arg(format!("--eval=vpath %.c {}", lib_dir.display()))
        .arg(format!("--eval=vpath %.h {}", lib_dir.display()))
        // Paths the Makefile defines relative to lib/, made absolute
        .arg(format!("INCDIR={}", include_dir.display()))
        .arg(format!(
            "RP1_PIO_VENDOR_DIR={}",
            lib_dir.join("rp1/rp1_pio_vendor").display()
        ))
        .arg("librgbmatrix.a")
        .arg("LTO_FLAGS=")
        .arg(format!("CC={cc}"))
        .arg(format!("CXX={cxx}"))
        .args(is_armv6.then_some("CPU_ARCH_FLAGS=-march=armv6"))
        .status()
        .expect("Failed to run make for rgbmatrix");
    assert!(status.success(), "rgbmatrix make failed");

    // Link the static library, and the C++ standard library it needs
    println!("cargo:rustc-link-search=native={}", build_dir.display());
    println!("cargo:rustc-link-lib=static=rgbmatrix"); // librgbmatrix.a
    println!("cargo:rustc-link-lib=stdc++");

    // Host builds (rust-lld): keep the custom sections bounded by __start_/__stop_ (used
    // by the RP1 code). The cross GNU ld keeps them by default, and is too old to know
    // this option: it would only warn about it.
    if !is_armv6 {
        println!("cargo:rustc-link-arg=-Wl,-z,nostart-stop-gc");
    }

    // Run this script again when the C library or its headers change
    println!("cargo:rerun-if-changed={}", wrapper_path.display());
    println!("cargo:rerun-if-changed={}", include_dir.display());
    println!("cargo:rerun-if-changed={}", lib_dir.display());
    println!(
        "cargo:rerun-if-changed={}",
        matrix_dir.join("config.mk").display()
    );

    // Generate the bindings. bindgen reads TARGET and BINDGEN_EXTRA_CLANG_ARGS_<target>
    // itself: nothing to configure for cross-compilation.
    let bindings = bindgen::Builder::default()
        .header(wrapper_path.to_str().unwrap())
        // Only the matrix C API (and the types it uses), not everything its headers include
        .allowlist_file(".*led-matrix-c\\.h")
        .clang_arg(format!("-I{}", include_dir.display()))
        .parse_callbacks(Box::new(bindgen::CargoCallbacks::new()))
        .generate()
        .expect("Unable to generate bindings");

    // Written to OUT_DIR, and included by `src/matrix/ffi.rs`
    let out_path = PathBuf::from(env::var("OUT_DIR").unwrap());
    bindings
        .write_to_file(out_path.join("bindings.rs"))
        .expect("Couldn't write bindings!");
}
