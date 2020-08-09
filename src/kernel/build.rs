use std::env;
use std::path::Path;
use std::process::Command;

fn main() {
    let out_dir = env::var("OUT_DIR").unwrap();

    let target_arch: String = env::var("CARGO_CFG_TARGET_ARCH").unwrap();

    match &*target_arch {
        "x86_64" => {
            Command::new("nasm")
                .args(&["-f", "win64", "src/arch/x86_64/asm.asm", "-o"])
                .arg(&format!("{}/asm.o", out_dir))
                .status()
                .unwrap();

            Command::new("ar")
                .args(&["crus", "libkernel.a", "asm.o"])
                .current_dir(&Path::new(&out_dir))
                .status()
                .unwrap();
        }
        _ => {
            println!("cargo:warning=TARGET_ARCH {} IS NOT SUPPORTED", target_arch);
            std::process::exit(1);
        }
    }

    println!("cargo:rustc-link-search=native={}", out_dir);
    println!("cargo:rustc-link-lib=static=kernel");
    println!("cargo:rerun-if-changed=src/arch/**/*.asm");
}
