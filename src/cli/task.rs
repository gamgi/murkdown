use std::{
    path::PathBuf,
    sync::{Arc, Mutex},
};

use murkdown::types::LocationMap;
use walkdir::WalkDir;

use super::{
    types::AppError,
    utils::{is_file, is_visible},
};

/// Index the contents of provided paths
pub async fn index(
    paths: Vec<PathBuf>,
    locations: Arc<Mutex<LocationMap>>,
) -> Result<bool, AppError> {
    let mut locations = locations.lock().expect("poisoned lock");
    for path in paths {
        let walker = WalkDir::new(path)
            .into_iter()
            .filter_entry(is_visible)
            .filter_map(Result::ok)
            .filter(is_file);
        for entry in walker {
            locations.insert(
                entry.path().display().to_string(),
                entry.path().to_path_buf(),
            );
        }
    }

    Ok(false)
}
