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
    let _ = match Command::new(&binary)
        .env("LANG", "en_US")
        .env("LC_ALL", "en_US")
        .output()
    {
        Ok(_ok) => {
            return true;
        }
        Err(_e) => {
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
    #[test]
    fn no_binary_found() {
        use check_external_cmds::*;
        use std::env;
        // clear PATH var
        env::set_var("PATH", "");
        // make sure it is empty
        assert_eq!(env::var("PATH"), Ok("".to_string()));

        let missing_binaries = all_binaries_available();
        // make sure we return that all 3 binaries are missing
        assert_eq!(missing_binaries, Err("ldd rustc cargo".to_string()));
    }
}
