use std::path::{self, PathBuf};

use anyhow::{Context, Result};

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
        .min_by_key(|(_k, pathdiff)| {
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

#[cfg(test)]
mod test {
    use anyhow::Result;
    use std::path::PathBuf;

    use crate::{
        devices::Device,
        storages::{online_storage::OnlineStorage, Storage, StorageExt, Storages},
    };

    use super::{expand_tilde, min_parent_storage};

    #[test]
    fn test_min_parent_storage() -> Result<()> {
        let device = Device::new("name".to_string());
        let storage1 = Storage::Online(OnlineStorage::new(
            "storage_name".to_string(),
            "provider".to_string(),
            1_000_000,
            "alias".to_string(),
            PathBuf::from("/mnt"),
            &device,
        ));
        let storage2 = Storage::Online(OnlineStorage::new(
            "root".to_string(),
            "provider".to_string(),
            1_000_000,
            "root".to_string(),
            PathBuf::from("/"),
            &device,
        ));
        let mut storages = Storages::new();
        storages.add(storage1)?;
        storages.add(storage2)?;

        let sample_target = PathBuf::from("/mnt/docs");
        let parent = min_parent_storage(&sample_target, &storages, &device);
        assert_eq!(parent.clone().unwrap().0.name(), "storage_name");
        assert_eq!(parent.unwrap().1, PathBuf::from("docs"));

        let sample_target_2 = PathBuf::from("/home/user/.config");
        let parent_2 = min_parent_storage(&sample_target_2, &storages, &device).unwrap();
        assert_eq!(parent_2.clone().0.name(), "root");
        assert_eq!(parent_2.1, PathBuf::from("home/user/.config"));

        Ok(())
    }

    #[test]
    fn test_expand_tilde() -> Result<()> {
        assert!(expand_tilde(PathBuf::from("/aaa/bbb/ccc"))
            .unwrap()
            .eq(&PathBuf::from("/aaa/bbb/ccc")));
        let expanded = expand_tilde(PathBuf::from("~/aaa/bbb/ccc"));
        match expanded {
            Ok(path) => assert!(path.eq(&dirs::home_dir().unwrap().join("aaa/bbb/ccc"))),
            Err(_) => (),
        }
        let expanded = expand_tilde(PathBuf::from("aaa/~/bbb"));
        match expanded {
            Ok(path) => assert_eq!(path, PathBuf::from("aaa/~/bbb")),
            Err(_) => (),
        }
        Ok(())
    }
}
