use std::{
    collections::BTreeMap,
    io::{self, stdout, Write},
    path::{Path, PathBuf},
};

use anyhow::{anyhow, Context, Ok, Result};
use chrono::Local;
use console::Style;
use dunce::canonicalize;
use git2::Repository;
use unicode_width::UnicodeWidthStr;

use crate::{
    add_and_commit,
    backups::{
        self, Backup, BackupCommand, BackupCommandExt, BackupLog, BackupResult, BackupTarget,
        Backups, ExternallyInvoked,
    },
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
    config_dir: &Path,
    storages: &Storages,
) -> Result<()> {
    trace!("Canonicalize path: {:?}", src);
    let src = canonicalize(util::expand_tilde(src)?)?;
    trace!("Canonicalize path: {:?}", dest);
    let dest = canonicalize(util::expand_tilde(dest)?)?;
    let device = devices::get_device(config_dir)?;
    let new_backup = new_backup(name, src, dest, cmd, &device, storages)?;
    let new_backup_name = new_backup.name().clone();
    let mut backups = Backups::read(config_dir, &device)?;
    println!("Backup config:");
    serde_yaml::to_writer(stdout(), &new_backup)?;
    backups.add(new_backup)?;
    backups.write(config_dir, &device)?;

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
        util::min_parent_storage(&src, storages, device).context(format!(
            "Coundn't find parent storage for src directory {}",
            src.display()
        ))?;
    let (dest_parent, dest_diff) =
        util::min_parent_storage(&dest, storages, device).context(format!(
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
        src_target?,
        dest_target?,
        command,
    ))
}

pub fn cmd_backup_list(
    src_storage: Option<String>,
    dest_storage: Option<String>,
    device_name: Option<String>,
    longprint: bool,
    config_dir: &Path,
    storages: &Storages,
) -> Result<()> {
    let devices = devices::get_devices(config_dir)?;
    let backups: BTreeMap<(String, String), Backup> = match device_name {
        Some(device_name) => {
            let device = devices
                .iter()
                .find(|dev| dev.name() == device_name)
                .context(format!("Device with name {} doesn't exist", device_name))?;
            let backups = Backups::read(config_dir, device)?;
            let mut allbackups = BTreeMap::new();
            for (name, backup) in backups.list {
                if allbackups.insert((device.name(), name), backup).is_some() {
                    return Err(anyhow!("unexpected duplication in backups hashmap"));
                };
            }
            allbackups
        }
        None => {
            let mut allbackups = BTreeMap::new();
            for device in &devices {
                let backups = Backups::read(config_dir, device)?;
                for (name, backup) in backups.list {
                    if allbackups.insert((device.name(), name), backup).is_some() {
                        return Err(anyhow!("unexpected duplication in backups hashmap"));
                    };
                }
            }
            allbackups
        }
    };
    // source/destination filtering
    let backups: BTreeMap<(String, String), Backup> = backups
        .into_iter()
        .filter(|((_dev, _name), backup)| {
            let src_matched = match &src_storage {
                Some(src_storage) => &backup.source().storage == src_storage,
                None => true,
            };
            let dest_matched = match &dest_storage {
                Some(dest_storage) => &backup.destination().storage == dest_storage,
                None => true,
            };
            src_matched && dest_matched
        })
        .collect();

    let mut stdout = io::BufWriter::new(io::stdout());
    write_backups_list(&mut stdout, backups, longprint, storages, &devices)?;
    stdout.flush()?;
    Ok(())
}

/// TODO: status printing
fn write_backups_list(
    mut writer: impl io::Write,
    backups: BTreeMap<(String, String), Backup>,
    longprint: bool,
    storages: &Storages,
    devices: &[Device],
) -> Result<()> {
    let mut name_width = 0;
    let mut dev_width = 0;
    let mut src_width = 0;
    let mut src_storage_width = 0;
    let mut dest_width = 0;
    let mut dest_storage_width = 0;
    let mut cmd_name_width = 0;
    // get widths
    for ((dev, _name), backup) in &backups {
        let device = backup.device(devices).context(format!(
            "Couldn't find device specified in backup config {}",
            backup.name()
        ))?;
        name_width = name_width.max(backup.name().width());
        dev_width = dev_width.max(dev.width());
        let src = backup
            .source()
            .path(storages, device)
            .context("Couldn't get path for source")?;
        src_width = src_width.max(format!("{}", src.display()).width());
        src_storage_width = src_storage_width.max(backup.source().storage.width());
        let dest = backup
            .destination()
            .path(storages, device)
            .context("Couldn't get path for destination")?;
        dest_width = dest_width.max(format!("{}", dest.display()).width());
        dest_storage_width = dest_storage_width.max(backup.destination().storage.width());
        let cmd_name = backup.command().name();
        cmd_name_width = cmd_name_width.max(cmd_name.width());
    }
    // main printing
    for ((dev, _name), backup) in &backups {
        let device = backup.device(devices).context(format!(
            "Couldn't find the device specified in the backup config: {}",
            backup.name()
        ))?;
        let src = backup
            .source()
            .path(storages, device)
            .context("Couldn't get path for source")?;
        let dest = backup
            .destination()
            .path(storages, device)
            .context("Couldn't get path for destination")?;
        let cmd_name = backup.command().name();
        let (last_backup_elapsed, style_on_time_elapsed) = match backup.last_backup() {
            Some(log) => {
                let time = Local::now() - log.datetime;
                let s = util::format_summarized_duration(time);
                let style = util::duration_style(time);
                (style.apply_to(s), style)
            }
            None => {
                let style = Style::new().red();
                (style.apply_to("---".to_string()), style)
            }
        };
        if !longprint {
            writeln!(
                writer,
                "{name:<name_width$} [{dev:<dev_width$}] {src:<src_storage_width$} → {dest:<dest_storage_width$} {last_backup_elapsed}",
                name = style_on_time_elapsed.apply_to(backup.name()),
                dev = console::style(dev).blue(),
                src = backup.source().storage,
                dest = backup.destination().storage,
            )?;
        } else {
            writeln!(
                writer,
                "[{dev:<dev_width$}] {name:<name_width$} {last_backup_elapsed}",
                dev = console::style(dev).blue(),
                name = style_on_time_elapsed.bold().apply_to(backup.name()),
            )?;
            let last_backup_date = match backup.last_backup() {
                Some(date) => date.datetime.format("%Y-%m-%d %T").to_string(),
                None => "never".to_string(),
            };
            let cmd_note = backup.command().note();
            writeln!(
                writer,
                "{s_src} {src}",
                s_src = console::style("src :").italic().bright().black(),
                src = src.display()
            )?;
            writeln!(
                writer,
                "{s_dest} {dest}",
                s_dest = console::style("dest:").italic().bright().black(),
                dest = dest.display()
            )?;
            writeln!(
                writer,
                "{s_last} {last}",
                s_last = console::style("last:").italic().bright().black(),
                last = last_backup_date,
            )?;
            writeln!(
                writer,
                "{s_cmd} {cmd_name}({note})",
                s_cmd = console::style("cmd :").italic().bright().black(),
                cmd_name = console::style(cmd_name).underlined(),
                note = console::style(cmd_note).italic(),
            )?;
            writeln!(writer)?;
        }
    }
    Ok(())
}

pub fn cmd_backup_done(
    name: String,
    exit_status: u64,
    log: Option<String>,
    repo: Repository,
    config_dir: &Path,
) -> Result<()> {
    let device = devices::get_device(config_dir)?;
    let mut backups = Backups::read(config_dir, &device)?;
    let backup = backups
        .get_mut(&name)
        .context(format!("Failed to get backup with name {}", name))?;
    trace!("Got backup: {:?}", backup);
    let backup_name = backup.name().clone();
    let status = BackupResult::from_exit_code(exit_status);
    let new_log = BackupLog::new_with_current_time(status, log.unwrap_or("".to_string()));
    trace!("New backup log: {:?}", new_log);
    backup.add_log(new_log);
    trace!("Added");
    backups.write(config_dir, &device)?;
    add_and_commit(
        &repo,
        &backups::backups_file(&device),
        &format!("Done backup: {}", backup_name),
    )?;
    Ok(())
}

#[cfg(test)]
mod test {
    use std::path::{Component, PathBuf};

    use anyhow::Result;

    use crate::{
        cmd_args::BackupAddCommands,
        devices::Device,
        storages::{online_storage::OnlineStorage, Storage, Storages},
    };

    use super::new_backup;
    #[test]
    fn test_new_backup() -> Result<()> {
        let device = Device::new("dev".to_string());
        let storage1 = Storage::Online(OnlineStorage::new(
            "online".to_string(),
            "provider".to_string(),
            1_000_000_000,
            "alias".to_string(),
            PathBuf::new()
                .join(Component::RootDir)
                .join("mnt")
                .join("sample"),
            &device,
        ));
        let storage2 = Storage::Online(OnlineStorage::new(
            "online2".to_string(),
            "provider".to_string(),
            1_000_000_000,
            "alias".to_string(),
            PathBuf::new()
                .join(Component::RootDir)
                .join("mnt")
                .join("different"),
            &device,
        ));
        let mut storages = Storages::new();
        storages.add(storage1)?;
        storages.add(storage2)?;
        let cmd = BackupAddCommands::External {
            name: "sampple_backup".to_string(),
            note: "This is just for test.".to_string(),
        };
        let backup = new_backup(
            "new backup".to_string(),
            PathBuf::new()
                .join(Component::RootDir)
                .join("mnt")
                .join("sample")
                .join("docs"),
            PathBuf::new()
                .join(Component::RootDir)
                .join("mnt")
                .join("sample")
                .join("tmp"),
            cmd,
            &device,
            &storages,
        )?;
        assert!(backup.source().storage == "online");
        assert_eq!(backup.source().path, vec!["docs"]);
        assert!(backup.destination().storage == "online");
        assert!(backup.destination().path == vec!["tmp"]);
        Ok(())
    }
}
