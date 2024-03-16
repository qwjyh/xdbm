use std::path::{self, PathBuf};

use anyhow::{Context, Result};
use chrono::format;

use crate::{
    devices::Device,
    storages::{Storage, StorageExt, Storages},
};

/// Find the closest parent storage from the `path`.
pub fn min_parent_storage<'a>(
    path: &PathBuf,
    storages: &'a Storages,
    device: &'a Device,
) -> Option<(&'a Storage, PathBuf)> {
    let (name, pathdiff) = storages
        .list
        .iter()
        .filter_map(|(k, storage)| {
            let storage_path = match storage.mount_path(device, storages) {
                Ok(path) => path,
                Err(_) => return None,
            };
            let diff = pathdiff::diff_paths(&path, storage_path)?;
            if diff.components().any(|c| c == path::Component::ParentDir) {
                None
            } else {
                Some((k, diff))
            }
        })
        .min_by_key(|(k, pathdiff)| {
            pathdiff
                .components()
                .collect::<Vec<path::Component>>()
                .len()
        })?;
    let storage = storages.get(name)?;
    Some((storage, pathdiff))
}

/// Expand first `~` in path as `home_dir`.
pub fn expand_tilde(path: PathBuf) -> Result<PathBuf> {
    if path.components().next() == Some(path::Component::Normal("~".as_ref())) {
        let mut expanded_path = dirs::home_dir().context("Failed to expand home directory.")?;
        for c in path.components().skip(1) {
            expanded_path.push(c)
        }
        Ok(expanded_path)
    } else {
        Ok(path)
    }
}

pub fn format_summarized_duration(dt: chrono::Duration) -> String {
    if dt.num_days() > 0 {
        format!("{}d", dt.num_days())
    } else if dt.num_hours() > 0 {
        format!("{}h", dt.num_hours())
    } else {
        format!("{}min", dt.num_minutes())
    }
}
