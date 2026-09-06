fn main() {
    if std::env::var("CARGO_CFG_TARGET_OS").as_deref() == Ok("macos") {
        println!("cargo:rerun-if-changed=src/macos/native.m");
        cc::Build::new()
            .file("src/macos/native.m")
            .flag("-fobjc-arc")
            .flag("-fblocks")
            .compile("clipmo_macos");
        println!("cargo:rustc-link-lib=framework=AppKit");
        println!("cargo:rustc-link-lib=framework=ApplicationServices");
    }
    tauri_build::build();
    prepare_webview2_loader();
}

/// GNU Windows builds load WebView2Loader.dll dynamically. Put the loader next
/// to the executable where Tauri expects it, deriving that directory from
/// OUT_DIR instead of assuming a repository-local `target` directory. This is
/// safe for `--target` and explicit/Cargo-configured CARGO_TARGET_DIR builds.
fn prepare_webview2_loader() {
    use std::path::PathBuf;

    let target = std::env::var("TARGET").unwrap_or_default();
    if !target.ends_with("-windows-gnu") {
        return;
    }
    if !target.starts_with("x86_64-") {
        panic!("Clipdeck release packaging only supports x64 Windows; got {target}");
    }

    let out_dir = PathBuf::from(
        std::env::var_os("OUT_DIR").expect("Cargo must set OUT_DIR for build scripts"),
    );
    // OUT_DIR is <target>/<triple>/<profile>/build/<crate-hash>/out.
    let profile_dir = out_dir
        .ancestors()
        .nth(3)
        .expect("unexpected Cargo OUT_DIR layout");
    let destination = profile_dir.join("WebView2Loader.dll");

    // Never let an earlier build's generic loader mask a missing/wrong target
    // loader. The dependency output below is authoritative for this build.
    if destination.exists() {
        std::fs::remove_file(&destination).unwrap_or_else(|error| {
            panic!("could not remove stale {}: {error}", destination.display())
        });
    }

    let build_dir = profile_dir.join("build");
    let loader = std::fs::read_dir(&build_dir)
        .unwrap_or_else(|error| panic!("could not inspect {}: {error}", build_dir.display()))
        .filter_map(Result::ok)
        .filter(|entry| {
            entry
                .file_name()
                .to_string_lossy()
                .starts_with("webview2-com-sys-")
        })
        .map(|entry| entry.path().join("out/x64/WebView2Loader.dll"))
        .find(|candidate| {
            candidate
                .metadata()
                .is_ok_and(|metadata| metadata.len() > 0)
        })
        .unwrap_or_else(|| {
            panic!(
                "x64 WebView2Loader.dll was not produced under {}",
                build_dir.display()
            )
        });

    std::fs::copy(&loader, &destination).unwrap_or_else(|error| {
        panic!(
            "could not copy {} to {}: {error}",
            loader.display(),
            destination.display()
        )
    });
    println!("cargo:rerun-if-changed={}", loader.display());
}
