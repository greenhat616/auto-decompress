use std::env;
use std::path::PathBuf;

fn main() {
    let manifest_dir = PathBuf::from(env::var("CARGO_MANIFEST_DIR").unwrap());
    let cpp_dir = manifest_dir.join("cpp");

    // Find bit7z library
    // Users should set BIT7Z_DIR environment variable to the bit7z installation directory
    // which should contain include/ and lib/ subdirectories
    let bit7z_dir = env::var("BIT7Z_DIR")
        .map(PathBuf::from)
        .unwrap_or_else(|_| {
            // Check for bit7z-src in workspace root (development setup)
            let workspace_bit7z = manifest_dir.parent().unwrap().join("bit7z-src");
            if workspace_bit7z.exists() {
                return workspace_bit7z;
            }
            // Default search paths
            if cfg!(windows) {
                PathBuf::from("C:/bit7z")
            } else {
                PathBuf::from("/usr/local")
            }
        });

    let bit7z_include = bit7z_dir.join("include");
    // bit7z builds to lib/x64/Release on Windows
    let bit7z_lib = if cfg!(windows) {
        bit7z_dir.join("lib").join("x64").join("Release")
    } else {
        bit7z_dir.join("lib")
    };

    // Check if bit7z is available
    if !bit7z_include.exists() {
        println!(
            "cargo::warning=bit7z include directory not found at {:?}",
            bit7z_include
        );
        println!("cargo::warning=Set BIT7Z_DIR environment variable to your bit7z installation");
        println!(
            "cargo::warning=Expected structure: $BIT7Z_DIR/include/bit7z/ and $BIT7Z_DIR/lib/"
        );
    }

    // Build the cxx bridge
    let mut build = cxx_build::bridge("src/ffi.rs");

    // OUT_DIR contains the generated cxx header
    let out_dir = PathBuf::from(env::var("OUT_DIR").unwrap());

    // Add workspace root for include paths
    let workspace_root = manifest_dir.parent().unwrap();

    build
        .file(cpp_dir.join("bit7z_bridge.cpp"))
        .include(&out_dir)
        .include(&cpp_dir)
        .include(&manifest_dir)
        .include(workspace_root)
        .include(&bit7z_include)
        .std("c++17")
        .warnings(false);

    // Platform-specific settings
    if cfg!(windows) {
        build
            .define("_WIN32", None)
            .define("NOMINMAX", None)
            .define("WIN32_LEAN_AND_MEAN", None);

        // MSVC-specific flags
        if env::var("CARGO_CFG_TARGET_ENV").as_deref() == Ok("msvc") {
            build.flag("/EHsc").flag("/utf-8");
        }
    }

    // Optional: enable auto format detection
    if env::var("BIT7Z_AUTO_FORMAT").is_ok() {
        build.define("BIT7Z_AUTO_FORMAT", None);
    }

    build.compile("bit7z_wrapper");

    // Link to bit7z library
    println!("cargo:rustc-link-search=native={}", bit7z_lib.display());
    println!("cargo:rustc-link-lib=static=bit7z");

    // Link to system libraries
    if cfg!(windows) {
        println!("cargo:rustc-link-lib=oleaut32");
        println!("cargo:rustc-link-lib=ole32");
        println!("cargo:rustc-link-lib=user32");
    }

    // Rerun if these change
    println!("cargo:rerun-if-changed=src/ffi.rs");
    println!("cargo:rerun-if-changed=cpp/bit7z_bridge.hpp");
    println!("cargo:rerun-if-changed=cpp/bit7z_bridge.cpp");
    println!("cargo:rerun-if-env-changed=BIT7Z_DIR");
    println!("cargo:rerun-if-env-changed=BIT7Z_AUTO_FORMAT");
}
