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

extern crate test;

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

#[cfg(test)]
mod tests {
    use std;
    use std::process::Command;
    #[test]
    fn test_help() {
        // this is a hack
        // we launch "cargo build", build the crate and when running the executable
        // make sure we get the desired output

        let mut dir = std::env::current_dir().unwrap();
        println!("dir: {:?}", dir);
        let cargo_cmd = Command::new("cargo")
            .arg("build")
            .current_dir(&dir)
            .output();
        // cargo build is ok
        assert!(cargo_cmd.unwrap().status.success());

        dir.push("target");
        dir.push("debug");
        let crc_cmd = Command::new("cargo-rebuild-check")
            .arg("--help")
            .current_dir(&dir)
            .output()
            .unwrap();

        assert!(crc_cmd.status.success());
        assert!(crc_cmd.stderr.is_empty());
        let output = String::from_utf8_lossy(&crc_cmd.stdout);
        let help_text = "cargo-rebuild-check 0.1.0
matthiaskrgr
find installed crates that need rebuild due to broken library links\n
USAGE:
    cargo [FLAGS] [SUBCOMMAND]\n
FLAGS:
    -a, --auto       Try to automatically reinstall broken crates
    -h, --help       Prints help information
    -V, --version    Prints version information\n
SUBCOMMANDS:
    help             Prints this message or the help of the given subcommand(s)
    rebuild-check    find installed crates that need rebuild due to broken library links\n";
        assert_eq!(output, help_text);
    }

} // mod test
