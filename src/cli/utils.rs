use std::{
    collections::HashSet,
    path::{Path, PathBuf},
    process::{Output, Stdio},
};

use tokio::{
    io::AsyncWriteExt,
    process::{Child, Command},
};
use walkdir::DirEntry;

use super::types::{AppError, Source};

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
    let path = entry.path().to_path_buf();
    let id = path
        .strip_prefix("./")
        .unwrap_or(&path)
        .display()
        .to_string();
    (id, path)
}

pub fn into_id_source_tuple(entry: DirEntry) -> (String, Source) {
    let path = entry.path().to_path_buf();
    let id = path
        .strip_prefix("./")
        .unwrap_or(&path)
        .display()
        .to_string();
    (id, Source::from(path))
}

pub fn spawn_command(program: &str, args: &str) -> Result<Child, AppError> {
    let args = shlex::split(args).ok_or(AppError::bad_exec_args(program, args))?;

    Command::new(program)
        .args(args)
        .stdout(Stdio::piped())
        .stdin(Stdio::piped())
        .kill_on_drop(true)
        .spawn()
        .map_err(|e| AppError::execution_io_failed(e, program))
}

pub async fn write_command(
    child: &mut Child,
    stdin: Option<&str>,
    name: &str,
) -> Result<(), AppError> {
    if let Some(input) = stdin {
        let mut stdin = child.stdin.take().ok_or(AppError::execution_failed(
            "could not take stdin",
            name.to_string(),
        ))?;

        stdin
            .write_all(input.as_bytes())
            .await
            .map_err(|e| AppError::execution_io_failed(e, name.to_string()))?;
    }
    Ok(())
}

pub async fn wait_command(child: Child, name: &str) -> Result<Output, AppError> {
    child
        .wait_with_output()
        .await
        .map_err(|e| AppError::execution_io_failed(e, name.to_string()))
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
