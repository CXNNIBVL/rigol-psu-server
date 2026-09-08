//! Guarantees the embed directory exists at compile time.
//!
//! `rust-embed` fails the build outright if its folder is missing, which would
//! make `cargo check` depend on having run `npm` first. Rather than have the
//! build shell out to npm — slow, and surprising in an editor — this creates a
//! placeholder so the Rust side always compiles, and anyone who reaches the
//! server without building the UI gets a page telling them so.

use std::path::Path;

fn main() {
    let dist = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../ui/dist");
    println!("cargo:rerun-if-changed={}", dist.display());

    if dist.join("index.html").exists() {
        return;
    }
    if std::fs::create_dir_all(&dist).is_err() {
        return;
    }
    let _ = std::fs::write(
        dist.join("index.html"),
        "<!doctype html><meta charset=\"utf-8\"><title>UI not built</title>\
         <body style=\"font-family:system-ui;padding:2rem\">\
         <h1>The UI has not been built</h1>\
         <p>Run <code>just build-ui</code> (or <code>npm run build</code> in <code>ui/</code>) \
         and restart the server.</p>",
    );
}
