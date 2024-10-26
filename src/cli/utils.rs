use std::{
    collections::HashSet,
    path::{Path, PathBuf},
};

use walkdir::DirEntry;

use super::types::{AppError, AppErrorKind};

pub fn is_visible(entry: &DirEntry) -> bool {
    entry
        .file_name()
        .to_str()
        .map(|s| !s.starts_with(".") || s.starts_with("./") || s == ".")
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
            Some(parent) if parent == Path::new("") => Ok(PathBuf::from(".")),
            Some(parent) => Ok(parent.to_path_buf()),
            None => Err(AppError::bad_path(p)),
        })
        .collect()
}

pub fn handle_exit(err: AppError) -> Result<(), AppError> {
    match err.inner() {
        AppErrorKind::Exit(0) => Ok(()),
        _ => Err(err),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parents() {
        let result = parents(
            [
                PathBuf::from("./foo.md"),
                PathBuf::from("./bar/bar.md"),
                PathBuf::from("./"),
            ]
            .into_iter(),
        )
        .unwrap();
        let expected = [PathBuf::from("./"), PathBuf::from("./bar")].into();

        assert_eq!(&result, &expected);
    }
}
