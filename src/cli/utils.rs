use std::{
    collections::HashSet,
    path::{Path, PathBuf},
};

use walkdir::DirEntry;

use super::types::AppError;

pub fn is_visible(entry: &DirEntry) -> bool {
    entry
        .file_name()
        .to_str()
        .map(|s| !s.starts_with("."))
        .unwrap_or(true)
}

pub fn is_file(entry: &DirEntry) -> bool {
    entry.path().is_file()
}

pub fn into_uri_path_tuple(entry: DirEntry) -> (String, PathBuf) {
    let id = entry.path().display().to_string();
    (id, entry.path().to_path_buf())
}

pub fn parents<I>(paths: I) -> Result<HashSet<PathBuf>, AppError>
where
    I: Iterator<Item = PathBuf>,
{
    paths
        .map(|p| match p.parent() {
            Some(parent) if parent == Path::new("") => Ok(p),
            Some(parent) => Ok(parent.to_path_buf()),
            None => Err(AppError::bad_path(p)),
        })
        .collect()
}
