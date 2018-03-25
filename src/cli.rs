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

use clap::{App, Arg, ArgMatches, SubCommand};

pub fn gen_clap<'a>() -> ArgMatches<'a> {
    let auto_rebuild = Arg::with_name("auto-rebuild")
        .short("a")
        .long("auto")
        .help("Try to automatically reinstall broken crates");

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
                .arg(&auto_rebuild),
        ) // subcommand
        .arg(&auto_rebuild)
        .get_matches()
}
