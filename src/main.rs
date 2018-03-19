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

use std::fs::*;
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

fn check_file(package: &Package, bin_dir: &std::path::PathBuf) {
    let mut print_string = String::new();
    print_string.push_str(&format!("checking: {} {}", package.name, package.version));

    for binary in &package.binaries {
        let mut bin_path: std::path::PathBuf = bin_dir.clone();
        bin_path.push(&binary);
        let binary_path = bin_path.into_os_string().into_string().unwrap();
        match Command::new("ldd").arg(&binary_path).output() {
            Ok(out) => {
                let output = String::from_utf8_lossy(&out.stdout);
                let output = output.into_owned();
                let mut first = true;
                for line in output.lines() {
                    if line.ends_with("=> not found") {
                        if first {
                            print_string.push_str(&format!("\nbinary: '{}' is missing\n", &binary));
                        }
                        print_string.push_str(&format!(
                            "\t\t{}\n",
                            line.replace("=> not found", "").trim()
                        ));
                        first = false;
                    }
                    //println!("{}", line);
                }
            }
            Err(e) => panic!("ERROR '{}'", e),
        }
    }
    if print_string.len() > 1 {
        println!("{}", print_string.trim());
    }
}

fn main() {
    let cargo_cfg = cargo::util::config::Config::default().unwrap();
    let mut bin_dir = cargo_cfg.home().clone().into_path_unlocked();
    bin_dir.push("bin");

    let mut crates_index = cargo_cfg.home().clone();
    crates_index.push(".crates.toml");

    let mut files = Vec::new();
    for file in read_dir(&bin_dir).unwrap() {
        files.push(file.unwrap());
    }

    let mut f = File::open(crates_index.into_path_unlocked()).expect("file not found");

    let mut file_content = String::new();
    f.read_to_string(&mut file_content)
        .expect(&format!("Error: could not read '{}'", file_content));

    let mut file_iter = file_content.lines().into_iter();
    let first_line = file_iter.next();
    assert_eq!(first_line.unwrap(), "[v1]", "Error: Api changed!");

    let mut packages = Vec::new();

    for line in file_iter {
        let line_split: Vec<&str> = line.split(' ').collect();
        let name = line_split[0].to_string().replace("\"", "");
        let version = line_split[1].to_string();
        let source = line_split[2].to_string();
        let mut binaries = Vec::new();

        // collect the binaries a crate has installed
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
        packages.push(package);
    }

    // iterate over the acquired metadata and check for broken library links
    packages
        .par_iter()
        .for_each(|binary| check_file(binary, &bin_dir));
}
