use clap::{crate_version, App, AppSettings, Arg, ArgMatches, SubCommand};

pub(crate) fn gen_clap<'a>() -> ArgMatches<'a> {
    let auto_rebuild = Arg::with_name("auto-rebuild")
        .short("a")
        .long("auto")
        .help("Try to automatically reinstall broken crates");

    let rebuild_all = Arg::with_name("rebuild-all")
        .short("r")
        .long("rebuild-all")
        .help("Rebuild all installed crates unconditionally");

    App::new("cargo-rebuild-check")
        .version(crate_version!())
        .bin_name("cargo")
        .about("Find installed crates that need rebuild due to broken library links")
        .author("matthiaskrgr")
        .subcommand(
            SubCommand::with_name("rebuild-check")
                .version(crate_version!())
                .bin_name("cargo-rebuild-check")
                .about("Find installed crates that need rebuild due to broken library links")
                .author("matthiaskrgr")
                .arg(&auto_rebuild)
                .arg(&rebuild_all)
                .setting(AppSettings::Hidden) // hide subcommand from --help
        ) // subcommand
        .arg(&auto_rebuild)
        .arg(&rebuild_all)
        .get_matches()
}

#[cfg(test)]
mod tests {
    use std::process::Command;
    #[test]
    fn test_help() {
        // this is a hack
        // we launch "cargo build", build the crate and when running the executable
        // make sure we get the desired output
        let mut dir = std::env::current_dir().unwrap();
        //println!("dir: {:?}", );
        let cargo_cmd = Command::new("cargo")
            .arg("build")
            .current_dir(&dir)
            .output();
        // cargo build is ok
        assert!(cargo_cmd.unwrap().status.success());

        dir.push("target");
        dir.push("debug");
        let crc_cmd = Command::new("./cargo-rebuild-check")
            .arg("--help")
            .current_dir(&dir)
            .env("LANG", "en_US")
            .env("LC_ALL", "en_US")
            .output()
            .unwrap();

        assert!(crc_cmd.status.success());
        assert!(crc_cmd.stderr.is_empty());
        let output = String::from_utf8_lossy(&crc_cmd.stdout);
        let help_text = "cargo-rebuild-check 0.1.0
matthiaskrgr
Find installed crates that need rebuild due to broken library links\n
USAGE:
    cargo [FLAGS]\n
FLAGS:
    -a, --auto           Try to automatically reinstall broken crates
    -h, --help           Prints help information
    -r, --rebuild-all    Rebuild all installed crates unconditionally
    -V, --version        Prints version information\n";
        assert_eq!(output, help_text);
    }

} // mod test
