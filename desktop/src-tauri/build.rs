fn main() {
    // tauri-build 2.6.3 does not track the ICO input to the Windows resource compiler.
    println!("cargo:rerun-if-changed=icons");
    tauri_build::build()
}
