#[cfg(test)]
use test::*;

use std::process::Command;

use rayon::iter::*;

use crate::check_external_cmds::*;
use crate::parse::*;

struct Output {
    stdout: String,
    stderr: String,
}

impl Output {
    fn new() -> Self {
        Self {
            stdout: String::new(),
            stderr: String::new(),
        }
    }
}

pub(crate) fn run_cargo_install<'a>(
    binary: &'a str,
    cargo_args: &[&str],
    list_of_failures: &mut Vec<&'a str>,
) {
    println!("  Reinstalling {}", binary);
    let mut cargo = Command::new("cargo");
    cargo.arg("install");
    cargo.arg(binary);
    cargo.arg("--force");
    for argument in cargo_args {
        // don't pass empty argument to cargo as this used to crash it
        if !argument.is_empty() {
            cargo.arg(argument);
        }
    }

    if let Ok(status) = cargo.status() {
        // bad exit status of cargo, build failed?
        if !status.success() {
            list_of_failures.push(binary);
        }
    } else {
        // maybe cargo crashed?
        list_of_failures.push(binary);
    }
}

fn check_bin_with_ldd(binary_path: &str, rustc_lib_path: &str) -> String {
    // checks a single binary with ldd
    let result = Command::new("ldd")
        .arg(&binary_path)
        .env("LD_LIBRARY_PATH", rustc_lib_path)
        // try to enforce english output to stabilize parsing
        .env("LANG", "en_US")
        .env("LC_ALL", "en_US")
        .output();

    match result {
        Ok(out) => {
            // string is passed to parse_ldd_output()
            String::from_utf8_lossy(&out.stdout).into_owned()
        }
        Err(e) => {
            // something went wrong while running ldd
            eprintln!("Error while running ldd: '{:?}'", e);
            std::process::exit(3);
        }
    }
}

fn parse_ldd_output<'a>(
    output_string: &mut Output,
    ldd_result: &str,
    binary: &str,
    package: &'a CrateInfo,
) -> Option<&'a CrateInfo> {
    // receive the output of ldd, parse it, print information on missing libraries to stderr
    // and mark the crate as outdated if it is

    // assume package is not outdated
    let mut outdated_package: Option<&CrateInfo> = None;

    // we need to know if this is the first missing lib of a binary when making our output string
    let output = ldd_result;

    let mut first_broken_link = true;
    for line in output.lines() {
        // is binary missing a library?
        if line.ends_with("=> not found") {
            if first_broken_link {
                // we found a broken library link, assume package is outdated
                outdated_package = Some(package);
                output_string
                    .stderr
                    .push_str(&format!("    Binary '{}' is missing:\n", &binary));
            }
            output_string.stderr.push_str(&format!(
                "\t\t{}\n",
                line.replace("=> not found", "").trim()
            ));
            first_broken_link = false;
        } // not found
    } // for line in output.lines()

    outdated_package
}

pub(crate) fn check_crate<'a>(
    package: &'a CrateInfo,
    bin_dir: &std::path::PathBuf,
    rustc_lib_path: &str,
    rebuild_all: bool,
) -> Option<&'a CrateInfo> {
    let mut output_string = Output::new();

    output_string.stdout.push_str(&format!(
        "  Checking crate {} {}\n",
        package.name, package.version
    ));

    let mut outdated_package: Option<&CrateInfo> = None;

    for binary in &package.binaries {
        if rebuild_all {
            // rebuild unconditionally
            outdated_package = Some(package);
        } else {
            // fuse together the path to the binary we are going to check and get its String
            let mut bin_path: std::path::PathBuf = bin_dir.clone();
            bin_path.push(&binary);
            let binary_path = bin_path.into_os_string().into_string().unwrap();
            // run ldd on it and check ldds output
            let ldd_result = check_bin_with_ldd(&binary_path, rustc_lib_path);
            outdated_package = parse_ldd_output(&mut output_string, &ldd_result, binary, package);
        }
    }
    // print to stdout/stderr respectively
    // don't print empty lines!
    if !output_string.stdout.is_empty() {
        print!("{}", &output_string.stdout);
    }
    if !output_string.stderr.is_empty() {
        eprint!("{}", &output_string.stderr);
    }
    outdated_package
}

pub(crate) fn get_rustc_lib_path() -> String {
    let rustc = get_rustc();
    let rust_lib_path = match Command::new(&rustc)
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

    rust_lib_path
        .into_os_string()
        .into_string()
        .expect("Failed to convert pathBuf to String")
}

pub(crate) fn check_and_rebuild_broken_crates(
    packages: &[CrateInfo],
    rust_lib_path: &str,
    bin_dir: &std::path::PathBuf,
    do_auto_rebuild: bool,
    rebuild_all: bool,
) {
    // iterate (in parallel) over the acquired metadata and check for broken library links
    // filter out all None values, only collect the Some() ones

    let broken_pkgs: Vec<&CrateInfo> = packages
        .par_iter()
        .filter_map(|crate_data| check_crate(crate_data, bin_dir, rust_lib_path, rebuild_all))
        .collect();

    let rebuilds_required: bool = !broken_pkgs.is_empty();

    if rebuilds_required {
        // concat list of names of crates needing rebuilding
        if rebuild_all {
            println!("\n  Rebuilding all installed crates as requested.");
        } else {
            let pkgs_string = &broken_pkgs
                .iter()
                .map(|pkg| pkg.name.clone())
                .collect::<Vec<_>>()
                .join(" ");

            println!("\n  Crates needing rebuild: {}", pkgs_string);
        }
    } else {
        // if all crates have working links, no need to do anything else
        println!("\n  Everything looks good! :)");
        std::process::exit(0);
    }

    let mut list_of_failures: Vec<&str> = Vec::with_capacity(broken_pkgs.len());
    // try to rebuild broken packages
    if rebuilds_required && (do_auto_rebuild || rebuild_all) {
        // we need to find out if a package is a git package
        for pkg in broken_pkgs {
            // read the line saved in .crates.toml and find out the according "cargo install" flags
            let mut cargo_args: Vec<&str> = Vec::new();
            if let Some(ref git_repo_addr) = pkg.git {
                cargo_args.push("--git");
                cargo_args.push(git_repo_addr);

                // we have a git package, check if it has branch, tag or rev, else install from repo
                if let Some(ref branch) = pkg.branch {
                    cargo_args.push("--branch");
                    cargo_args.push(branch);
                }
                if let Some(ref tag) = pkg.tag {
                    cargo_args.push("--tag");
                    cargo_args.push(tag);
                }
                if let Some(ref rev) = pkg.rev {
                    cargo_args.push("--rev");
                    cargo_args.push(rev);
                }
            } else {
                // normal crates.io package?
                if let Some(ref registry) = pkg.registry {
                    if registry == "https://github.com/rust-lang/crates.io-index" {
                        // crates io, reinstall the same version
                        cargo_args.push("--version");
                        cargo_args.push(&pkg.version);
                    } else {
                        eprintln!("error unknown registry!");
                        panic!();
                    }
                } // match pkg.registry
                  // if we just have a path, there's not much we can do, I guess...
                if let Some(ref path) = pkg.path {
                    cargo_args.push("--path");
                    cargo_args.push(path);
                } // match pkg.path
            } // if let Some(ref git_repo_addr) = pkg.git

            run_cargo_install(&pkg.name, &cargo_args, &mut list_of_failures);
        }
    }
    if !list_of_failures.is_empty() {
        println!("    Failed rebuilds: {}", list_of_failures.join(" "));
        std::process::exit(4);
    }
}

#[cfg(test)]
mod tests {
    use self::test::Bencher;
    use super::*;

    #[test]
    fn package_needs_rebuild() {
        let clippy_line ="\"clippy 0.0.189 (registry+https://github.com/rust-lang/crates.io-index)\" = [\"cargo-clippy\", \"clippy-driver\"]";

        let clippy_crateinfo = decode_line(clippy_line);

        let mut to_be_printed_string = Output::new();
        // clippy-driver
        let ldd_output = "    linux-vdso.so.1 (0x00007ffec37d0000)
    librustc_driver-6516506ab0349d45.so => not found
    librustc_plugin-14c7fbb709ee1764.so => not found
    librustc_typeck-ca6d3c89de970134.so => not found
    librustc-6b0d6e07668228e2.so => not found
    libsyntax-5ece0a81ed6c5461.so => not found
    librustc_errors-7907d589f279528b.so => not found
    libsyntax_pos-610524479a0d36fa.so => not found
    librustc_data_structures-b8a8de55dc5cd1ce.so => not found
    libstd-0cfbe79f10411924.so => not found
    libpthread.so.0 => /usr/lib/libpthread.so.0 (0x00007f2367625000)
    libgcc_s.so.1 => /usr/lib/libgcc_s.so.1 (0x00007f236740e000)
    libc.so.6 => /usr/lib/libc.so.6 (0x00007f2367057000)
    libm.so.6 => /usr/lib/libm.so.6 (0x00007f2366d0b000)
    /lib64/ld-linux-x86-64.so.2 => /usr/lib64/ld-linux-x86-64.so.2 (0x00007f2367c6f000)\n";

        let our_formatted_output = "    Binary 'clippy-driver' is missing:
\t\tlibrustc_driver-6516506ab0349d45.so
\t\tlibrustc_plugin-14c7fbb709ee1764.so
\t\tlibrustc_typeck-ca6d3c89de970134.so
\t\tlibrustc-6b0d6e07668228e2.so
\t\tlibsyntax-5ece0a81ed6c5461.so
\t\tlibrustc_errors-7907d589f279528b.so
\t\tlibsyntax_pos-610524479a0d36fa.so
\t\tlibrustc_data_structures-b8a8de55dc5cd1ce.so
\t\tlibstd-0cfbe79f10411924.so\n";

        let parsed = parse_ldd_output(
            &mut to_be_printed_string,
            ldd_output,
            "clippy-driver",
            &clippy_crateinfo,
        );
        assert!(parsed.is_some());
        let ci = parsed.unwrap();
        // do some sanity checks
        assert_eq!(ci.name, "clippy");
        assert_eq!(ci.git, None,);
        assert_eq!(ci.branch, None);
        assert_eq!(ci.tag, None);
        assert_eq!(ci.rev, None);
        assert_eq!(ci.binaries, vec!["cargo-clippy", "clippy-driver"]);
        //rintln!("str: {}", to_be_printed_string);
        assert_eq!(our_formatted_output, to_be_printed_string.stderr);
    }

    #[test]
    fn package_does_not_need_rebuild() {
        let clippy_line ="\"clippy 0.0.189 (registry+https://github.com/rust-lang/crates.io-index)\" = [\"cargo-clippy\", \"clippy-driver\"]";

        let clippy_crateinfo = decode_line(clippy_line);

        let mut to_be_printed_string = Output::new();
        // clippy-driver
        let ldd_output = "    linux-vdso.so.1 (0x00007ffec37d0000)
librustc_driver-6516506ab0349d45.so => foo.so
librustc_plugin-14c7fbb709ee1764.so => foo.so
librustc_typeck-ca6d3c89de970134.so => foo.so
librustc-6b0d6e07668228e2.so => foo.so
libsyntax-5ece0a81ed6c5461.so => foo.so
librustc_errors-7907d589f279528b.so => foo.so
libsyntax_pos-610524479a0d36fa.so => foo.so
librustc_data_structures-b8a8de55dc5cd1ce.so => foo.so
libstd-0cfbe79f10411924.so => foo.so
libpthread.so.0 => /usr/lib/libpthread.so.0 (0x00007f2367625000)
libgcc_s.so.1 => /usr/lib/libgcc_s.so.1 (0x00007f236740e000)
libc.so.6 => /usr/lib/libc.so.6 (0x00007f2367057000)
libm.so.6 => /usr/lib/libm.so.6 (0x00007f2366d0b000)
/lib64/ld-linux-x86-64.so.2 => /usr/lib64/ld-linux-x86-64.so.2 (0x00007f2367c6f000)\n";

        let parsed = parse_ldd_output(
            &mut to_be_printed_string,
            ldd_output,
            "clippy-driver",
            &clippy_crateinfo,
        );
        assert!(parsed.is_none());
        assert!(to_be_printed_string.stderr.is_empty());
    }

    #[bench]
    fn bench_decode_ldd_output_all_libs_found(b: &mut Bencher) {
        let clippy_line ="\"clippy 0.0.189 (registry+https://github.com/rust-lang/crates.io-index)\" = [\"cargo-clippy\", \"clippy-driver\"]";

        let clippy_crateinfo = decode_line(clippy_line);

        let mut to_be_printed_string = Output::new();
        // clippy-driver
        let ldd_output = "    linux-vdso.so.1 (0x00007ffec37d0000)
librustc_driver-6516506ab0349d45.so => foo.so
librustc_plugin-14c7fbb709ee1764.so => foo.so
librustc_typeck-ca6d3c89de970134.so => foo.so
librustc-6b0d6e07668228e2.so => foo.so
libsyntax-5ece0a81ed6c5461.so => foo.so
librustc_errors-7907d589f279528b.so => foo.so
libsyntax_pos-610524479a0d36fa.so => foo.so
librustc_data_structures-b8a8de55dc5cd1ce.so => foo.so
libstd-0cfbe79f10411924.so => foo.so
libpthread.so.0 => /usr/lib/libpthread.so.0 (0x00007f2367625000)
libgcc_s.so.1 => /usr/lib/libgcc_s.so.1 (0x00007f236740e000)
libc.so.6 => /usr/lib/libc.so.6 (0x00007f2367057000)
libm.so.6 => /usr/lib/libm.so.6 (0x00007f2366d0b000)
/lib64/ld-linux-x86-64.so.2 => /usr/lib64/ld-linux-x86-64.so.2 (0x00007f2367c6f000)\n";

        b.iter(|| {
            parse_ldd_output(
                &mut to_be_printed_string,
                ldd_output,
                "clippy-driver",
                &clippy_crateinfo,
            )
        });
    }

    #[bench]
    fn bench_decode_ldd_output_some_libs_not_found(b: &mut Bencher) {
        let clippy_line ="\"clippy 0.0.189 (registry+https://github.com/rust-lang/crates.io-index)\" = [\"cargo-clippy\", \"clippy-driver\"]";

        let clippy_crateinfo = decode_line(clippy_line);

        let mut to_be_printed_string = Output::new();
        // clippy-driver
        let ldd_output = "    linux-vdso.so.1 (0x00007ffec37d0000)
            librustc_driver-6516506ab0349d45.so => not found
            librustc_plugin-14c7fbb709ee1764.so => not found
            librustc_typeck-ca6d3c89de970134.so => not found
            librustc-6b0d6e07668228e2.so => not found
            libsyntax-5ece0a81ed6c5461.so => not found
            librustc_errors-7907d589f279528b.so => not found
            libsyntax_pos-610524479a0d36fa.so => not found
            librustc_data_structures-b8a8de55dc5cd1ce.so => not found
            libstd-0cfbe79f10411924.so => not found
            libpthread.so.0 => /usr/lib/libpthread.so.0 (0x00007f2367625000)
            libgcc_s.so.1 => /usr/lib/libgcc_s.so.1 (0x00007f236740e000)
            libc.so.6 => /usr/lib/libc.so.6 (0x00007f2367057000)
            libm.so.6 => /usr/lib/libm.so.6 (0x00007f2366d0b000)
            /lib64/ld-linux-x86-64.so.2 => /usr/lib64/ld-linux-x86-64.so.2 (0x00007f2367c6f000)\n";

        b.iter(|| {
            parse_ldd_output(
                &mut to_be_printed_string,
                ldd_output,
                "clippy-driver",
                &clippy_crateinfo,
            )
        });
    }

} // mod test
