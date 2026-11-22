// SPDX-FileCopyrightText: 2026 KOGA Mitsuhiro
// SPDX-License-Identifier: MIT

use std::{env, path::PathBuf};

fn main() {
    let source = PathBuf::from(env::var_os("CARGO_MANIFEST_DIR").unwrap())
        .join("../../kynput");
    let out = PathBuf::from(env::var_os("OUT_DIR").unwrap());
    println!("cargo:rerun-if-changed={}", source.display());
    let config = cbindgen::Config::from_file(source.join("cbindgen.toml")).unwrap();
    cbindgen::Builder::new()
        .with_crate(&source)
        .with_config(config)
        .generate()
        .expect("Generate embedded kynput header")
        .write_to_file(out.join("kynput.h"));
    let mut builder = bindgen::Builder::default();
    if env::var("CARGO_CFG_TARGET_OS").as_deref() == Ok("windows") {
        // Preserve the upstream unsigned C enum representation on MSVC too.
        builder = builder.clang_arg("--target=x86_64-w64-windows-gnu");
    }
    builder
        .header(out.join("kynput.h").to_str().unwrap())
        .use_core()
        .ctypes_prefix("libc")
        .allowlist_function(r"(\w*kynput_\w*)")
        .allowlist_type(r"(\w*kynput_\w*)")
        .generate()
        .expect("Generate embedded kynput bindings")
        .write_to_file(out.join("bindings.rs"))
        .unwrap();
}
