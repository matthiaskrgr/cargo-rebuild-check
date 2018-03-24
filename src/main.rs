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

use std::fs::File;
use std::io::prelude::*;
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
        let mut package = CrateInfo {
            name: String::new(),
            version: String::new(),
            git: None,
            branch: None,
            tag: None,
            rev: None,
            registry: None,
            path: None,
            binaries: vec![],
        };

        let line_split: Vec<&str> = line.split(' ').collect();
        let name = line_split[0].to_string().replace("\"", "");
        let version = line_split[1].to_string();
        let sourceinfo = line_split[2].to_string();
        // sourceinfo tells us if we have a crates registy or git crate and what
        let sourceinfo = sourceinfo.replace("(", "").replace(")", "");
        let sourceinfo_split: Vec<&str> = sourceinfo.split('+').collect();
        let kind = &sourceinfo_split.first();
        let mut addr = &sourceinfo_split.last();
        let mut addr = addr.unwrap().to_string();
        addr.pop(); // remove last char which is "

        package.name = name;
        package.version = version;

        match *kind {
            Some(&"registry") => package.registry = Some(addr),
            Some(&"git") => {
                // cargo-rebuild-check v0.1.0 (https://github.com/matthiaskrgr/cargo-rebuild-check#2ce1ed0b):
                let mut split = addr.split('#');
                let mut repo = split.next().unwrap();
                // rev does not matter unless we have "?rev="
                // cargo-update v1.4.1 (https://github.com/nabijaczleweli/cargo-update/?rev=ab82e070aaf4755fc38d15ca7d58acf4b697731d#ab82e070):
                //
                let has_explicit_rev: bool = repo.contains("?rev=");
                let has_explicit_tag: bool = repo.contains("?tag=");
                let has_explicit_branch: bool = repo.contains("?branch=");

                let should_be_one_at_most =
                    has_explicit_rev as u8 + has_explicit_tag as u8 + has_explicit_branch as u8;
                if should_be_one_at_most > 1 {
                    eprintln!(
                        "Should only have at most one of rev, tag, branch, had: {}",
                        should_be_one_at_most
                    );
                    eprintln!("line was: '{}'", line);
                    eprintln!(
                        "rev: {}, tag: {}, branch: {}",
                        has_explicit_rev, has_explicit_branch, has_explicit_tag
                    );
                    panic!();
                }

                if has_explicit_rev {
                    let explicit_rev = repo.split("?rev=").last().unwrap();
                    package.rev = Some(explicit_rev.to_string());
                } else if has_explicit_tag {
                    let explicit_tag = repo.split("?tag=").last().unwrap();
                    package.tag = Some(explicit_tag.to_string());
                } else if has_explicit_branch {
                    let explicit_branch = repo.split("?branch=").last().unwrap();
                    package.branch = Some(explicit_branch.to_string());
                }
                let repo_url = repo.split('?').nth(0).unwrap();
                package.git = Some(repo_url.to_string());
            }
            Some(&"path") => {
                // try to make the path absolute (file:///home/....  -> /home/....)
                package.path = Some(addr.to_string().replace("file://", "a"));
            }
            Some(&&_) => {
                let string: &str =
                    &format!("Unknown sourceinfo kind '{:?}', please file bug!", kind);
                eprintln!("{}", string);
                panic!();
            }

            None => {
                eprintln!("Failed to parse sourceinfo kind!");
                eprintln!("Sourceinfo: {}", sourceinfo);
                eprintln!("Please file a bug!");
                panic!();
            }
        }

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
            package.binaries.push(binary);
        }

        // collect the packages
        packages.push(package);
    }

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
