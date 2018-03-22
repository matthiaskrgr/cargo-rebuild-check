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
#[macro_use]
extern crate clap;
extern crate rayon;

mod check_external_cmds;

use std::fs::File;
use std::io::prelude::*;
use std::process::Command;

use rayon::prelude::*;
use clap::{App, Arg, SubCommand};

use check_external_cmds::*;

// deserialize the ~/.cargo/.crates.toml

// a package that we may need to rebuild
#[derive(Debug)]
struct CrateInfo {
    name: String,
    version: String,
    git: Option<String>,
    branch: Option<String>,
    tag: Option<String>,
    rev: Option<String>,
    registry: Option<String>,
    path: Option<String>,
    binaries: Vec<String>,
}

fn main() {
    assert_lld_is_available();

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
            }
            Some(&"path") => {
                package.path = Some(addr.to_string());
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

    if rebuilds_required {
        // concat list of names of crates needing rebuilding
        let mut pkgs_string = String::new();
        for pkg in &broken_pkgs {
            pkgs_string.push_str(&pkg.name);
            pkgs_string.push_str(" ");
        }
        println!("\n  Crates needing rebuild: {}", pkgs_string.trim());
        std::process::exit(2);
    } else {
        println!("\n  Everything looks good.");
    }

    // try to rebuild broken packages
    if rebuilds_required && cfg.is_present("auto-rebuild") {
        // we need to find out if a package is a git package
        for pkg in broken_pkgs {
            println!("WIP Reinstalling {:?}", pkg);
        }
    }
}

fn check_binary<'a>(
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

fn gen_clap<'a>() -> clap::ArgMatches<'a> {
    App::new("cargo-rebuild-check")
        .version(crate_version!())
        .bin_name("cargo")
        .about("find installed crates that need rebuild due to broken library links")
        .author("matthiaskrgr")
        .subcommand(
            SubCommand::with_name("rebuild-check")
            .version(crate_version!())
            .bin_name("cargo-rebuild-check")
            .about("find installed crates that need rebuild due to broken library links")
            .author("matthiaskrgr")
            .arg(
                Arg::with_name("auto-rebuild")
                .short("a")
                .long("auto")
                .help("Try to automatically reinstall broken crates"),
            ) // arg
        ) // subcommand
        .arg(
            Arg::with_name("auto-rebuild")
                .short("a")
                .long("auto")
                .help("Try to automatically reinstall broken crates"),
        )
        .get_matches()
}
