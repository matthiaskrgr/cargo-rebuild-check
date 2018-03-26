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

use std::fs::File;
use std::io::prelude::*;

// a package that we may need to rebuild
pub struct CrateInfo {
    pub name: String,
    pub version: String,
    pub git: Option<String>,
    pub branch: Option<String>,
    pub tag: Option<String>,
    pub rev: Option<String>,
    pub registry: Option<String>,
    pub path: Option<String>,
    pub binaries: Vec<String>,
}

pub fn get_installed_crate_information() -> Vec<CrateInfo> {
    let cargo_cfg = cargo::util::config::Config::default().unwrap();

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
        let package = decode_line(&line);
        // collect the packages
        packages.push(package);
    } // for line in file_iter
      // done reading in the file

    packages
}

pub fn decode_line(line: &str) -> self::CrateInfo {
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
    let addr = &sourceinfo_split.last();
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
            let string: &str = &format!("Unknown sourceinfo kind '{:?}', please file bug!", kind);
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
    package
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

    #[bench]
    fn bench_line_simple(b: &mut Bencher) {
        let line = "\"gitgc 0.1.0 (git+https://github.com/matthiaskrgr/gitgc#038fbf2cbfb0120cb2cb7960f713e3d80aa0b95f)\" = [\"gitgc\"]";
        b.iter(|| decode_line(line))
    }

    #[bench]
    fn bench_line_binaries(b: &mut Bencher) {
        let line = "\"rustfmt-nightly 0.4.1 (registry+https://github.com/rust-lang/crates.io-index)\" = [\"cargo-fmt\", \"git-rustfmt\", \"rustfmt\", \"rustfmt-format-diff\"]";
        b.iter(|| decode_line(line))
    }
} // mod te
