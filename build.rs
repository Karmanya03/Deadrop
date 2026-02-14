use std::path::Path;

fn main() {
    println!("cargo:rerun-if-changed=wasm/src/lib.rs");
    println!("cargo:rerun-if-changed=wasm/Cargo.toml");
    println!("cargo:rerun-if-changed=web/index.html");
    println!("cargo:rerun-if-changed=web/worker.js");
    println!("cargo:rerun-if-changed=web/style.css");

    let wasm_pkg = Path::new("wasm/pkg/deadrop_wasm_bg.wasm");

    // Skip WASM build if pkg already exists (pre-built for publishing)
    if wasm_pkg.exists() {
        println!("cargo:warning=WASM module already built, skipping wasm-pack");
        return;
    }

    // Only build WASM in development
    let status = std::process::Command::new("wasm-pack")
        .args(["build", "--target", "web", "--release", "wasm/"])
        .status();

    match status {
        Ok(s) if s.success() => {}
        Ok(s) => {
            println!("cargo:warning=wasm-pack exited with: {}", s);
        }
        Err(e) => {
            println!("cargo:warning=wasm-pack not found ({}). Using pre-built WASM.", e);
        }
    }
}
