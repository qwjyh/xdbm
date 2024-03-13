use std::path::{self, PathBuf};

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
