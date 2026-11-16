use std::path::Path;

fn main() {
    let catalog = "../.m03-data/4.7.0/catalog.json";
    println!("cargo:rerun-if-changed={catalog}");
    println!("cargo:rustc-check-cfg=cfg(bundled_catalog)");
    if Path::new(catalog).is_file() {
        println!("cargo:rustc-cfg=bundled_catalog");
    }

    tauri_build::build();
}
