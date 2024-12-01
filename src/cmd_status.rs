use anyhow::{Context, Result};
use chrono::Local;
use std::{
    env,
    path::{self, Path, PathBuf},
};

use crate::{
    backups::{Backup, Backups},
    devices::{self, Device},
    storages::{self, Storage, StorageExt, Storages},
    util,
};

// TODO: fine styling like `backup list`, or should I just use the same style?
pub(crate) fn cmd_status(
    path: Option<PathBuf>,
    show_storage: bool,
    show_backup: bool,
    config_dir: &Path,
) -> Result<()> {
    let path = path.unwrap_or(env::current_dir().context("Failed to get current directory.")?);
    let currrent_device = devices::get_device(config_dir)?;

    if show_storage {
        let storages = storages::Storages::read(config_dir)?;
        let storage = util::min_parent_storage(&path, &storages, &currrent_device);
        trace!("storage {:?}", storage);

        // TODO: recursively trace all storages for subdirectory?
        match storage {
            Some(storage) => {
                println!("Storage: {}", storage.0.name())
            }
            None => {
                println!("Storage: None");
            }
        }
    }
    if show_backup {
        let devices = devices::get_devices(config_dir)?;
        let storages = storages::Storages::read(config_dir)?;
        let backups = devices.iter().map(|device| {
            Backups::read(config_dir, device)
                .context("Backups were not found")
                .unwrap()
        });

        let (target_storage, target_diff_from_storage) =
            util::min_parent_storage(&path, &storages, &currrent_device)
                .context("Target path is not covered in any storage")?;

        let covering_backup: Vec<_> = devices
            .iter()
            .zip(backups)
            .map(|(device, backups)| {
                debug!(
                    "dev {}, storage {:?}",
                    device.name(),
                    backups
                        .list
                        .iter()
                        .map(|(backup_name, backup)| format!(
                            "{} {}",
                            backup_name,
                            backup.source().storage
                        ))
                        .collect::<Vec<_>>()
                );
                (
                    device,
                    parent_backups(
                        &target_diff_from_storage,
                        target_storage,
                        backups,
                        &storages,
                        device,
                    ),
                )
            })
            .collect();
        trace!("{:?}", covering_backup.first());

        let name_len = &covering_backup
            .iter()
            .map(|(_, backups)| {
                backups
                    .iter()
                    .map(|(backup, _path)| backup.name().len())
                    .max()
                    .unwrap_or(0)
            })
            .max()
            .unwrap_or(5);

        for (backup_device, covering_backups) in covering_backup {
            println!("Device: {}", backup_device.name());
            for (backup, path_from_backup) in covering_backups {
                let last_backup = match backup.last_backup() {
                    Some(log) => util::format_summarized_duration(Local::now() - log.datetime),
                    None => "---".to_string(),
                };
                println!(
                    "  {:<name_len$} {} {}",
                    console::style(backup.name()).bold(),
                    last_backup,
                    path_from_backup.display(),
                );
            }
        }
    }
    todo!()
}

/// Get [`Backup`]s for `device` which covers `target_path`.
/// Returns [`Vec`] of tuple of [`Backup`] and relative path from the backup root.
fn parent_backups<'a>(
    target_path_from_storage: &'a Path,
    target_storage: &'a Storage,
    backups: Backups,
    storages: &'a Storages,
    device: &'a Device,
) -> Vec<(Backup, PathBuf)> {
    trace!("Dev {:?}", device.name());
    let target_path = match target_storage.mount_path(device) {
        Some(target_path) => target_path.join(target_path_from_storage),
        None => return vec![],
    };
    trace!("Path on the device {:?}", target_path);
    backups
        .list
        .into_iter()
        .filter_map(|(_k, backup)| {
            let backup_path = backup.source().path(storages, device)?;
            trace!("{:?}", backup_path.components());
            let diff = pathdiff::diff_paths(&target_path, backup_path.clone())?;
            trace!("Backup: {:?}, Diff: {:?}", backup_path, diff);
            if diff.components().any(|c| c == path::Component::ParentDir) {
                None
            } else {
                Some((backup, diff))
            }
        })
        .collect()
}

#[cfg(test)]
mod test {
    use std::{path::PathBuf, vec};

    use crate::{
        backups::{self, ExternallyInvoked},
        devices,
        storages::{self, online_storage::OnlineStorage, StorageExt},
        util,
    };

    use super::parent_backups;

    #[test]
    fn test_parent_backups() {
        let device1 = devices::Device::new("device_1".to_string());
        let mut storage1 = storages::Storage::Online(OnlineStorage::new(
            "storage_1".to_string(),
            "smb".to_string(),
            1_000_000,
            "str1".to_string(),
            PathBuf::from("/home/foo/"),
            &device1,
        ));
        let storage2 = storages::Storage::Online(OnlineStorage::new(
            "storage_2".to_string(),
            "smb".to_string(),
            1_000_000_000,
            "str2".to_string(),
            PathBuf::from("/"),
            &device1,
        ));
        let device2 = devices::Device::new("device_2".to_string());
        storage1
            .bound_on_device("alias".to_string(), PathBuf::from("/mnt/dev"), &device2)
            .unwrap();
        let storage3 = storages::Storage::Online(OnlineStorage::new(
            "storage_3".to_string(),
            "smb".to_string(),
            2_000_000_000,
            "str2".to_string(),
            PathBuf::from("/"),
            &device2,
        ));
        let storages = {
            let mut storages = storages::Storages::new();
            storages.add(storage1).unwrap();
            storages.add(storage2).unwrap();
            storages.add(storage3).unwrap();
            storages
        };

        let backup1 = backups::Backup::new(
            "backup_1".to_string(),
            device1.name().to_string(),
            backups::BackupTarget {
                storage: "storage_1".to_string(),
                path: vec!["bar".to_string()],
            },
            backups::BackupTarget {
                storage: "storage_1".to_string(),
                path: vec!["hoge".to_string()],
            },
            backups::BackupCommand::ExternallyInvoked(ExternallyInvoked::new(
                "cmd".to_string(),
                "".to_string(),
            )),
        );
        let backup2 = backups::Backup::new(
            "backup_2".to_string(),
            device2.name().to_string(),
            backups::BackupTarget {
                storage: "storage_1".to_string(),
                path: vec!["".to_string()],
            },
            backups::BackupTarget {
                storage: "storage_3".to_string(),
                path: vec!["foo".to_string()],
            },
            backups::BackupCommand::ExternallyInvoked(ExternallyInvoked::new(
                "cmd".to_string(),
                "".to_string(),
            )),
        );

        let backups = {
            let mut backups = backups::Backups::new();
            backups.add(backup1).unwrap();
            backups.add(backup2).unwrap();
            backups
        };

        let target_path1 = PathBuf::from("/home/foo/bar/hoo");
        let (target_storage1, target_path_from_storage1) =
            util::min_parent_storage(&target_path1, &storages, &device1)
                .expect("Failed to get storage");
        let covering_backups_1 = parent_backups(
            &target_path_from_storage1,
            target_storage1,
            backups.clone(),
            &storages,
            &device1,
        );
        assert_eq!(covering_backups_1.len(), 2);

        let target_path2 = PathBuf::from("/mnt/");
        let (target_storage2, target_path_from_storage2) =
            util::min_parent_storage(&target_path2, &storages, &device2)
                .expect("Failed to get storage");
        let covering_backups_2 = parent_backups(
            &target_path_from_storage2,
            target_storage2,
            backups.clone(),
            &storages,
            &device2,
        );
        assert_eq!(covering_backups_2.len(), 0);

        let target_path3 = PathBuf::from("/mnt/dev/foo");
        let (target_storage3, target_path_from_storage3) =
            util::min_parent_storage(&target_path3, &storages, &device2)
                .expect("Failed to get storage");
        let covering_backups_3 = parent_backups(
            &target_path_from_storage3,
            target_storage3,
            backups,
            &storages,
            &device2,
        );
        assert_eq!(covering_backups_3.len(), 1);
        let mut covering_backup_names_3 =
            covering_backups_3.iter().map(|(backup, _)| backup.name());
        assert_eq!(covering_backup_names_3.next().unwrap(), "backup_2");
        assert!(covering_backup_names_3.next().is_none());
    }
}
