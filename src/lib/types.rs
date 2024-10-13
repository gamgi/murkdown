use std::{collections::HashMap, path::PathBuf};

/// Map from identifier to location on disk
pub type LocationMap = HashMap<String, PathBuf>;
