#[derive(Debug, PartialEq)]
pub enum ErrorKind {
    NoCargoHome,       // could not find $CARGO_HOME
    NoCratesToml,      // could not find $CARGO_HOME/.crates.toml
    NoReadCratesToml,  // failed to read .crates.toml
    NotOpenCratesToml, // could not open file
    UnknownAPI,        // api changed, cargo-rebuild-check most likely incompatibe to file format
}
