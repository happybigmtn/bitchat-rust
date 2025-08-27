use std::env;

fn main() {
    // Generate UniFFI scaffolding
    #[cfg(feature = "uniffi")]
    {
        uniffi::generate_scaffolding("src/bitcraps.udl")
            .expect("Failed to generate UniFFI scaffolding");
    }
    
    // Configure Android linking
    if env::var("CARGO_CFG_TARGET_OS").unwrap_or_default() == "android" {
        println!("cargo:rustc-link-lib=log");
        println!("cargo:rustc-link-lib=android");
    }
    
    // Rebuild if UDL file changes
    println!("cargo:rerun-if-changed=src/bitcraps.udl");
    println!("cargo:rerun-if-changed=build.rs");
    
    // Set up output directory for generated bindings
    let _out_dir = env::var_os("OUT_DIR").unwrap();
    
    // Platform-specific linking is handled by UniFFI automatically
}