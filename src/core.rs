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

extern crate cargo;
extern crate rayon;
extern crate test;

use std;
use std::process::Command;

use self::rayon::iter::*;

use parse::*;

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

fn check_bin_with_ldd(
    binary_path: &str,
    rustc_lib_path: &str,
) -> Result<std::process::Output, std::io::Error> {
    Command::new("ldd")
        .arg(&binary_path)
        .env("LD_LIBRARY_PATH", rustc_lib_path)
        // try to enfore english output to stabilize parsing
        .env("LANG", "en_US")
        .env("LC_ALL", "en_US")
        .output()
}

pub fn check_binary<'a>(
    package: &'a CrateInfo,
    bin_dir: &std::path::PathBuf,
    rustc_lib_path: &str,
) -> Option<&'a CrateInfo> {
    let mut print_string =
        format!("  Checking crate {} {}", package.name, package.version).to_string();

    let mut outdated_package: Option<&CrateInfo> = None;
    for binary in &package.binaries {
        let mut bin_path: std::path::PathBuf = bin_dir.clone();
        bin_path.push(&binary);
        let binary_path = bin_path.into_os_string().into_string().unwrap();
        let ldd_output = check_bin_with_ldd(&binary_path, &rustc_lib_path);
        match ldd_output {
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
            Err(e) => eprintln!("Error while running ldd: '{}'", e),
        } // match
    } // for binary in &package.binaries

    println!("{}", print_string);
    outdated_package
}

pub fn get_rustc_lib_path() -> String {
    let rust_lib_path = match Command::new("rustc")
        .arg("--print")
        .arg("sysroot")
        .env("LANG", "en_US")
        .env("LC_ALL", "en_US")
        .output()
    {
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

    rust_lib_path_string
}

pub fn check_and_rebuild_broken_crates(
    packages: Vec<CrateInfo>,
    rust_lib_path: &str,
    bin_dir: &std::path::PathBuf,
    do_auto_rebuild: bool,
) {
    // iterate (in parallel) over the acquired metadata and check for broken library links
    // filter out all None values, only collect the Some() ones

    // todo: can we avoid sorting into a separate vector here?
    let broken_pkgs: Vec<&CrateInfo> = packages
        .par_iter()
        .filter_map(|binary| check_binary(binary, &bin_dir, &rust_lib_path))
        .collect();

    let rebuilds_required: bool = !broken_pkgs.is_empty();

    if rebuilds_required {
        // concat list of names of crates needing rebuilding
        let mut pkgs_string = String::new();
        for pkg in &broken_pkgs {
            pkgs_string.push_str(&pkg.name);
            pkgs_string.push_str(" ");
        }
        println!("\n  Crates needing rebuild: {}", pkgs_string.trim());
        if !do_auto_rebuild {
            std::process::exit(2);
        }
    } else {
        println!("\n  Everything looks good! :)");
    }
    let mut list_of_failures: Vec<String> = Vec::new();

    // try to rebuild broken packages
    if rebuilds_required && do_auto_rebuild {
        // we need to find out if a package is a git package
        for pkg in broken_pkgs {
            let mut cargo_args: Vec<String> = Vec::new();
            match pkg.git {
                Some(ref git_repo_addr) => {
                    cargo_args.push("--git".to_string());
                    cargo_args.push(git_repo_addr.to_string());
                    // we have a git package, check if it has branch, tag or rev, else install from repo

                    if let Some(ref branch) = pkg.branch {
                        cargo_args.push("--branch".to_string());
                        cargo_args.push(branch.to_string());
                    }

                    if let Some(ref tag) = pkg.tag {
                        cargo_args.push("--tag".to_string());
                        cargo_args.push(tag.to_string());
                    }
                    if let Some(ref rev) = pkg.rev {
                        cargo_args.push("--rev".to_string());
                        cargo_args.push(rev.to_string());
                    }
                } // Some(ref git_repo_addr)
                None => {
                    // normal crates.io package?
                    if let Some(ref registry) = pkg.registry {
                        if registry == "https://github.com/rust-lang/crates.io-index" {
                            // crates io, reinstall the same version
                            cargo_args.push("--version".to_string());
                            cargo_args.push(pkg.version.to_string());
                        } else {
                            eprintln!("error unknown registry!");
                            panic!();
                        }
                    } // match pkg.registry
                      // if we just have a path, there's not much we can do I guess
                    if let Some(ref path) = pkg.path {
                        cargo_args.push("--path".to_string());
                        cargo_args.push(path.to_string());
                    } // match pkg.path
                } // pkg.git == None /// else
            } // match pkg.git

            run_cargo_install(&pkg.name, &cargo_args, &mut list_of_failures);
        }
    }
    if !list_of_failures.is_empty() {
        println!("Failed rebuilds: {}", list_of_failures.join(" "));
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use self::test::Bencher;

    #[test]
    fn empty() {}

    #[bench]
    fn bench_print(b: &mut Bencher) {
        b.iter(|| println!(2));
    }

} // mod test
