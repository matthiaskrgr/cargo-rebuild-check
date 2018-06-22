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
#![cfg_attr(feature = "cargo-clippy", warn(needless_borrow))]


#[derive(Debug, PartialEq)]
pub enum ErrorKind {
    NoCargoHome,       // could not find $CARGO_HOME
    NoCratesToml,      // could not find $CARGO_HOME/.crates.toml
    NoReadCratesToml,  // failed to read .crates.toml
    NotOpenCratesToml, // could not open file
    UnknownAPI,        // api changed, cargo-rebuild-check most likely incompatibe to file format
}
