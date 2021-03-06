#![feature(rust_2018_preview, use_extern_macros)]
#![feature(test)]
// these [allow()] by default, make them warn:
#![warn(
    ellipsis_inclusive_range_patterns,
    single_use_lifetimes,
    trivial_casts,
    trivial_numeric_casts,
    unreachable_pub,
    unsafe_code,
    unused,
    rust_2018_idioms
)]
// enable additional clippy warnings
#![cfg_attr(
    feature = "cargo-clippy",
    warn(
        clippy,
        clippy_correctness,
        clippy_perf,
        clippy_complexity,
        clippy_style,
        clippy_pedantic,
        clippy_nursery
    )
)]
//#![cfg_attr(feature = "cargo-clippy", warn(clippy_cargo))]
// additional warnings from "cippy_restriction" group
#![cfg_attr(
    feature = "cargo-clippy",
    warn(shadow_reuse, shadow_same, shadow_unrelated)
)]
#![cfg_attr(feature = "cargo-clippy", warn(pub_enum_variant_names))]
#![cfg_attr(
    feature = "cargo-clippy",
    warn(string_add, string_add_assign)
)]
#![cfg_attr(feature = "cargo-clippy", warn(needless_borrow))]

mod check_external_cmds;
mod cli;
mod core;
mod errors;
mod parse;
#[cfg(test)]
mod test_helpers;

use crate::check_external_cmds::*;
use crate::cli::*;
use crate::core::*;
use crate::parse::*;

// deserialize the ~/.cargo/.crates.toml

fn main() {
    match all_binaries_available() {
        Ok(_) => {}
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

    let file = read_crates_toml();
    let packages = match get_installed_crate_information(file) {
        Ok(pkgs) => pkgs,
        Err(error) => if let errors::ErrorKind::UnknownAPI = error {
            std::process::exit(2);
        } else {
            eprintln!("bad error: {:?}", error);
            std::process::exit(3);
        },
    };

    // get the path where rustc libs are stored: $(rustc --print sysroot)/lib
    let rust_lib_path = get_rustc_lib_path();

    check_and_rebuild_broken_crates(
        &packages,
        &rust_lib_path,
        &bin_dir,
        cfg.is_present("auto-rebuild"),
        cfg.is_present("rebuild-all"),
    )
}
