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
//#![cfg_attr(feature = "cargo-clippy", warn(result_unwrap_used))]

extern crate cargo;
extern crate rayon;

use std::process::Command;
use rayon::prelude::*;
use std::fs::File;
use std::io::prelude::*;

// deserialize the ~/.cargo/.crates.toml

#[derive(Debug)]
struct Package {
    name: String,
    version: String,
    source: String,
    binaries: Vec<String>,
}

fn check_binary(
    package: &Package,
    bin_dir: &std::path::PathBuf,
    rust_lib_path: &str,
) -> Option<String> {
    let mut print_string =
        format!("  Checking crate {} {}", package.name, package.version).to_string();

    let mut outdated_package: Option<String> = None;
    for binary in &package.binaries {
        let mut bin_path: std::path::PathBuf = bin_dir.clone();
        bin_path.push(&binary);
        let binary_path = bin_path.into_os_string().into_string().unwrap();
        match Command::new("ldd")
            .arg(&binary_path)
            .env("LD_LIBRARY_PATH", rust_lib_path)
            .output()
        {
            Ok(out) => {
                let output = String::from_utf8_lossy(&out.stdout).into_owned();
                let mut first = true;
                for line in output.lines() {
                    if line.ends_with("=> not found") {
                        if first {
                            // package needs rebuild
                            outdated_package = Some(package.name.clone());
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
            Err(e) => panic!("Error while runnning ldd: '{}'", e),
        } // match
    } // for binary in &package.binaries

    println!("{}", print_string);
    outdated_package
}

fn main() {
    let cargo_cfg = cargo::util::config::Config::default().unwrap();
    let mut bin_dir = cargo_cfg.home().clone().into_path_unlocked();
    bin_dir.push("bin");

    let mut crates_index = cargo_cfg.home().clone();
    crates_index.push(".crates.toml");

    let mut f = File::open(crates_index.into_path_unlocked()).expect("file not found");

    let mut file_content = String::new();
    f.read_to_string(&mut file_content)
        .expect(&format!("Error: could not read '{}'", file_content));

    let mut file_iter = file_content.lines().into_iter();
    // skip the first line when unwrapping
    // the first line also tells the api version, so assert that we are sort of compatible
    assert_eq!(file_iter.next().unwrap(), "[v1]", "Error: Api changed!");

    let mut packages = Vec::new();

    for line in file_iter {
        let line_split: Vec<&str> = line.split(' ').collect();
        let name = line_split[0].to_string().replace("\"", "");
        let version = line_split[1].to_string();
        let source = line_split[2].to_string();
        let mut binaries = Vec::new();

        // collect the binaries a crate has installed

        // the line looks like this:
        // "rustfmt-nightly 0.4.1 (registry+https://github.com/rust-lang/crates.io-index)" = ["cargo-fmt", "git-rustfmt", "rustfmt", "rustfmt-format-diff"]
        // split at the "=" and get everything after it
        let bins_split_from_line: Vec<&str> = line.split('=').collect();
        let bins = bins_split_from_line.last().unwrap();
        for bin in bins.split(',') {
            // clean up, remove characters remaining from toml encoding
            let binary: String = bin.replace("[", "")
                .replace("]", "")
                .replace("\"", "")
                .trim()
                .to_string();
            binaries.push(binary);
        }

        let package = Package {
            name,
            version,
            source,
            binaries,
        };
        // collect the packages
        packages.push(package);
    }

    // get the path where rustc libs are stored: $(rustc --print sysroot)/lib
    let rust_lib_path = match Command::new("rustc").arg("--print").arg("sysroot").output() {
        Ok(out) => {
            let mut output = String::from_utf8_lossy(&out.stdout).into_owned();
            // remove \n
            output.pop();
            let mut path = std::path::PathBuf::from(output);
            path.push("lib");
            path
        }
        Err(e) => panic!("Error getting rustc sysroot path '{}'", e),
    };

    let rust_lib_path_string = rust_lib_path
        .into_os_string()
        .into_string()
        .expect("Failed to convert pathBuf to String");

    // iterate (in parallel) over the acquired metadata and check for broken library links
    // filter out all None values, only collect the Some() ones
    let broken_pkgs: Vec<String> = packages
        .par_iter()
        .filter_map(|binary| check_binary(binary, &bin_dir, &rust_lib_path_string))
        .collect();

    if !broken_pkgs.is_empty() {
        println!("\n  Crates needing rebuild: {}", broken_pkgs.join(" "));
        std::process::exit(2);
    } else {
        println!("\n  Everything looks good.");
    }
}
