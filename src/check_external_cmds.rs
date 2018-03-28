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

use std::process::Command;
use std::string::String;

fn has_binary(binary: &str) -> bool {
    // we need to grab the output so it does not spam in to program stdout
    // check if we can find the binary
    // @TODO can we check that we don't get any output in stdout/stderr from this function?
    let _ = match Command::new(&binary)
        .env("LANG", "en_US")
        .env("LC_ALL", "en_US")
        .output()
    {
        Ok(_) => {
            return true;
        }
        Err(_) => {
            return false;
        }
    };
}

pub fn all_binaries_available() -> Result<bool, String> {
    // we need ldd, rustc and cargo
    let mut missing_bins = String::new();
    if !has_binary("ldd") {
        missing_bins.push_str("ldd");
    }
    if !has_binary("rustc") {
        missing_bins.push_str(" rustc");
    }
    if !has_binary("cargo") {
        missing_bins.push_str(" cargo");
    }
    missing_bins.trim();

    if missing_bins.is_empty() {
        Ok(true)
    } else {
        Err(missing_bins)
    }
}

#[cfg(test)]
mod tests {
    use std;
    use std::process::Command;
    #[test]
    fn no_binary_found() {
        // do this similar to test_help
        // run a "cargo build", execute the binary with the PATH env var cleared
        // to prevent finding any binaries
        // then assert that we get a warning as output
        let mut dir = std::env::current_dir().unwrap();
        let cargo_cmd = Command::new("cargo")
            .arg("build")
            .current_dir(&dir)
            .output();
        // cargo build is ok
        assert!(cargo_cmd.unwrap().status.success());

        dir.push("target");
        dir.push("debug");
        let crc_cmd = Command::new("cargo-rebuild-check")
            .current_dir(&dir)
            .env("PATH", "")
            .env("LANG", "en_US")
            .env("LC_ALL", "en_US")
            .output()
            .unwrap();
        // assert that we failed
        assert!(!crc_cmd.status.success());
        assert!(crc_cmd.stdout.is_empty());
        let output = String::from_utf8_lossy(&crc_cmd.stderr);
        let error_msg = "Could not find the following binaries: 'ldd rustc cargo'
Please make them available in your $PATH.\n";
        assert_eq!(output, error_msg);
    }
}
