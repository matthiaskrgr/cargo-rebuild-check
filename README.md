# cargo-rebuild-check

Checks installed crate binaries for missing library links.

Requires ````ldd````, ````cargo```` and ````rustc```` to be installed.

## Install or Update

````cargo install --git https://github.com/matthiaskrgr/cargo-rebuild-check --force````

## Usage

Use ````cargo rebuild-check --auto```` to automatically rebuild and reinstall crates
against current versions of libraries.

Use ````cargo rebuild-check --rebuild-all```` tries to reinstall all crates unconditionally.

## Sample output

````
  Checking crate alacritty 0.1.0
  Checking crate ripgrep 0.8.0
  Checking crate clippy 0.0.189
    Binary 'clippy-driver' is missing:
                librustc_driver-d5cac83e5c5b550f.so
                librustc_plugin-5faf8b922dc8abb4.so
                librustc_typeck-c5a675d2e1c198c7.so
                librustc-120adce04c19a52b.so
                libsyntax-1c14591008350f74.so
                librustc_errors-d9b9551e9c964ec8.so
                libsyntax_pos-f986bb0ca2284a57.so
                librustc_data_structures-1cbce7698121b6bf.so
                libstd-183b70a6dbaa3f1a.so
  Checking crate cargo-asm 0.1.11
  Checking crate cargo-modules 0.3.6
    Binary 'cargo-modules' is missing:
                libsyntax-1c14591008350f74.so
                librustc_errors-d9b9551e9c964ec8.so
                libsyntax_pos-f986bb0ca2284a57.so
                libstd-183b70a6dbaa3f1a.so
  Checking crate rustup-toolchain-install-master 0.1.0
    Crates needing rebuild: cargo-modules clippy
  ````
