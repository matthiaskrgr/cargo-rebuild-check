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

use std::fs;
use std::process::Command;

fn main() {
    let cargo_cfg = cargo::util::config::Config::default().unwrap();
    let mut bin_dir = cargo_cfg.home().clone().into_path_unlocked();
    bin_dir.push("bin");
    // check all files in this dir
    for binary in fs::read_dir(&bin_dir).unwrap() {
        let path = binary.unwrap();
        let name_string = path.path();
        println!("checking: {}", &name_string.display());
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
                            println!("binary: {}", name_string.display());
                        }
                        println!("\t\t is missing: {}", line.replace("=> not found", "").trim());
                        first = false;
                    }
                    //println!("{}", line);
                }
            }
            Err(e) => println!("ERROR '{}'", e),
        }
    }
}
