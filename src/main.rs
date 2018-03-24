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
#[macro_use]
extern crate clap;
extern crate rayon;

mod check_external_cmds;
mod cli;
mod core;

use std::process::Command;

use rayon::prelude::*;

use check_external_cmds::*;
use cli::*;
use core::*;

// deserialize the ~/.cargo/.crates.toml

fn main() {
    match all_binaries_available() {
        Ok(_ok) => {}
        Err(missing_bins) => {
            eprintln!("Could not find the following binaries: '{}'", missing_bins);
            eprintln!("Please make them available in your $PATH.");
            std::process::exit(1);
        }
    }

    let cfg = gen_clap();
    // we need this in case we call "cargo-rebuild-check" directly
    let cfg = cfg.subcommand_matches("rebuild-check").unwrap_or(&cfg);

    let cargo_cfg = cargo::util::config::Config::default().unwrap();
    let mut bin_dir = cargo_cfg.home().clone().into_path_unlocked();
    bin_dir.push("bin");

    // get vector of packages from parsed .crates.toml file
    let packages = get_installed_crate_information();

    // get the path where rustc libs are stored: $(rustc --print sysroot)/lib
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

    // iterate (in parallel) over the acquired metadata and check for broken library links
    // filter out all None values, only collect the Some() ones

    // todo: can we avoid sorting into a separate vector here?
    let broken_pkgs: Vec<&CrateInfo> = packages
        .par_iter()
        .filter_map(|binary| check_binary(binary, &bin_dir, &rust_lib_path_string))
        .collect();

    let rebuilds_required: bool = !broken_pkgs.is_empty();
    let do_auto_rebuild = cfg.is_present("auto-rebuild");

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
