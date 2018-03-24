// enable additional rustc warnings
#![warn(trivial_casts, trivial_numeric_casts, unsafe_code)]
// enable additional clippy warnings
#![cfg_attr(feature = "cargo-clippy", warn(int_plus_one))]
#![cfg_attr(feature = "cargo-clippy", warn(shadow_reuse, shadow_same, shadow_unrelated))]
#![cfg_attr(feature = "cargo-clippy", warn(mut_mut))]
#![cfg_attr(feature = "cargo-clippy", warn(nonminimal_bool))]
#![cfg_attr(feature = "cargo-clippy", warn(pub_enum_variant_names))]
#![cfg_attr(feature = "cargo-clippy", warn(range_plus_one))]
#![cfg_attr(feature = "cargo-clippy", warn(string_add, string_add_assign))]
#![cfg_attr(feature = "cargo-clippy", warn(stutter))]

use std;
use std::process::Command;

// a package that we may need to rebuild
pub struct CrateInfo {
    pub name: String,
    pub version: String,
    pub git: Option<String>,
    pub branch: Option<String>,
    pub tag: Option<String>,
    pub rev: Option<String>,
    pub registry: Option<String>,
    pub path: Option<String>,
    pub binaries: Vec<String>,
}

pub fn run_cargo_install(binary: &str, args: &[String], list_of_failures: &mut Vec<String>) {
    println!("Reinstalling {}", binary);
    let mut cargo = Command::new("cargo");
    cargo.arg("install");
    cargo.arg(binary);
    cargo.arg("--force");
    for argument in args {
        // don't pass empty argument to cargo
        if !argument.is_empty() {
            cargo.arg(argument);
        }
    }

    let cargo_status = cargo.status();
    match cargo_status {
        Ok(status) => {
            // bad exit status of cargo, build failed?
            if !status.success() {
                list_of_failures.push(binary.to_string());
            }
        }
        Err(_) => {
            // maybe cargo crashed?
            list_of_failures.push(binary.to_string());
        }
    }
}

pub fn check_binary<'a>(
    package: &'a CrateInfo,
    bin_dir: &std::path::PathBuf,
    rust_lib_path: &str,
) -> Option<&'a CrateInfo> {
    let mut print_string =
        format!("  Checking crate {} {}", package.name, package.version).to_string();

    let mut outdated_package: Option<&CrateInfo> = None;
    for binary in &package.binaries {
        let mut bin_path: std::path::PathBuf = bin_dir.clone();
        bin_path.push(&binary);
        let binary_path = bin_path.into_os_string().into_string().unwrap();
        match Command::new("ldd")
            .arg(&binary_path)
            .env("LD_LIBRARY_PATH", rust_lib_path)
            // try to enfore english output to stabilize parsing
            .env("LANG", "en_US")
            .env("LC_ALL", "en_US")
            .output()
        {
            Ok(out) => {
                let output = String::from_utf8_lossy(&out.stdout).into_owned();
                let mut first = true;
                for line in output.lines() {
                    if line.ends_with("=> not found") {
                        if first {
                            // package needs rebuild
                            outdated_package = Some(package);
                            print_string
                                .push_str(&format!("\n    Binary '{}' is missing:\n", &binary));
                        }
                        print_string.push_str(&format!(
                            "\t\t{}\n",
                            line.replace("=> not found", "").trim()
                        ));
                        first = false;
                    }
                }
            }
            Err(e) => eprintln!("Error while runnning ldd: '{}'", e),
        } // match
    } // for binary in &package.binaries

    println!("{}", print_string);
    outdated_package
}
