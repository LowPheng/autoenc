use std::{env, path::PathBuf};

fn main() {
    let os = env::var("CARGO_CFG_TARGET_OS").unwrap();
    let arch = env::var("CARGO_CFG_TARGET_ARCH").unwrap();
    if os != "windows" || arch != "x86" {
        panic!("Only Windows x86 target supported");
    }

    let manifest_dir = PathBuf::from(env::var_os("CARGO_MANIFEST_DIR").unwrap());

    let def = manifest_dir.join("proxy.def");
    println!("cargo:rerun-if-changed={}", def.display());
    match env::var("CARGO_CFG_TARGET_ENV").as_deref() {
        Ok("msvc") => {
            println!("cargo:rustc-link-arg=/DEF:{}", def.display());
        }
        Ok("gnu") => {
            println!("cargo:rustc-link-arg={}", def.display());
        }
        _ => {}
    }
}
