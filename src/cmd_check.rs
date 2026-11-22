use std::path::Path;

use anyhow::{Context, Result, anyhow};

use crate::{
    backups::Backups,
    devices,
    storages::{Storage, StorageExt, Storages},
    util,
};

pub(crate) fn cmd_check(config_dir: &Path) -> Result<()> {
    info!("Config dir: {}", &config_dir.display());

    let device = devices::get_device(config_dir)?;
    info!("Current device: {:?}", device);

    let devices = devices::get_devices(config_dir)?;
    info!("Configured devices: {:?}", devices);

    let storages = Storages::read(config_dir)?;
    info!("Storages: {:?}", storages);
    if !(storages.list.iter().all(|(_name, storage)| match storage {
        Storage::SubDirectory(storage) => storage.parent(&storages).is_some(),
        _ => true,
    })) {
        return Err(anyhow!(
            "Some SubDirectory doesn't have its parent in the storages list."
        ));
    };
    info!("All SubDirectory's parent exists.");

    for device in &devices {
        let backups = Backups::read(config_dir, device)?;
        for (name, backup) in &backups.list {
            if name != backup.name() {
                return Err(anyhow!(
                    "The backup {} name and its key is different.",
                    name
                ));
            }
            let _device = backup
                .device(&devices)
                .context(format!("The backup {}'s device doesn't exist.", name))?;
            if !(storages.list.contains_key(&backup.source().storage)) {
                return Err(anyhow!(
                    "The source of backup {} doesn't exist in storages.",
                    &backup.name()
                ));
            }
            if !(storages.list.contains_key(&backup.destination().storage)) {
                return Err(anyhow!(
                    "The destination of backup {} doesn't exist in storages.",
                    &backup.name()
                ));
            }
            let parent_storage = util::min_parent_storage(
                &backup.source().path(&storages, device).unwrap(),
                &storages,
                device,
            );
            if let Some((parent_storage, _)) = parent_storage {
                if *parent_storage.name() != backup.source().storage {
                    anyhow::bail!(
                        "Backup's source storage doesn't match the parent storage: backup config = {}, parent storage = {}",
                        backup.source().storage,
                        parent_storage
                    );
                }
            } else {
                anyhow::bail!(
                    "Backup {} source path is not covered in any storage",
                    backup.name()
                );
            }
        }
    }
    println!("All check passed");
    Ok(())
}
