// SPDX-FileCopyrightText: 2026 KOGA Mitsuhiro
// SPDX-License-Identifier: MIT

fn main() {
    println!("cargo:rerun-if-changed=src/android.cpp");
    if std::env::var("CARGO_CFG_TARGET_OS").as_deref() == Ok("android") {
        cc::Build::new()
            .cpp(true)
            .file("src/android.cpp")
            .flag("-std=c++17")
            .cpp_link_stdlib("c++_static")
            .compile("kyber_media");
        for library in ["mediandk", "android", "c++abi"] {
            println!("cargo:rustc-link-lib={library}");
        }
    }
    println!("cargo:rerun-if-changed=src/windows.cpp");
    for file in [
        "windows_encoder.h",
        "windows_gpu.h",
        "apple_encoder.h",
        "linux_encoder.h",
        "android_encoder.h",
    ] {
        println!("cargo:rerun-if-changed=src/{file}");
    }
    println!("cargo:rerun-if-changed=src/linux.cpp");
    println!("cargo:rerun-if-changed=src/apple.mm");
    if matches!(
        std::env::var("CARGO_CFG_TARGET_OS").as_deref(),
        Ok("macos" | "ios")
    ) {
        let mut build = cc::Build::new();
        if std::env::var("CARGO_CFG_TARGET_OS").as_deref() == Ok("ios") {
            println!("cargo:rerun-if-changed=src/ios_picker.mm");
            build.file("src/ios_picker.mm");
            println!("cargo:rustc-link-lib=framework=UIKit");
            println!("cargo:rustc-link-lib=framework=UniformTypeIdentifiers");
        }
        if std::env::var("CARGO_CFG_TARGET_OS").as_deref() == Ok("macos") {
            build.flag(format!(
                "-mmacosx-version-min={}",
                std::env::var("MACOSX_DEPLOYMENT_TARGET").unwrap_or_else(|_| "11.0".into())
            ));
        }
        build
            .cpp(true)
            .file("src/apple.mm")
            .flag("-std=c++17")
            .flag("-fobjc-arc")
            .flag("-Wno-deprecated-declarations")
            .compile("kyber_media");
        for framework in [
            "AVFoundation",
            "VideoToolbox",
            "CoreMedia",
            "CoreVideo",
            "Foundation",
            "CoreGraphics",
        ] {
            println!("cargo:rustc-link-lib=framework={framework}");
        }
    }
    if std::env::var("CARGO_CFG_TARGET_OS").as_deref() == Ok("windows") {
        cc::Build::new()
            .cpp(true)
            .file("src/windows.cpp")
            .flag_if_supported("/std:c++17")
            .flag_if_supported("/EHsc")
            .compile("kyber_media");
        for library in ["mfuuid", "ole32", "oleaut32", "wmcodecdspuuid"] {
            println!("cargo:rustc-link-lib={library}");
        }
    }
    if std::env::var("CARGO_CFG_TARGET_OS").as_deref() == Ok("linux") {
        let mut build = cc::Build::new();
        for package in [
            "gstreamer-app-1.0",
            "gstreamer-video-1.0",
            "gstreamer-pbutils-1.0",
        ] {
            let library = pkg_config::Config::new()
                .atleast_version("1.20")
                .cargo_metadata(false)
                .probe(package)
                .expect("Install GStreamer development headers (see docs/development.md)");
            for include in library.include_paths {
                build.include(include);
            }
        }
        build
            .cpp(true)
            .file("src/linux.cpp")
            .flag("-std=c++17")
            .compile("kyber_media");
        println!("cargo:rustc-link-lib=dl");
    }
}
