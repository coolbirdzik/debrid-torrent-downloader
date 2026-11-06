fn main() {
    // Recompile native resources when the artwork changes, even if the config
    // and Rust sources stay the same. Otherwise incremental builds keep the old ICO.
    println!("cargo:rerun-if-changed=icons");
    tauri_build::build()
}
