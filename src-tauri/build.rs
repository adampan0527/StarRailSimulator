use std::path::Path;

fn main() {
    let catalog = "../.m03-data/4.7.0/catalog.json";
    println!("cargo:rerun-if-changed={catalog}");
    println!("cargo:rustc-check-cfg=cfg(bundled_catalog)");
    if Path::new(catalog).is_file() {
        println!("cargo:rustc-cfg=bundled_catalog");
    }

    let archiver = "../.archiver/reliquary-archiver-pktmon-x64.exe";
    println!("cargo:rerun-if-changed={archiver}");
    println!("cargo:rustc-check-cfg=cfg(bundled_archiver)");
    if Path::new(archiver).is_file() {
        println!("cargo:rustc-cfg=bundled_archiver");
    } else if std::env::var_os("CARGO_FEATURE_CUSTOM_PROTOCOL").is_some() {
        panic!("Bundled Archiver missing. Build with npm run desktop:build.");
    }

    tauri_build::build();
}
