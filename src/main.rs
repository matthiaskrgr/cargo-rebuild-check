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
    //metadata: String,
    name: String,
    version: String,
    source: String,
    binaries: Vec<String>,
}

fn check_file(path: &DirEntry) {
    let mut print_string = String::new();
    let name_string = path.path();
    print_string.push_str(&format!("checking: {}", &name_string.display()));

    match Command::new("ldd").arg(&name_string).output() {
        Ok(out) => {
            //    println!("git gc error\nstatus: {}", out.status);
            //    println!("stdout:\n {}", String::from_utf8_lossy(&out.stdout));
            //    println!("stderr:\n {}", String::from_utf8_lossy(&out.stderr));
            //if out.status.success() {}
            let output = String::from_utf8_lossy(&out.stdout);
            let output = output.into_owned();
            let mut first = true;
            for line in output.lines() {
                if line.ends_with("=> not found") {
                    if first {
                        print_string.push_str(&format!("\nbinary: {}\n", &name_string.display()));
                    }
                    print_string.push_str(&format!(
                        "\t\t is missing: {}\n",
                        line.replace("=> not found", "").trim()
                    ));
                    first = false;
                }
                //println!("{}", line);
            }
        }
        Err(e) => panic!("ERROR '{}'", e),
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
    assert_eq!(first_line.unwrap(), "[v1]", "api changed!");

    let mut packages = Vec::new();

    for line in file_iter {
        let line_split: Vec<&str> = line.split(' ').collect();
        let name = line_split[0].to_string();
        let version = line_split[1].to_string();
        let source = line_split[2].to_string();
        let mut binaries = Vec::new();
        for bin in line_split[4..].iter() {
            binaries.push(bin.to_string());
        }
        let package = Package {
            name,
            version,
            source,
            binaries,
        };
        packages.push(package);
    }

    files.par_iter().for_each(|binary| check_file(binary));
}
