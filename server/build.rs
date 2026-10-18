use std::{fs, path::PathBuf};

fn main() {
    let dist = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../client/web/dist");
    println!("cargo:rerun-if-changed={}", dist.display());

    let index = dist.join("index.html");
    if index.exists() {
        return;
    }

    // Allow `cargo check` without a prior frontend build.
    fs::create_dir_all(&dist).expect("create stub client/web/dist");
    fs::write(
        index,
        b"<!doctype html><title>nimbus</title><p>Build @nimbus/web before running the server.</p>\n",
    )
    .expect("write stub index.html");
}
