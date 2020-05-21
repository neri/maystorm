use std::env;
use std::path::Path;
use std::process::Command;

fn main() {
    let out_dir = env::var("OUT_DIR").unwrap();

    Command::new("nasm")
        .args(&["-f", "win64", "src/myos/arch/asm.asm", "-o"])
        .arg(&format!("{}/asm.o", out_dir))
        .status()
        .unwrap();

    Command::new("ar")
        .args(&["crus", "libmyos.a", "asm.o"])
        .current_dir(&Path::new(&out_dir))
        .status()
        .unwrap();

    println!("cargo:rustc-link-search=native={}", out_dir);
    println!("cargo:rustc-link-lib=static=myos");
    println!("cargo:rerun-if-changed=src/myos/arch/*.asm");
}
