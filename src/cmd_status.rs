use anyhow::{Context, Result};
use console::Style;
use std::{
    env,
    path::{self, Path, PathBuf},
};

use crate::{
    backups::Backups,
    devices::{self, Device},
    storages::{self, StorageExt, Storages},
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
    let device = devices::get_device(config_dir)?;

    if show_storage {
        let storages = storages::Storages::read(config_dir)?;
        let storage = util::min_parent_storage(&path, &storages, &device);

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
        let backups = Backups::read(config_dir, &device)?;
        let covering_backup = devices
            .iter()
            .map(|device| (device, parent_backups(&path, &backups, &storages, device)));

        for (backup_device, covering_backups) in covering_backup {
            println!("Device: {}", backup_device.name());
            for backup in covering_backups {
                println!("  {}", console::style(backup.0).bold());
            }
        }
    }
    todo!()
}

fn parent_backups<'a>(
    target_path: &'a PathBuf,
    backups: &'a Backups,
    storages: &'a Storages,
    device: &'a Device,
) -> Vec<(&'a String, PathBuf)> {
    backups
        .list
        .iter()
        .filter_map(|(k, v)| {
            let backup_path = match v.source().path(storages, device) {
                Ok(path) => path,
                Err(e) => {
                    error!("Error while getting backup source path: {}", e);
                    return None;
                }
            };
            let diff = pathdiff::diff_paths(target_path, backup_path)?;
            if diff.components().any(|c| c == path::Component::ParentDir) {
                None
            } else {
                Some((k, diff))
            }
        })
        .collect()
}
