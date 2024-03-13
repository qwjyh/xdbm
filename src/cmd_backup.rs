use std::{io::stdout, path::{Path, PathBuf}};

use anyhow::{Context, Result};
use git2::Repository;

use crate::{
    add_and_commit,
    backups::{self, Backup, BackupCommand, BackupTarget, Backups, ExternallyInvoked},
    cmd_args::BackupAddCommands,
    devices::{self, Device},
    storages::{StorageExt, Storages},
    util,
};

pub(crate) fn cmd_backup_add(
    name: String,
    src: PathBuf,
    dest: PathBuf,
    cmd: BackupAddCommands,
    repo: Repository,
    config_dir: &PathBuf,
    storages: &Storages,
) -> Result<()> {
    let device = devices::get_device(&config_dir)?;
    let new_backup = new_backup(name, src, dest, cmd, &device, storages)?;
    let new_backup_name = new_backup.name().clone();
    let mut backups = Backups::read(&config_dir, &device)?;
    println!("Backup config:");
    serde_yaml::to_writer(stdout(), &new_backup)?;
    backups.add(new_backup)?;
    backups.write(&config_dir, &device)?;

    add_and_commit(
        &repo,
        &backups::backups_file(&device),
        &format!("Add new backup: {}", new_backup_name),
    )?;

    println!("Added new backup.");
    trace!("Finished adding backup");
    Ok(())
}

fn new_backup(
    name: String,
    src: PathBuf,
    dest: PathBuf,
    cmd: BackupAddCommands,
    device: &Device,
    storages: &Storages,
) -> Result<Backup> {
    let (src_parent, src_diff) =
        util::min_parent_storage(&src, &storages, &device).context(format!(
            "Coundn't find parent storage for src directory {}",
            src.display()
        ))?;
    let (dest_parent, dest_diff) =
        util::min_parent_storage(&dest, &storages, &device).context(format!(
            "Couldn't find parent storage for dest directory: {}",
            dest.display()
        ))?;
    let src_target = BackupTarget::new(src_parent.name().to_string(), src_diff);
    trace!("Backup source target: {:?}", src_target);
    let dest_target = BackupTarget::new(dest_parent.name().to_string(), dest_diff);
    trace!("Backup destination target: {:?}", dest_target);

    let command: BackupCommand = match cmd {
        BackupAddCommands::External { name, note } => {
            BackupCommand::ExternallyInvoked(ExternallyInvoked::new(name, note))
        }
    };
    trace!("Backup command: {:?}", command);

    Ok(Backup::new(
        name,
        device.name(),
        src_target,
        dest_target,
        command,
    ))
}
