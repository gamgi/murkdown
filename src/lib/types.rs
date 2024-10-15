use std::{collections::HashMap, path::PathBuf};

/// Uniform Resource Identifier (eg. load:foo.fd)
pub type URI = String;
/// Map from Resource Name (eg. foo.fd) to location on disk
pub type LocationMap = HashMap<String, PathBuf>;
