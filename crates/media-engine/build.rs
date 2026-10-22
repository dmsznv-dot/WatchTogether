fn main() {
    if std::env::var("CARGO_CFG_TARGET_OS").as_deref() == Ok("macos") {
        cc::Build::new()
            .file("src/mpv/macos_surface.m")
            .flag("-fno-objc-arc")
            .flag("-Wno-deprecated-declarations")
            .compile("watch_together_surface");
        println!("cargo:rustc-link-lib=framework=AppKit");
        println!("cargo:rustc-link-lib=framework=OpenGL");
        println!("cargo:rerun-if-changed=src/mpv/macos_surface.m");
    }
}
