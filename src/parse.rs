#[cfg(test)]
use test::*;

use std::fs::File;
use std::io::prelude::*;

use crate::errors::*;

// a package that we may need to rebuild
#[derive(Debug)]
pub(crate) struct CrateInfo {
    pub(crate) name: String,
    pub(crate) version: String,
    pub(crate) git: Option<String>,
    pub(crate) branch: Option<String>,
    pub(crate) tag: Option<String>,
    pub(crate) rev: Option<String>,
    pub(crate) registry: Option<String>,
    pub(crate) path: Option<String>,
    pub(crate) binaries: Vec<String>,
}

pub(crate) fn read_crates_toml() -> Result<String, ErrorKind> {
    let cargo_cfg = match cargo::util::config::Config::default() {
        Ok(cargo_cfg) => cargo_cfg,
        Err(e) => {
            eprintln!("{}", e);
            return Err(ErrorKind::NoCargoHome);
        }
    };

    let mut crates_index = cargo_cfg.home().clone();
    crates_index.push(".crates.toml");
    let crates_toml_path = crates_index.into_path_unlocked();

    if !crates_toml_path.is_file() {
        eprintln!("No .crates.toml");
        return Err(ErrorKind::NoCratesToml);
    }

    let mut f = match File::open(crates_toml_path) {
        Ok(f) => f,
        Err(e) => {
            eprintln!("{}", e);
            return Err(ErrorKind::NotOpenCratesToml);
        }
    };

    let mut file_content = String::new();
    match f.read_to_string(&mut file_content) {
        Ok(_) => {}
        Err(e) => {
            eprintln!("{}", e);
            return Err(ErrorKind::NoReadCratesToml);
        }
    }
    Ok(file_content)
}

pub(crate) fn get_installed_crate_information(
    file_content: Result<String, ErrorKind>,
) -> Result<Vec<CrateInfo>, ErrorKind> {
    let file = file_content.unwrap();

    let mut file_lines = file.lines();
    // skip the first line when unwrapping
    // the first line also tells the api version, so assert that we are sort of compatible
    let first_line = file_lines.next().unwrap().trim();
    if first_line != "[v1]" {
        eprintln!("Error: API changed!");
        return Err(ErrorKind::UnknownAPI);
    }

    let packages = file_lines
        .map(|line| decode_line(line))
        .collect::<Vec<CrateInfo>>();
    Ok(packages)
}

pub(crate) fn decode_line(line: &str) -> self::CrateInfo {
    let mut package = CrateInfo {
        name: String::new(),
        version: String::new(),
        git: None,
        branch: None,
        tag: None,
        rev: None,
        registry: None,
        path: None,
        binaries: Vec::new(),
    };

    let mut line_split = line.split_whitespace();
    let name = line_split.next().unwrap().to_string().replace("\"", "");
    let version = line_split.next().unwrap();
    let sourceinfo = line_split.next().unwrap();
    // sourceinfo tells us if we have a crates registy or git crate
    let sourceinfo = sourceinfo.replace("(", "").replace(")", "");
    let mut sourceinfo_split = sourceinfo.split('+');
    let kind = &sourceinfo_split.next();
    let addr = &sourceinfo_split.last();
    let mut addr = addr.unwrap().to_string();
    addr.pop(); // remove last char which is \"

    package.name = name;
    package.version = version.to_string();

    match *kind {
        Some("registry") => package.registry = Some(addr),
        Some("git") => {
            // cargo-rebuild-check v0.1.0 (https://github.com/matthiaskrgr/cargo-rebuild-check#2ce1ed0b):
            let mut split = addr.split('#');
            let repo = split.next().unwrap();
            // rev does not matter unless we have "?rev="
            // cargo-update v1.4.1 (https://github.com/nabijaczleweli/cargo-update/?rev=ab82e070aaf4755fc38d15ca7d58acf4b697731d#ab82e070):

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
        Some("path") => {
            // try to make the path absolute (file:///home/....  -> /home/....)
            package.path = Some(addr.to_string().replace("file://", ""));
        }
        Some(&_) => {
            let string: &str = &format!(
                "Unknown sourceinfo kind '{:?}', please file bug ticket!",
                kind
            );
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
    let bins_split_from_line = line.split('=');
    let bins = bins_split_from_line.last().unwrap();
    // clean up, remove characters remaining from toml encoding
    bins.split(',').for_each(|bin| {
        package.binaries.push(
            bin.replace("[", "")
                .replace("]", "")
                .replace("\"", "")
                .trim()
                .to_string(),
        )
    });

    package
}

#[cfg(test)]
mod tests {
    use self::test::Bencher;
    use super::*;

    #[test]
    fn decode_line_git_simple() {
        let line = "\"cargo-cache 0.1.0 (git+http://github.com/matthiaskrgr/cargo-cache#6083f409343aeb8c7fcedd1877fd1ae4ef8c9e49)\" = [\"cargo-cache\"]";
        let ci = decode_line(line); // crate info obj
        assert_eq!(ci.name, "cargo-cache");
        assert_eq!(ci.version, "0.1.0");
        assert_eq!(
            ci.git,
            Some("http://github.com/matthiaskrgr/cargo-cache".to_string())
        );
        assert_eq!(ci.branch, None);
        assert_eq!(ci.tag, None);
        assert_eq!(ci.rev, None);
        assert_eq!(ci.registry, None);
        assert_eq!(ci.path, None);
        assert_eq!(ci.binaries, vec!["cargo-cache"]);
    }

    #[test]
    fn decode_line_git_branch() {
        let line = "\"alacritty 0.1.0 (git+https://github.com/jwilm/alacritty/?branch=scrollback#9ee1cf2455d5512b087757e09451f9d122548da2)\" = [\"alacritty\"]";
        let ci = decode_line(line);
        assert_eq!(ci.name, "alacritty");
        assert_eq!(ci.version, "0.1.0");
        assert_eq!(
            ci.git,
            Some("https://github.com/jwilm/alacritty/".to_string())
        );
        assert_eq!(ci.branch, Some("scrollback".to_string()));
        assert_eq!(ci.tag, None);
        assert_eq!(ci.rev, None);
        assert_eq!(ci.registry, None);
        assert_eq!(ci.path, None);
        assert_eq!(ci.binaries, vec!["alacritty"]);
    }

    #[test]
    fn decode_line_git_tag() {
        let line = "\"ripgrep 0.8.0 (git+https://github.com/BurntSushi/ripgrep?tag=0.8.0#23d1b91eaddbfb886a3a99d615f49551cd35cb6c)\" = [\"rg\"]";
        let ci = decode_line(line);
        assert_eq!(ci.name, "ripgrep");
        assert_eq!(ci.version, "0.8.0");
        assert_eq!(
            ci.git,
            Some("https://github.com/BurntSushi/ripgrep".to_string())
        );
        assert_eq!(ci.branch, None);
        assert_eq!(ci.tag, Some("0.8.0".to_string()));
        assert_eq!(ci.rev, None);
        assert_eq!(ci.registry, None);
        assert_eq!(ci.path, None);
        assert_eq!(ci.binaries, vec!["rg"]);
    }

    #[test]
    fn decode_line_git_rev() {
        let line = "\"cargo-rebuild-check 0.1.0 (git+https://github.com/matthiaskrgr/cargo-rebuild-check?rev=37a364d852f697612fede36546cbb31ef0265f08#37a364d852f697612fede36546cbb31ef0265f08)\" = [\"cargo-rebuild-check\"]";
        let ci = decode_line(line);
        assert_eq!(ci.name, "cargo-rebuild-check");
        assert_eq!(ci.version, "0.1.0");
        assert_eq!(
            ci.git,
            Some("https://github.com/matthiaskrgr/cargo-rebuild-check".to_string())
        );
        assert_eq!(ci.branch, None);
        assert_eq!(ci.tag, None);
        assert_eq!(
            ci.rev,
            Some("37a364d852f697612fede36546cbb31ef0265f08".to_string())
        );
        assert_eq!(ci.registry, None);
        assert_eq!(ci.path, None);
        assert_eq!(ci.binaries, vec!["cargo-rebuild-check"]);
    }
    #[test]
    fn decode_line_registry() {
        let line = "\"mdbook 0.1.5 (registry+https://github.com/rust-lang/crates.io-index)\" = [\"mdbook\"]";
        let ci = decode_line(line);
        assert_eq!(ci.name, "mdbook");
        assert_eq!(ci.version, "0.1.5");
        assert_eq!(ci.git, None,);
        assert_eq!(ci.branch, None);
        assert_eq!(ci.tag, None);
        assert_eq!(ci.rev, None);
        assert_eq!(
            ci.registry,
            Some("https://github.com/rust-lang/crates.io-index".to_string())
        );
        assert_eq!(ci.path, None);
        assert_eq!(ci.binaries, vec!["mdbook"]);
    }

    #[test]
    fn decode_line_path() {
        let line = "\"racer 2.0.12 (path+file:///tmp/racer)\" = [\"racer\"]";
        let ci = decode_line(line);
        assert_eq!(ci.name, "racer");
        assert_eq!(ci.version, "2.0.12");
        assert_eq!(ci.git, None,);
        assert_eq!(ci.branch, None);
        assert_eq!(ci.tag, None);
        assert_eq!(ci.rev, None);
        assert_eq!(ci.registry, None);
        assert_eq!(ci.path, Some("/tmp/racer".to_string()));
        assert_eq!(ci.binaries, vec!["racer"]);
    }

    #[test]
    fn decode_line_multiple_binaries() {
        let line = "\"rustfmt-nightly 0.4.1 (registry+https://github.com/rust-lang/crates.io-index)\" = [\"cargo-fmt\", \"git-rustfmt\", \"rustfmt\", \"rustfmt-format-diff\"]";
        let ci = decode_line(line);
        assert_eq!(ci.name, "rustfmt-nightly");
        assert_eq!(ci.version, "0.4.1");
        assert_eq!(ci.git, None,);
        assert_eq!(ci.branch, None);
        assert_eq!(ci.tag, None);
        assert_eq!(ci.rev, None);
        assert_eq!(
            ci.registry,
            Some("https://github.com/rust-lang/crates.io-index".to_string())
        );
        assert_eq!(ci.path, None);
        assert_eq!(
            ci.binaries,
            vec!["cargo-fmt", "git-rustfmt", "rustfmt", "rustfmt-format-diff"]
        );
    }

    #[test]
    fn check_failure_on_invalid_api() {
        let file_content = "[v2]
\"afl 0.3.2 (registry+https://github.com/rust-lang/crates.io-index)\" = [\"cargo-afl\"]"
            .to_string();
        let file_result = Ok(file_content);
        let parsed = get_installed_crate_information(file_result);
        assert!(parsed.is_err());
        match parsed {
            Err(e) => {
                assert_eq!(e, ErrorKind::UnknownAPI);
            }
            _ => unreachable!(),
        }
    }
    #[test]
    fn check_success_on_valid_api() {
        let file_content = "[v1]
\"afl 0.3.2 (registry+https://github.com/rust-lang/crates.io-index)\" = [\"cargo-afl\"]"
            .to_string();
        let file_result = Ok(file_content);
        let parsed = get_installed_crate_information(file_result);
        assert!(parsed.is_ok());
        let parsed = parsed.unwrap();
        assert_eq!(parsed.len(), 1);
        let pkg = &parsed.first().unwrap();
        assert_eq!(pkg.name, "afl");
        assert_eq!(pkg.version, "0.3.2");
    }

    #[bench]
    fn bench_decode_line_git_simple(b: &mut Bencher) {
        let line = "\"cargo-cache 0.1.0 (git+http://github.com/matthiaskrgr/cargo-cache#6083f409343aeb8c7fcedd1877fd1ae4ef8c9e49)\" = [\"cargo-cache\"]";
        b.iter(|| decode_line(line))
    }

    #[bench]
    fn bench_decode_line_git_tag(b: &mut Bencher) {
        let line = "\"ripgrep 0.8.0 (git+https://github.com/BurntSushi/ripgrep?tag=0.8.0#23d1b91eaddbfb886a3a99d615f49551cd35cb6c)\" = [\"rg\"]";
        b.iter(|| decode_line(line))
    }

    #[bench]
    fn bench_decode_line_git_branch(b: &mut Bencher) {
        let line = "\"alacritty 0.1.0 (git+https://github.com/jwilm/alacritty/?branch=scrollback#9ee1cf2455d5512b087757e09451f9d122548da2)\" = [\"alacritty\"]";

        b.iter(|| decode_line(line))
    }

    #[bench]
    fn bench_decode_line_git_rev(b: &mut Bencher) {
        let line = "\"cargo-rebuild-check 0.1.0 (git+https://github.com/matthiaskrgr/cargo-rebuild-check?rev=37a364d852f697612fede36546cbb31ef0265f08#37a364d852f697612fede36546cbb31ef0265f08)\" = [\"cargo-rebuild-check\"]";
        b.iter(|| decode_line(line))
    }

    #[bench]
    fn bench_decode_line_registry(b: &mut Bencher) {
        let line = "\"mdbook 0.1.5 (registry+https://github.com/rust-lang/crates.io-index)\" = [\"mdbook\"]";
        b.iter(|| decode_line(line))
    }

    #[bench]
    fn bench_decode_line_path(b: &mut Bencher) {
        let line = "\"racer 2.0.12 (path+file:///tmp/racer)\" = [\"racer\"]";
        b.iter(|| decode_line(line))
    }

    #[bench]
    fn bench_decode_line_multiple_binaries(b: &mut Bencher) {
        let line = "\"rustfmt-nightly 0.4.1 (registry+https://github.com/rust-lang/crates.io-index)\" = [\"cargo-fmt\", \"git-rustfmt\", \"rustfmt\", \"rustfmt-format-diff\"]";
        b.iter(|| decode_line(line))
    }

} // mod test
