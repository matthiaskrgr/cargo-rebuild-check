use std::path::PathBuf;
use std::process::Command;

pub(crate) fn run_cargo_build() -> PathBuf {
    // run a "cargo build, then we can run the resulting binary and test things

    let dir = std::env::current_dir().unwrap();
    let cargo_cmd = Command::new("cargo")
        .arg("build")
        .current_dir(&dir)
        .output();
    // cargo build is ok
    assert!(cargo_cmd.unwrap().status.success());
    // return the directory since we are going to reuse it
    dir
}
