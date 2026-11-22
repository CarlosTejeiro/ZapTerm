fn main() {
    // A beta build (from a `-beta` tag) also offers betas in Check for
    // updates; see commands/update.rs.
    println!("cargo:rerun-if-env-changed=ZAPTERM_CHANNEL");
    tauri_build::build()
}
