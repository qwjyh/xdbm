use std::path::PathBuf;

use anyhow::{anyhow, Context, Result};

use crate::{
    backups::Backups,
    devices,
    storages::{Storage, StorageExt, Storages},
};

pub(crate) fn cmd_check(config_dir: &PathBuf) -> Result<()> {
    info!("Config dir: {}", &config_dir.display());

    let device = devices::get_device(&config_dir)?;
    info!("Current device: {:?}", device);

    let devices = devices::get_devices(&config_dir)?;
    info!("Configured devices: {:?}", devices);

    let storages = Storages::read(&config_dir)?;
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
        let backups = Backups::read(&config_dir, &device)?;
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
        }
    }
    println!("All check passed");
    Ok(())
}
