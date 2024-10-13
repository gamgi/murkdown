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

pub fn parents<'a, I>(paths: I) -> Result<HashSet<PathBuf>, AppError>
where
    I: Iterator<Item = PathBuf>,
{
    paths
        .map(PathBuf::from)
        .map(|p| match p.parent() {
            Some(parent) if parent == Path::new("") => Ok(p),
            Some(parent) => Ok(parent.to_path_buf()),
            None => Err(AppError::path_error(p)),
        })
        .collect()
}
