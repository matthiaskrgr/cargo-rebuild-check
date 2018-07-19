use std::env;
use std::process::Command;
use std::string::String;

fn has_binary(binary: &str) -> bool {
    // check if we can find the binary
    Command::new(&binary)
        .env("LANG", "en_US")
        .env("LC_ALL", "en_US")
        .output()
        .is_ok()
}

pub(crate) fn get_rustc() -> String {
    match env::var_os("RUSTC") {
        Some(rustc) => rustc.into_string().unwrap(),
        None => String::from("rustc"),
    }
}

pub(crate) fn all_binaries_available() -> Result<bool, String> {
    // we need ldd, rustc and cargo
    let mut missing_bins = String::new();
    if !has_binary("ldd") {
        missing_bins.push_str("ldd");
    }

    let rustc = get_rustc();

    if !has_binary(&rustc) {
        missing_bins.push_str(" rustc");
    }
    if !has_binary("cargo") {
        missing_bins.push_str(" cargo");
    }
    // remove excess whitespaces
    missing_bins.trim();

    if missing_bins.is_empty() {
        Ok(true)
    } else {
        Err(missing_bins)
    }
}

#[cfg(test)]
mod tests {
    use crate::test_helpers::*;
    use std::process::Command;
    #[test]
    fn no_binary_found() {
        let mut dir = run_cargo_build();
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
